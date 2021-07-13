// This file is part of the Polymesh distribution (https://github.com/PolymathNetwork/Polymesh).
// Copyright (c) 2020 Polymath

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

//! # Rewards Module
//!
//! TODO.
//!
//! ## Overview
//!
//! TODO
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `TODO`: TODO.

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]

use codec::{Decode, Encode};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::DispatchResult,
    ensure,
    traits::{Currency, ExistenceRequirement, GetCallMetadata, IsSubType, WithdrawReasons},
    unsigned::{TransactionValidity, TransactionValidityError},
    weights::{DispatchClass, Pays},
};
use pallet_identity::{self as identity, PermissionedCallOriginData};
use pallet_staking::{self as staking, RewardDestination};
use polymesh_common_utilities::{
    constants::{currency::POLY, REWARDS_MODULE_ID},
    traits::{identity::Config as IdentityConfig, CommonConfig},
    with_transaction,
};
use polymesh_primitives::IdentityId;
use sp_runtime::transaction_validity::InvalidTransaction;
use sp_runtime::{
    traits::{AccountIdConversion, DispatchInfoOf, SignedExtension, StaticLookup, Verify},
    transaction_validity::ValidTransaction,
    DispatchError,
};
use sp_std::{convert::TryInto, fmt, marker::PhantomData};

type Identity<T> = identity::Module<T>;
type Staking<T> = staking::Module<T>;
type BalanceOf<T> = <<T as pallet_staking::Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

pub trait Config: frame_system::Config + IdentityConfig + staking::Config {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
    /// Weight information for extrinsic of the sto pallet.
    type WeightInfo: WeightInfo;
}

pub trait WeightInfo {}

/// Represents an Itn reward's status.
#[derive(Decode, Encode, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItnRewardStatus<Balance: Encode + Decode> {
    UnClaimed(Balance),
    Claimed,
}

/// A signed extension used to prevent invalid ITN reward claims.
#[derive(Encode, Decode, Clone, Eq, PartialEq, Default)]
pub struct ValidateItnRewardClaim<T: Config>(PhantomData<T>);

impl<T: Config> fmt::Debug for ValidateItnRewardClaim<T> {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ValidateItnRewardClaim<{:?}>", self.0)
    }

    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

decl_error! {
    pub enum Error for Module<T: Config> {
        /// Address was not found in the list of Itn addresses.
        UnknownItnAddress,
        /// Itn reward was already claimed.
        ItnRewardAlreadyClaimed,
        /// Provided signature was invalid.
        InvalidSignature,
        /// Balance can not be converted to a primitive.
        UnableToCovertBalance,
        // Insufficient balance to payout rewards.
        InsufficientBalance,
    }
}

decl_storage! {
    trait Store for Module<T: Config> as Rewards {
        /// Map of (Itn Address `T::AccountId`) -> (Reward `ItnRewardStatus`).
        pub ItnRewards get(fn itn_rewards)
            config(): map hasher(blake2_128_concat) T::AccountId
            => Option<ItnRewardStatus<T::Balance>>;
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: <T as frame_system::Config>::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Claim an ITN reward
        ///
        /// ## Arguments
        /// `itn_address` - Address of the awarded account on ITN.
        /// `signature` - Signature of the encoded reward data (originKey, ItnKey, amount)
        ///
        /// # Errors
        /// * `InsufficientBalance` - Itn rewards has insufficient funds to issue the reward.
        /// * `InvalidSignature` - `signature` had an invalid signer or invalid message.
        /// * `ItnRewardAlreadyClaimed` - Reward issued to the `itn_address` has already been claimed.
        /// * `UnknownItnAddress` - `itn_address` is not in the rewards table and has no reward to be claimed.
        #[weight = (0, DispatchClass::Operational, Pays::No)]
        pub fn claim_itn_reaward(origin, itn_address: T::AccountId, signature: T::OffChainSignature) -> DispatchResult {
            let PermissionedCallOriginData{ sender, primary_did, .. } = <Identity<T>>::ensure_origin_call_permissions(origin.clone())?;
            <ItnRewards<T>>::try_mutate(&itn_address, |reward| {
                match reward{
                    // Unclaimed. Attempt to claim.
                    Some(ItnRewardStatus::UnClaimed(amount)) => {
                        let amount = *amount;
                        // `amount` and `bonded_amount` are equal in value but different types.
                        // `deposit_amount` is 1 POLY more because we bond `bonded_amount`, we don't want all the users poly bonded.
                        let (bonded_amount, deposit_amount) = Self::convert_balance(amount)?;
                        ensure!(
                            Self::balance() >= deposit_amount,
                            Error::<T>::InsufficientBalance
                        );
                        ensure!(
                            signature.verify((sender.clone(), itn_address.clone(), amount).encode().as_slice(), &itn_address),
                            Error::<T>::InvalidSignature
                        );
                        with_transaction(|| {
                           let _ = T::Currency::withdraw(
                                &Self::account_id(),
                                deposit_amount,
                                WithdrawReasons::TRANSFER,
                                ExistenceRequirement::AllowDeath,
                            );
                            let _ = T::Currency::deposit_into_existing(&sender, deposit_amount);
                            if <Staking<T>>::bonded(&sender).is_some() {
                                <Staking<T>>::bond_extra(origin, bonded_amount)?;
                            } else {
                                <Staking<T>>::bond(origin, T::Lookup::unlookup(sender), bonded_amount, RewardDestination::Staked)?;
                            }
                            *reward = Some(ItnRewardStatus::Claimed);
                            Self::deposit_event(Event::<T>::ItnRwardClaimed(primary_did, amount));
                            Ok(())
                        })
                    },
                    // Already Claimed.
                    Some(ItnRewardStatus::Claimed) => Err(Error::<T>::ItnRewardAlreadyClaimed.into()),
                    // Unknown Address.
                    None => Err(Error::<T>::UnknownItnAddress.into()),
                }
            })
        }
    }
}

