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

//! # Treasury Module
//!
//! Treasury module contains a couple of functions to manage the treasury through the governance
//! module.
//!
//! ## Overview
//!
//! Treasury balance is filled by fees of each operation, but it also accepts donations
//! through [reimbursement](Module::reimbursement) method.
//!
//! The disbursement mechanism is designed to incentivize Polymesh Improvement Proposals.
//!
//! ## Dispatchable Functions
//!
//! - [disbursement](Module::disbursement) - Transfers from the treasury to the given benericiaries.
//! - [reimbursement](Module::reimbursement) - Transfers to the treasury.
//!
#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

use frame_support::{
    decl_error, decl_event, decl_module, ensure,
    traits::{Currency, ExistenceRequirement, Imbalance, OnUnbalanced, WithdrawReason},
    weights::Weight,
};
use frame_system::ensure_root;
use pallet_identity as identity;
use polymesh_common_utilities::{
    constants::TREASURY_MODULE_ID, traits::balances::Trait as BalancesTrait, Context, GC_DID,
};
use polymesh_primitives::{Beneficiary, IdentityId};
use sp_runtime::traits::{AccountIdConversion, Saturating};
use sp_std::prelude::*;

pub type ProposalIndex = u32;

type Identity<T> = identity::Module<T>;
type BalanceOf<T> =
    <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
    <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::NegativeImbalance;

pub trait Trait: frame_system::Trait + BalancesTrait {
    // The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// The native currency.
    type Currency: Currency<Self::AccountId>;
    /// Weight information for extrinsics in the identity pallet.
    type WeightInfo: WeightInfo;
}

pub trait TreasuryTrait<Balance> {
    fn disbursement(target: IdentityId, amount: Balance);
    fn balance() -> Balance;
}

pub trait WeightInfo {
    fn reimbursement() -> Weight;
    fn disbursement(beneficiary_count: u32) -> Weight;
}

decl_event!(
    pub enum Event<T>
    where
        Balance = BalanceOf<T>,
    {
        /// Disbursement to a target Identity.
        /// (target identity, amount)
        TreasuryDisbursement(IdentityId, IdentityId, Balance),

        /// Treasury reimbursement.
        TreasuryReimbursement(IdentityId, Balance),
    }
);

decl_error! {
    /// Error for the treasury module.
    pub enum Error for Module<T: Trait> {
        /// Proposer's balance is too low.
        InsufficientBalance,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// It transfers balances from treasury to each of beneficiaries and the specific amount
        /// for each of them.
        ///
        /// # Error
        /// * `BadOrigin`: Only root can execute transaction.
        /// * `InsufficientBalance`: If treasury balances is not enough to cover all beneficiaries.
        #[weight = <T as Trait>::WeightInfo::disbursement( beneficiaries.len() as u32)]
        pub fn disbursement(origin, beneficiaries: Vec<Beneficiary<BalanceOf<T>>>) {
            ensure_root(origin)?;

            // Ensure treasury has enough balance.
            let total_amount = beneficiaries.iter().fold(0u32.into(), |acc,b| b.amount.saturating_add(acc));
            ensure!(
                Self::balance() >= total_amount,
                Error::<T>::InsufficientBalance
            );
            beneficiaries.into_iter().for_each(|b| Self::unsafe_disbursement(b.id, b.amount));
        }

        /// It transfers the specific `amount` from `origin` account into treasury.
        ///
        /// Only accounts which are associated to an identity can make a donation to treasury.
        #[weight = <T as Trait>::WeightInfo::reimbursement()]
        pub fn reimbursement(origin, amount: BalanceOf<T>) {
            let identity::PermissionedCallOriginData {
                sender,
                primary_did,
                ..
            } = Identity::<T>::ensure_origin_call_permissions(origin)?;

            // Not checking the cdd for the treasury account as it is assumed
            // that treasury account posses a valid CDD check during the genesis phase
            T::Currency::transfer(
                &sender,
                &Self::account_id(),
                amount,
                ExistenceRequirement::AllowDeath,
            )?;

            Self::deposit_event(RawEvent::TreasuryReimbursement(primary_did, amount));
        }
    }
}

impl<T: Trait> Module<T> {
    /// The account ID of the treasury pot.
    ///
    /// This actually does computation. If you need to keep using it, then make sure you cache the
    /// value and only call this once.
    pub fn account_id() -> T::AccountId {
        TREASURY_MODULE_ID.into_account()
    }

    pub fn unsafe_disbursement(target: IdentityId, amount: BalanceOf<T>) {
        let _ = T::Currency::withdraw(
            &Self::account_id(),
            amount,
            WithdrawReason::Transfer.into(),
            ExistenceRequirement::AllowDeath,
        );
        let primary_key = Identity::<T>::did_records(target).primary_key;
        let _ = T::Currency::deposit_into_existing(&primary_key, amount);
        let current_did = Context::current_identity::<Identity<T>>().unwrap_or(GC_DID);
        Self::deposit_event(RawEvent::TreasuryDisbursement(current_did, target, amount));
    }

    fn balance() -> BalanceOf<T> {
        T::Currency::free_balance(&Self::account_id())
    }
}

impl<T: Trait> TreasuryTrait<BalanceOf<T>> for Module<T> {
    #[inline]
    fn disbursement(target: IdentityId, amount: BalanceOf<T>) {
        Self::unsafe_disbursement(target, amount);
    }

    #[inline]
    fn balance() -> BalanceOf<T> {
        Self::balance()
    }
}

/// That trait implementation is needed to receive a portion of the fees from transactions.
impl<T: Trait> OnUnbalanced<NegativeImbalanceOf<T>> for Module<T> {
    fn on_nonzero_unbalanced(amount: NegativeImbalanceOf<T>) {
        let numeric_amount = amount.peek();

        let _ = T::Currency::resolve_creating(&Self::account_id(), amount);
        let current_did = Context::current_identity::<Identity<T>>().unwrap_or_default();
        Self::deposit_event(RawEvent::TreasuryReimbursement(current_did, numeric_amount));
    }
}
