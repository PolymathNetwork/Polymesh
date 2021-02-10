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

//! # Protocol Fee Module
//!
//! This module stores the fee of each protocol operation, and a common coefficient which is applied on
//! fee computation.
//!
//! It also provides helper functions to calculate and charge fees on each protocol operation.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - [change_coefficient](Module::change_coefficient) - It changes the fee coefficient.
//! - [change_base_fee](Module::change_base_fee) - It changes the base fee.
//!
//! ### Public Functions
//!
//! - [compute_fee](Module::compute_fee) - It computes the fee of the operation.
//! - [charge_fee](Module::charge_fee) - It calculates the fee and charges it.
//! - [batch_charge_fee](Module::batch_charge_fee) - It calculates the fee and charges it on a batch operation.
//!
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    traits::{Currency, ExistenceRequirement, Imbalance, OnUnbalanced, WithdrawReason},
    weights::Weight,
};
use frame_system::ensure_root;
use pallet_identity as identity;
use polymesh_common_utilities::{
    identity::Trait as IdentityTrait,
    protocol_fee::{ChargeProtocolFee, ProtocolOp},
    transaction_payment::CddAndFeeDetails,
    Context, GC_DID,
};
use polymesh_primitives::{IdentityId, PosRatio};
use sp_runtime::{
    traits::{Saturating, Zero},
    Perbill,
};

type BalanceOf<T> =
    <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
    <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::NegativeImbalance;
/// Either an imbalance or an error.
type WithdrawFeeResult<T> = sp_std::result::Result<NegativeImbalanceOf<T>, DispatchError>;
type Identity<T> = identity::Module<T>;

pub trait WeightInfo {
    fn change_coefficient() -> Weight;
    fn change_base_fee() -> Weight;
}

pub trait Trait: frame_system::Trait + IdentityTrait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// The currency type in which fees will be paid.
    type Currency: Currency<Self::AccountId> + Send + Sync;
    /// Handler for the unbalanced reduction when taking protocol fees.
    type OnProtocolFeePayment: OnUnbalanced<NegativeImbalanceOf<Self>>;
    /// Weight calaculation.
    type WeightInfo: WeightInfo;
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// Insufficient account balance to pay the fee.
        InsufficientAccountBalance,
        /// Not able to handled the imbalances
        UnHandledImbalances
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as ProtocolFee {
        /// The mapping of operation names to the base fees of those operations.
        pub BaseFees get(fn base_fees) config(): map hasher(twox_64_concat) ProtocolOp => BalanceOf<T>;
        /// The fee coefficient as a positive rational (numerator, denominator).
        pub Coefficient get(fn coefficient) config() build(|config: &GenesisConfig<T>| {
            if config.coefficient.1 == 0 {
                PosRatio(1, 1)
            } else {
                config.coefficient
            }
        }): PosRatio;
    }
}

decl_event! {
    pub enum Event<T> where
        AccountId = <T as frame_system::Trait>::AccountId,
        Balance = BalanceOf<T>,
    {
        /// The protocol fee of an operation.
        FeeSet(IdentityId, Balance),
        /// The fee coefficient.
        CoefficientSet(IdentityId, PosRatio),
        /// Fee charged.
        FeeCharged(AccountId, Balance),
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Changes the fee coefficient for the root origin.
        ///
        /// # Errors
        /// * `BadOrigin` - Only root allowed.
        #[weight = <T as Trait>::WeightInfo::change_coefficient()]
        pub fn change_coefficient(origin, coefficient: PosRatio) {
            ensure_root(origin)?;
            let id = Context::current_identity::<Identity<T>>().unwrap_or(GC_DID);

            Coefficient::put(&coefficient);
            Self::deposit_event(RawEvent::CoefficientSet(id, coefficient));
        }

        /// Changes the a base fee for the root origin.
        ///
        /// # Errors
        /// * `BadOrigin` - Only root allowed.
        #[weight = <T as Trait>::WeightInfo::change_base_fee()]
        pub fn change_base_fee(origin, op: ProtocolOp, base_fee: BalanceOf<T>) {
            ensure_root(origin)?;
            let id = Context::current_identity::<Identity<T>>().unwrap_or(GC_DID);

            <BaseFees<T>>::insert(op, &base_fee);
            Self::deposit_event(RawEvent::FeeSet(id, base_fee));
        }
    }
}