decl_event! {
    pub enum Event<T>
    where
        Balance = <T as CommonConfig>::Balance,
    {
        /// Itn reward was claimed.
        ItnRwardClaimed(IdentityId, Balance),
    }
}

impl<T: Config> Module<T> {
    /// The account ID of the rewards pot.
    ///
    /// This actually does computation. If you need to keep using it, then make sure you cache the
    /// value and only call this once.
    pub fn account_id() -> T::AccountId {
        REWARDS_MODULE_ID.into_account()
    }

    fn balance() -> BalanceOf<T> {
        <T as pallet_staking::Config>::Currency::free_balance(&Self::account_id())
    }

    fn convert_balance(balance: T::Balance) -> Result<(BalanceOf<T>, BalanceOf<T>), DispatchError> {
        let raw_balance: u128 = balance
            .try_into()
            .map_err(|_| Error::<T>::UnableToCovertBalance)?;
        Ok((
            raw_balance
                .try_into()
                .map_err(|_| Error::<T>::UnableToCovertBalance)?,
            (raw_balance.saturating_add(1 * POLY))
                .try_into()
                .map_err(|_| Error::<T>::UnableToCovertBalance)?,
        ))
    }

    fn valid_claim(
        sender: &T::AccountId,
        itn_address: &T::AccountId,
        signature: &T::OffChainSignature,
    ) -> bool {
        match <ItnRewards<T>>::get(itn_address) {
            Some(ItnRewardStatus::UnClaimed(amount)) => signature.verify(
                (sender.clone(), itn_address.clone(), amount)
                    .encode()
                    .as_slice(),
                itn_address,
            ),
            _ => false,
        }
    }
}

impl<T> SignedExtension for ValidateItnRewardClaim<T>
where
    T: Config + Send + Sync,
    <T as frame_system::Config>::Call: GetCallMetadata + IsSubType<Call<T>>,
{
    const IDENTIFIER: &'static str = "ValidateItnRewardClaim";
    type AccountId = T::AccountId;
    type Call = <T as frame_system::Config>::Call;
    type AdditionalSigned = ();
    type Pre = ();

    fn additional_signed(&self) -> Result<(), TransactionValidityError> {
        Ok(())
    }

    fn validate(
        &self,
        sender: &Self::AccountId,
        call: &Self::Call,
        _: &DispatchInfoOf<Self::Call>,
        _: usize,
    ) -> TransactionValidity {
        if let Some(local_call) = IsSubType::<Call<T>>::is_sub_type(call) {
            if let Call::claim_itn_reaward(itn_address, signature) = local_call {
                if !<Module<T>>::valid_claim(sender, itn_address, signature) {
                    return Err(TransactionValidityError::Invalid(
                        InvalidTransaction::Custom(0),
                    ));
                }
            }
        }
        Ok(ValidTransaction::default())
    }
}
