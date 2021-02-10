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

use crate::traits::{identity::Trait as IdentityTrait, CommonTrait, NegativeImbalance};
use codec::{Decode, Encode};
use frame_support::{
    decl_event,
    dispatch::{DispatchError, DispatchResult},
    traits::{
        BalanceStatus as Status, ExistenceRequirement, Get, LockIdentifier, LockableCurrency,
        OnUnbalanced, StoredMap, WithdrawReason, WithdrawReasons,
    },
    weights::Weight,
};
use frame_system::{self as system};
use polymesh_primitives::IdentityId;
use polymesh_primitives_derive::SliceU8StrongTyped;
use sp_runtime::{traits::Saturating, RuntimeDebug};
use sp_std::ops::BitOr;

#[derive(Encode, Default, Decode, Clone, PartialEq, Eq, SliceU8StrongTyped)]
pub struct Memo(pub [u8; 32]);

// POLYMESH-NOTE: Make `AccountData` public to access it from the outside module.
/// All balance information for an account.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct AccountData<Balance> {
    /// Non-reserved part of the balance. There may still be restrictions on this, but it is the
    /// total pool what may in principle be transferred, reserved and used for tipping.
    ///
    /// This is the only balance that matters in terms of most operations on tokens. It
    /// alone is used to determine the balance when in the contract execution environment.
    pub free: Balance,
    /// Balance which is reserved and may not be used at all.
    ///
    /// This can still get slashed, but gets slashed last of all.
    ///
    /// This balance is a 'reserve' balance that other subsystems use in order to set aside tokens
    /// that are still 'owned' by the account holder, but which are suspendable.
    pub reserved: Balance,
    /// The amount that `free` may not drop below when withdrawing for *anything except transaction
    /// fee payment*.
    pub misc_frozen: Balance,
    /// The amount that `free` may not drop below when withdrawing specifically for transaction
    /// fee payment.
    pub fee_frozen: Balance,
}

impl<Balance: Saturating + Copy + Ord> AccountData<Balance> {
    /// How much this account's balance can be reduced for the given `reasons`.
    pub fn usable(&self, reasons: Reasons) -> Balance {
        self.free.saturating_sub(self.frozen(reasons))
    }
    /// The amount that this account's free balance may not be reduced beyond for the given
    /// `reasons`.
    pub fn frozen(&self, reasons: Reasons) -> Balance {
        match reasons {
            Reasons::All => self.misc_frozen.max(self.fee_frozen),
            Reasons::Misc => self.misc_frozen,
            Reasons::Fee => self.fee_frozen,
        }
    }
    /// The total balance in this account including any that is reserved and ignoring any frozen.
    pub fn total(&self) -> Balance {
        self.free.saturating_add(self.reserved)
    }
}

/// Simplified reasons for withdrawing balance.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug)]
pub enum Reasons {
    /// Paying system transaction fees.
    Fee = 0,
    /// Any reason other than paying system transaction fees.
    Misc = 1,
    /// Any reason at all.
    All = 2,
}

impl From<WithdrawReasons> for Reasons {
    fn from(r: WithdrawReasons) -> Reasons {
        if r == WithdrawReasons::from(WithdrawReason::TransactionPayment) {
            Reasons::Fee
        } else if r.contains(WithdrawReason::TransactionPayment) {
            Reasons::All
        } else {
            Reasons::Misc
        }
    }
}

impl BitOr for Reasons {
    type Output = Reasons;
    fn bitor(self, other: Reasons) -> Reasons {
        if self == other {
            return self;
        }
        Reasons::All
    }
}

decl_event!(
    pub enum Event<T> where
    <T as system::Trait>::AccountId,
    <T as CommonTrait>::Balance
    {
         /// An account was created with some free balance. \[did, account, free_balance]
        Endowed(Option<IdentityId>, AccountId, Balance),
        /// Transfer succeeded (from_did, from, to_did, to, value, memo).
        Transfer(Option<IdentityId>, AccountId, Option<IdentityId>, AccountId, Balance, Option<Memo>),
        /// A balance was set by root (did, who, free, reserved).
        BalanceSet(IdentityId, AccountId, Balance, Balance),
        /// The account and the amount of unlocked balance of that account that was burned.
        /// (caller Id, caller account, amount)
        AccountBalanceBurned(IdentityId, AccountId, Balance),
        /// Some balance was reserved (moved from free to reserved). \[who, value]
        Reserved(AccountId, Balance),
        /// Some balance was unreserved (moved from reserved to free). \[who, value]
        Unreserved(AccountId, Balance),
        /// Some balance was moved from the reserve of the first account to the second account.
        /// Final argument indicates the destination balance type.
        /// \[from, to, balance, destination_status]
        ReserveRepatriated(AccountId, AccountId, Balance, Status),
    }
);

pub trait WeightInfo {
    fn transfer() -> Weight;
    fn transfer_with_memo() -> Weight;
    fn deposit_block_reward_reserve_balance() -> Weight;
    fn set_balance() -> Weight;
    fn force_transfer() -> Weight;
    fn burn_account_balance() -> Weight;
}

pub trait Trait: IdentityTrait {
    /// The means of storing the balances of an account.
    type AccountStore: StoredMap<Self::AccountId, AccountData<Self::Balance>>;

    /// Handler for the unbalanced reduction when removing a dust account.
    type DustRemoval: OnUnbalanced<NegativeImbalance<Self>>;

    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

    /// This type is no longer needed but kept for compatibility reasons.
    /// The minimum amount required to keep an account open.
    type ExistentialDeposit: Get<<Self as CommonTrait>::Balance>;

    /// Used to check if an account is linked to a CDD'd identity
    type CddChecker: CheckCdd<Self::AccountId>;

    /// Weight information for extrinsics in this pallet.
    type WeightInfo: WeightInfo;

    /// The maximum number of locks that should exist on an account.
    /// Not strictly enforced, but used for weight estimation.
    type MaxLocks: Get<u32>;
}

pub trait BalancesTrait<A, B, NI> {
    fn withdraw(
        who: &A,
        value: B,
        reasons: WithdrawReasons,
        _liveness: ExistenceRequirement,
    ) -> sp_std::result::Result<NI, DispatchError>;
}

pub trait CheckCdd<AccountId> {
    fn check_key_cdd(key: &AccountId) -> bool;
    fn get_key_cdd_did(key: &AccountId) -> Option<IdentityId>;
}

/// Additional functionality atop `LockableCurrency` allowing a local,
/// per-id, stacking layer atop the overlay.
pub trait LockableCurrencyExt<AccountId>: LockableCurrency<AccountId> {
    /// Reduce the locked amount under `id` for `who`.
    /// If less than `amount` was locked, then `InsufficientBalance` is raised.
    /// If the whole locked amount is reduced, then the lock is removed.
    fn reduce_lock(id: LockIdentifier, who: &AccountId, amount: Self::Balance) -> DispatchResult;

    /// Increase the locked amount under `id` for `who` or raises `Overflow`.
    /// If there's no lock already, it will be made, unless `amount.is_zero()`.
    /// Before committing to storage, `check_sum` is called with the lock total,
    /// allowing the transaction to be aborted.
    fn increase_lock(
        id: LockIdentifier,
        who: &AccountId,
        amount: Self::Balance,
        reasons: WithdrawReasons,
        check_sum: impl FnOnce(Self::Balance) -> DispatchResult,
    ) -> DispatchResult;
}