impl<T: Trait> Module<T> {
    /// Computes the fee of the operation as `(base_fee * coefficient.0) / coefficient.1`.
    pub fn compute_fee(ops: &[ProtocolOp]) -> BalanceOf<T> {
        let coefficient = Self::coefficient();
        let ratio = Perbill::from_rational_approximation(coefficient.0, coefficient.1);
        let base = ops.iter().fold(Zero::zero(), |a, e| a + Self::base_fees(e));
        ratio * base
    }

    /// Computes the fee of the operations and charges it to the current payer. The fee is then
    /// credited to the intended recipients according to the implementation of
    /// `OnProtocolFeePayment`.
    pub fn charge_fees(ops: &[ProtocolOp]) -> DispatchResult {
        if ops.is_empty() {
            return Ok(());
        }
        let fee = Self::compute_fee(ops);
        if fee.is_zero() {
            return Ok(());
        }
        Self::withdraw_from_payer(fee)
    }

    /// Used to charge the instantiation fee of the smart extension.
    /// fee get divided between the owner of the template and the network (Treasury + Block Author).
    pub fn charge_extension_instantiation_fee(
        fee: BalanceOf<T>,
        owner: T::AccountId,
        network_share: Perbill,
    ) -> DispatchResult {
        if let Some(payer) = T::CddHandler::get_payer_from_context() {
            // 1. Withdraw fee from the payer balance.
            let negative_imbalance = Self::withdraw_fee(payer, fee)?;

            // 2. Calculate the amount that need to transfer to the owner of the SE template.
            let owner_amount = fee.saturating_sub(network_share * fee);
            // 3. Deposit the `owner_amount` into the owner address.
            let positive_imbalance = T::Currency::deposit_into_existing(&owner, owner_amount)?;

            // It always return the negative imbalance as negative_imbalance always >= positive_imbalance.
            let imbalance = negative_imbalance
                .offset(positive_imbalance)
                .map_err(|_| Error::<T>::UnHandledImbalances)?;
            T::OnProtocolFeePayment::on_unbalanced(imbalance);
        }
        Ok(())
    }

    /// Computes the fee for `count` similar operations, and charges that fee to the current payer.
    pub fn batch_charge_fee(op: ProtocolOp, count: usize) -> DispatchResult {
        let fee = Self::compute_fee(&[op]).saturating_mul(<BalanceOf<T>>::from(count as u32));
        if fee.is_zero() {
            return Ok(());
        }
        Self::withdraw_from_payer(fee)
    }

    /// Withdraws a precomputed fee from the current payer if it is defined or from the current
    /// identity otherwise.
    fn withdraw_fee(account: T::AccountId, fee: BalanceOf<T>) -> WithdrawFeeResult<T> {
        let ret = T::Currency::withdraw(
            &account,
            fee,
            WithdrawReason::Fee.into(),
            ExistenceRequirement::KeepAlive,
        )
        .map_err(|_| Error::<T>::InsufficientAccountBalance)?;
        Self::deposit_event(RawEvent::FeeCharged(account, fee));
        Ok(ret)
    }

    fn withdraw_from_payer(fee: BalanceOf<T>) -> DispatchResult {
        if let Some(payer) = T::CddHandler::get_payer_from_context() {
            let imbalance = Self::withdraw_fee(payer, fee)?;
            T::OnProtocolFeePayment::on_unbalanced(imbalance);
        }
        Ok(())
    }
}

impl<T: Trait> ChargeProtocolFee<T::AccountId, BalanceOf<T>> for Module<T> {
    fn charge_fee(op: ProtocolOp) -> DispatchResult {
        Self::charge_fees(&[op])
    }

    fn charge_fees(ops: &[ProtocolOp]) -> DispatchResult {
        Self::charge_fees(ops)
    }

    fn batch_charge_fee(op: ProtocolOp, count: usize) -> DispatchResult {
        Self::batch_charge_fee(op, count)
    }

    fn charge_extension_instantiation_fee(
        fee: BalanceOf<T>,
        owner: T::AccountId,
        network_share: Perbill,
    ) -> DispatchResult {
        Self::charge_extension_instantiation_fee(fee, owner, network_share)
    }
}
