// Copyright 2017-2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

// Modified by Polymath Inc - 16th March 2020
// Implement `BlockRewardsReserveCurrency` trait in the balances module.
// Remove migration functionality from the balances module as Polymesh doesn't needed
// any migration data structure.

//! # Balances Module
//!
//! The Balances module provides functionality for handling accounts and balances.
//!
//! - [`balances::Trait`](./trait.Trait.html)
//! - [`Call`](./enum.Call.html)
//! - [`Module`](./struct.Module.html)
//!
//! ## Overview
//!
//! This is a modified implementation of substrate's balances FRAME.
//! The modifications made are as follows:
//!
//! - To curb front running, sending a tip along with your transaction is now prohibited.
//! - Added From<u128> trait to Balances type.
//! - Removed existential amount requirement to prevent a replay attack scenario.
//! - Added block rewards reserve that subsidize minting for block rewards.
//! - Added CDD check for POLYX recipients.
//! - Added ability to attach a memo with a transfer.
//! - Added ability to burn your tokens.
//!
//! The Original Balances module provides functions for:
//!
//! - Getting and setting free balances.
//! - Retrieving total, reserved and unreserved balances.
//! - Repatriating a reserved balance to a beneficiary account that exists.
//! - Transferring a balance between accounts (when not reserved).
//! - Slashing an account balance.
//! - Account creation and removal.
//! - Managing total issuance.
//! - Setting and managing locks.
//!
//! ### Terminology
//!
//! - **Total Issuance:** The total number of units in existence in a system.
//! - **Reaping an account:** The act of removing an account by resetting its nonce. Happens after its balance is set
//! to zero.
//! - **Free Balance:** The portion of a balance that is not reserved. The free balance is the only balance that matters
//! for most operations.
//! - **Reserved Balance:** Reserved balance still belongs to the account holder, but is suspended. Reserved balance
//! can still be slashed, but only after all the free balance has been slashed.
//! - **Imbalance:** A condition when some funds were credited or debited without equal and opposite accounting
//! (i.e. a difference between total issuance and account balances). Functions that result in an imbalance will
//! return an object of the `Imbalance` trait that can be managed within your runtime logic. (If an imbalance is
//! simply dropped, it should automatically maintain any book-keeping such as total issuance.)
//! - **Lock:** A freeze on a specified amount of an account's free balance until a specified block number. Multiple
//! locks always operate over the same funds, so they "overlay" rather than "stack".
//! - **Vesting:** Similar to a lock, this is another, but independent, liquidity restriction that reduces linearly
//! over time.
//!
//! ### Implementations
//!
//! The Balances module provides implementations for the following traits. If these traits provide the functionality
//! that you need, then you can avoid coupling with the Balances module.
//!
//! - [`Currency`](../frame_support/traits/trait.Currency.html): Functions for dealing with a
//! fungible assets system.
//! - [`ReservableCurrency`](../frame_support/traits/trait.ReservableCurrency.html):
//! Functions for dealing with assets that can be reserved from an account.
//! - [`LockableCurrency`](../frame_support/traits/trait.LockableCurrency.html): Functions for
//! dealing with accounts that allow liquidity restrictions.
//! - [`Imbalance`](../frame_support/traits/trait.Imbalance.html): Functions for handling
//! imbalances between total issuance in the system and account balances. Must be used when a function
//! creates new funds (e.g. a reward) or destroys some funds (e.g. a system fee).
//! - [`IsDeadAccount`](../srml_system/trait.IsDeadAccount.html): Determiner to say whether a
//! given account is unused.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `transfer` - Transfer some liquid free balance to another account.
//! - `transfer_with_memo` - Transfer some liquid free balance to another account alon with a memo.
//! - `set_balance` - Set the balances of a given account. The origin of this call must be root.
//! - `deposit_block_reward_reserve_balance` - Transfer some liquid free balance to block rewards reserve.
//! - `force_transfer` - Force transfer some balance from one account to another. The origin of this call must be root.
//! - `burn_account_balance` - Burn some liquid free balance.

//! ### Public Functions
//!
//! - `free_balance` - Get the free balance of an account.
//! - `usable_balance` - Get the balance of an account that can be used for transfers, reservations, or any other non-locking, non-transaction-fee activity.
//! - `usable_balance_for_fees` - Get the balance of an account that can be used for paying transaction fees (not tipping, or any other kind of fees, though).
//! - `reserved_balance` - Get the reserved balance of an account.
//! ## Usage
//!
//! The following examples show how to use the Balances module in your custom module.
//!
//! ### Examples from the FRAME
//!
//! The Contract module uses the `Currency` trait to handle gas payment, and its types inherit from `Currency`:
//!
//! ```
//! use frame_support::traits::Currency;
//! # pub trait Trait: frame_system::Trait {
//! # type Currency: Currency<Self::AccountId>;
//! # }
//!
//! pub type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;
//! pub type NegativeImbalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::NegativeImbalance;
//!
//! # fn main() {}
//! ```
//!
//! The Staking module uses the `LockableCurrency` trait to lock a stash account's funds:
//!
//! ```
//! use frame_support::traits::{WithdrawReasons, LockableCurrency};
//! use sp_runtime::traits::Bounded;
//! pub trait Trait: frame_system::Trait {
//! type Currency: LockableCurrency<Self::AccountId, Moment=Self::BlockNumber>;
//! }
//! # struct StakingLedger<T: Trait> {
//!     # stash: <T as frame_system::Trait>::AccountId,
//!     # total: <<T as Trait>::Currency as frame_support::traits::Currency<<T as frame_system::Trait>::AccountId>>::Balance,
//!     # phantom: std::marker::PhantomData<T>,
//! # }
//! # const STAKING_ID: [u8; 8] = *b"staking ";
//!
//! fn update_ledger<T: Trait>(
//!     controller: &T::AccountId,
//!     ledger: &StakingLedger<T>
//! ) {
//!     T::Currency::set_lock(
//!         STAKING_ID,
//!         &ledger.stash,
//!         ledger.total,
//!         WithdrawReasons::all()
//!     );
//!     // <Ledger<T>>::insert(controller, ledger); // Commented out as we don't have access to Staking's storage here.
//! }
//! # fn main() {}
//! ```
//!
//! ## Genesis config
//!
//! The Balances module depends on the [`GenesisConfig`](./struct.GenesisConfig.html).
//!
//! ## Assumptions
//!
//! * Total issued balanced of all accounts should be less than `Trait::Balance::max_value()`.

// TODO: Benchmark modified extrinsics. Currently using Substrate values based on non-modified code.
// Specifically CDD checks should be considered!

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

use codec::{Decode, Encode};
use frame_support::traits::Get;
use frame_support::{
    decl_error, decl_module, decl_storage, ensure,
    traits::{
        BalanceStatus as Status, Currency, ExistenceRequirement, Imbalance, IsDeadAccount,
        LockIdentifier, LockableCurrency, ReservableCurrency, SignedImbalance, StoredMap,
        WithdrawReason, WithdrawReasons,
    },
    StorageValue,
};
use frame_system::{self as system, ensure_root, ensure_signed};
pub use polymesh_common_utilities::traits::balances::WeightInfo;
use polymesh_common_utilities::{
    traits::{
        balances::{AccountData, BalancesTrait, CheckCdd, Memo, RawEvent, Reasons},
        identity::IdentityFnTrait,
        NegativeImbalance, PositiveImbalance,
    },
    Context, SystematicIssuers, GC_DID,
};
use polymesh_primitives::traits::BlockRewardsReserveCurrency;
use sp_runtime::{
    traits::{
        AccountIdConversion, Bounded, CheckedAdd, CheckedSub, MaybeSerializeDeserialize,
        Saturating, StaticLookup, Zero,
    },
    DispatchError, DispatchResult, RuntimeDebug,
};
use sp_std::{cmp, convert::Infallible, fmt::Debug, mem, prelude::*, result};

pub use polymesh_common_utilities::traits::balances::{LockableCurrencyExt, Trait};

pub type Event<T> = polymesh_common_utilities::traits::balances::Event<T>;
type CallPermissions<T> = pallet_permissions::Module<T>;

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// Account liquidity restrictions prevent withdrawal
        LiquidityRestrictions,
        /// Got an overflow after adding
        Overflow,
        /// Balance too low to send value
        InsufficientBalance,
        /// Value too low to create account due to existential deposit
        ExistentialDeposit,
        /// Receiver does not have a valid CDD
        ReceiverCddMissing,
    }
}

/// A single lock on a balance. There can be many of these on an account and they "overlap", so the
/// same balance is frozen by multiple locks.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct BalanceLock<Balance> {
    /// An identifier for this lock. Only one lock may be in existence for each identifier.
    pub id: LockIdentifier,
    /// The amount which the free balance may not drop below when this lock is in effect.
    pub amount: Balance,
    /// If true, then the lock remains in effect even for payment of transaction fees.
    pub reasons: Reasons,
}

decl_storage! {
    trait Store for Module<T: Trait> as Balances {
        /// The total units issued in the system.
        pub TotalIssuance get(fn total_issuance) build(|config: &GenesisConfig<T>| {
            let f = |u: T::Balance, &v| u + v;
            config.balances
                .iter()
                .map(|(_, v)| v)
                .fold(Zero::zero(), f)
        }): T::Balance;


        /// Any liquidity locks on some account balances.
        /// NOTE: Should only be accessed when setting, changing and freeing a lock.
        pub Locks get(fn locks): map hasher(blake2_128_concat) T::AccountId => Vec<BalanceLock<T::Balance>>;

    }
    add_extra_genesis {
        /// Account balances at genesis.
        config(balances): Vec<(T::AccountId, T::Balance)>;
        build(|config: &GenesisConfig<T>| {
            for (who, free) in &config.balances {
                T::AccountStore::insert(who, AccountData { free: *free, .. Default::default() });
            }
        });
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        // Polymesh modified code. Existential Deposit requirements are zero in Polymesh.
        /// This is no longer needed but kept for compatibility reasons
        /// The minimum amount required to keep an account open.
        const ExistentialDeposit: T::Balance = 0u32.into();

        fn deposit_event() = default;

        /// Transfer some liquid free balance to another account.
        ///
        /// `transfer` will set the `FreeBalance` of the sender and receiver.
        /// It will decrease the total issuance of the system by the `TransferFee`.
        ///
        /// The dispatch origin for this call must be `Signed` by the transactor.
        ///
        /// # <weight>
        /// - Dependent on arguments but not critical, given proper implementations for
        ///   input config types. See related functions below.
        /// - It contains a limited number of reads and writes internally and no complex computation.
        ///
        /// Related functions:
        ///
        ///   - `ensure_can_withdraw` is always called internally but has a bounded complexity.
        ///   - Transferring balances to accounts that did not exist before will cause
        ///      `T::OnNewAccount::on_new_account` to be called.
        /// ---------------------------------
        /// - Base Weight: 73.64 µs, worst case scenario (account created, account removed)
        /// - DB Weight: 1 Read and 1 Write to destination account.
        /// - Origin account is already in memory, so no DB operations for them.
        /// # </weight>
        #[weight = <T as Trait>::WeightInfo::transfer()]
        pub fn transfer(
            origin,
            dest: <T::Lookup as StaticLookup>::Source,
            #[compact] value: T::Balance
        ) {
            let transactor = ensure_signed(origin)?;
            let dest = T::Lookup::lookup(dest)?;
            // Polymesh modified code. CDD is checked before processing transfer.
            Self::safe_transfer_core(&transactor, &dest, value, None, ExistenceRequirement::AllowDeath)?;
        }

        // Polymesh modified code. New function to transfer with a memo.
        /// Transfer the native currency with the help of identifier string
        /// this functionality can help to differentiate the transfers.
        ///
        /// # <weight>
        /// - Base Weight: 73.64 µs, worst case scenario (account created, account removed)
        /// - DB Weight: 1 Read and 1 Write to destination account.
        /// - Origin account is already in memory, so no DB operations for them.
        /// # </weight>
        #[weight = <T as Trait>::WeightInfo::transfer_with_memo()]
        pub fn transfer_with_memo(
            origin,
            dest: <T::Lookup as StaticLookup>::Source,
            #[compact] value: T::Balance,
            memo: Option<Memo>
        ) {
            let transactor = ensure_signed(origin)?;
            let dest = T::Lookup::lookup(dest)?;
            Self::safe_transfer_core(&transactor, &dest, value, memo, ExistenceRequirement::AllowDeath)?;
        }

        // Polymesh specific change. New function to transfer balance to BRR.
        /// Move some POLYX from balance of self to balance of BRR.
        #[weight = <T as Trait>::WeightInfo::deposit_block_reward_reserve_balance()]
        pub fn deposit_block_reward_reserve_balance(
            origin,
            #[compact] value: T::Balance
        ) {
            let transactor = ensure_signed(origin)?;
            CallPermissions::<T>::ensure_call_permissions(&transactor)?;
            let dest = Self::block_rewards_reserve();
            Self::transfer_core(&transactor, &dest, value, None, ExistenceRequirement::AllowDeath)?;
        }

        /// Set the balances of a given account.
        ///
        /// This will alter `FreeBalance` and `ReservedBalance` in storage. it will
        /// also decrease the total issuance of the system (`TotalIssuance`).
        ///
        /// The dispatch origin for this call is `root`.
        #[weight = <T as Trait>::WeightInfo::set_balance()]
        fn set_balance(
            origin,
            who: <T::Lookup as StaticLookup>::Source,
            #[compact] new_free: T::Balance,
            #[compact] new_reserved: T::Balance
        ) {
            ensure_root(origin)?;
            let who = T::Lookup::lookup(who)?;
            let caller_id = Context::current_identity_or::<T::IdentityFn>(&who)
                .unwrap_or(GC_DID);

            let (free, reserved) = Self::mutate_account(&who, |account| {
                if new_free > account.free {
                    mem::drop(PositiveImbalance::<T>::new(new_free - account.free));
                } else if new_free < account.free {
                    mem::drop(NegativeImbalance::<T>::new(account.free - new_free));
                }

                if new_reserved > account.reserved {
                    mem::drop(PositiveImbalance::<T>::new(new_reserved - account.reserved));
                } else if new_reserved < account.reserved {
                    mem::drop(NegativeImbalance::<T>::new(account.reserved - new_reserved));
                }

                account.free = new_free;
                account.reserved = new_reserved;

                (account.free, account.reserved)
            });
            Self::deposit_event(RawEvent::BalanceSet(caller_id, who, free, reserved));
        }

        /// Exactly as `transfer`, except the origin must be root and the source account may be
        /// specified.
        ///
        /// # <weight>
        /// - Same as transfer, but additional read and write because the source account is
        ///   not assumed to be in the overlay.
        /// # </weight>
        #[weight = <T as Trait>::WeightInfo::force_transfer()]
        pub fn force_transfer(
            origin,
            source: <T::Lookup as StaticLookup>::Source,
            dest: <T::Lookup as StaticLookup>::Source,
            #[compact] value: T::Balance
        ) {
            ensure_root(origin)?;
            let source = T::Lookup::lookup(source)?;
            let dest = T::Lookup::lookup(dest)?;
            Self::transfer_core(&source, &dest, value, None, ExistenceRequirement::AllowDeath)?;
        }

        // Polymesh modified code. New dispatchable function that anyone can call to burn their balance.
        /// Burns the given amount of tokens from the caller's free, unlocked balance.
        #[weight = <T as Trait>::WeightInfo::burn_account_balance()]
        pub fn burn_account_balance(origin, amount: T::Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            CallPermissions::<T>::ensure_call_permissions(&who)?;
            let caller_id = Context::current_identity_or::<T::IdentityFn>(&who)?;
            // Withdraw the account balance and burn the resulting imbalance by dropping it.
            let _ = <Self as Currency<T::AccountId>>::withdraw(
                &who,
                amount,
                // There is no specific "burn" reason in Substrate. However, if the caller is
                // allowed to transfer then they should also be allowed to burn.
                WithdrawReason::Transfer.into(),
                ExistenceRequirement::AllowDeath,
            )?;
            Self::deposit_event(RawEvent::AccountBalanceBurned(caller_id, who, amount));
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    // PRIVATE MUTABLES

    /// Get the free balance of an account.
    pub fn free_balance(who: impl sp_std::borrow::Borrow<T::AccountId>) -> T::Balance {
        Self::account(who.borrow()).free
    }

    /// Get the balance of an account that can be used for transfers, reservations, or any other
    /// non-locking, non-transaction-fee activity. Will be at most `free_balance`.
    pub fn usable_balance(who: impl sp_std::borrow::Borrow<T::AccountId>) -> T::Balance {
        Self::account(who.borrow()).usable(Reasons::Misc)
    }

    /// Get the balance of an account that can be used for paying transaction fees (not tipping,
    /// or any other kind of fees, though). Will be at most `free_balance`.
    pub fn usable_balance_for_fees(who: impl sp_std::borrow::Borrow<T::AccountId>) -> T::Balance {
        Self::account(who.borrow()).usable(Reasons::Fee)
    }

    /// Get the reserved balance of an account.
    pub fn reserved_balance(who: impl sp_std::borrow::Borrow<T::AccountId>) -> T::Balance {
        Self::account(who.borrow()).reserved
    }

    pub fn block_rewards_reserve() -> T::AccountId {
        SystematicIssuers::BlockRewardReserve
            .as_module_id()
            .into_account()
    }

    /// Get both the free and reserved balances of an account.
    fn account(who: &T::AccountId) -> AccountData<T::Balance> {
        T::AccountStore::get(&who)
    }

    /// Places the `free` and `reserved` parts of `new` into `account`. Also does any steps needed
    /// after mutating an account. This includes DustRemoval unbalancing, in the case than the `new`
    /// account's total balance is non-zero but below ED.
    ///
    /// Returns the final free balance, iff the account was previously of total balance zero, known
    /// as its "endowment".
    fn post_mutation(
        _who: &T::AccountId,
        new: AccountData<T::Balance>,
    ) -> Option<AccountData<T::Balance>> {
        // Polymesh modified code. Removed Existential Deposit logic
        Some(new)
    }

    /// Mutate an account to some new value, or delete it entirely with `None`.
    ///
    /// NOTE: Doesn't do any preparatory work for creating a new account, so should only be used
    /// when it is known that the account already exists.
    ///
    /// NOTE: LOW-LEVEL: This will not attempt to maintain total issuance. It is expected that
    /// the caller will do this.
    pub fn mutate_account<R>(
        who: &T::AccountId,
        f: impl FnOnce(&mut AccountData<T::Balance>) -> R,
    ) -> R {
        Self::try_mutate_account(who, |a, _| -> Result<R, Infallible> { Ok(f(a)) })
            .expect("Error is infallible; qed")
    }

    /// Mutate an account to some new value, or delete it entirely with `None`.
    /// This will do nothing if the result of `f` is an `Err`.
    ///
    /// NOTE: Doesn't do any preparatory work for creating a new account, so should only be used
    /// when it is known that the account already exists.
    ///
    /// NOTE: LOW-LEVEL: This will not attempt to maintain total issuance. It is expected that
    /// the caller will do this.
    fn try_mutate_account<R, E>(
        who: &T::AccountId,
        f: impl FnOnce(&mut AccountData<T::Balance>, bool) -> Result<R, E>,
    ) -> Result<R, E> {
        T::AccountStore::try_mutate_exists(who, |maybe_account| {
            let is_new = maybe_account.is_none();
            let mut account = maybe_account.take().unwrap_or_default();
            f(&mut account, is_new).map(move |result| {
                let maybe_endowed = if is_new { Some(account.free) } else { None };
                // `post_mutation` always return the same account store
                *maybe_account = Self::post_mutation(who, account);
                (maybe_endowed, result)
            })
        })
        .map(|(maybe_endowed, result)| {
            if let Some(endowed) = maybe_endowed {
                // Polymesh-note: Modified the code in the favour of Polymesh code base
                let who_id = T::IdentityFn::get_identity(who);
                Self::deposit_event(RawEvent::Endowed(who_id, who.clone(), endowed));
            }
            result
        })
    }

    /// Update the account entry for `who`, given the locks.
    fn update_locks(who: &T::AccountId, locks: &[BalanceLock<T::Balance>]) {
        if locks.len() as u32 > T::MaxLocks::get() {
            frame_support::debug::warn!(
                "Warning: A user has more currency locks than expected. \
                A runtime configuration adjustment may be needed."
            );
        }

        Self::mutate_account(who, |b| {
            b.misc_frozen = Zero::zero();
            b.fee_frozen = Zero::zero();
            for l in locks.iter() {
                if l.reasons == Reasons::All || l.reasons == Reasons::Misc {
                    b.misc_frozen = b.misc_frozen.max(l.amount);
                }
                if l.reasons == Reasons::All || l.reasons == Reasons::Fee {
                    b.fee_frozen = b.fee_frozen.max(l.amount);
                }
            }
        });

        let existed = Locks::<T>::contains_key(who);
        if locks.is_empty() {
            Locks::<T>::remove(who);
            if existed {
                // TODO: use Locks::<T, I>::hashed_key
                // https://github.com/paritytech/substrate/issues/4969
                system::Module::<T>::dec_ref(who);
            }
        } else {
            Locks::<T>::insert(who, locks);
            if !existed {
                system::Module::<T>::inc_ref(who);
            }
        }
    }

    // Polymesh modified code. New wrapper function for the transfer_core function that checks for CDD.
    /// Checks CDD and then only performs the transfer
    fn safe_transfer_core(
        transactor: &T::AccountId,
        dest: &T::AccountId,
        value: T::Balance,
        memo: Option<Memo>,
        existence_requirement: ExistenceRequirement,
    ) -> DispatchResult {
        ensure!(
            T::CddChecker::check_key_cdd(dest),
            Error::<T>::ReceiverCddMissing
        );

        Self::transfer_core(transactor, dest, value, memo, existence_requirement)
    }

    /// Common functionality for transfers.
    /// It does not emit any event.
    ///
    /// # Return
    /// On success, It will return the applied feed.
    // Transfer some free balance from `transactor` to `dest`.
    // Is a no-op if value to be transferred is zero or the `transactor` is the same as `dest`.
    fn transfer_core(
        transactor: &T::AccountId,
        dest: &T::AccountId,
        value: T::Balance,
        memo: Option<Memo>,
        _existence_requirement: ExistenceRequirement,
    ) -> DispatchResult {
        if value.is_zero() || transactor == dest {
            return Ok(());
        }

        Self::try_mutate_account(dest, |to_account, _| -> DispatchResult {
            Self::try_mutate_account(transactor, |from_account, _| -> DispatchResult {
                from_account.free = from_account
                    .free
                    .checked_sub(&value)
                    .ok_or(Error::<T>::InsufficientBalance)?;

                // NOTE: total stake being stored in the same type means that this could never overflow
                // but better to be safe than sorry.
                to_account.free = to_account
                    .free
                    .checked_add(&value)
                    .ok_or(Error::<T>::Overflow)?;

                Self::ensure_can_withdraw(
                    transactor,
                    value,
                    WithdrawReason::Transfer.into(),
                    from_account.free,
                )?;

                Ok(())
            })
        })?;

        let transactor_id = T::IdentityFn::get_identity(transactor);
        let dest_id = T::IdentityFn::get_identity(dest);

        Self::deposit_event(RawEvent::Transfer(
            transactor_id,
            transactor.clone(),
            dest_id,
            dest.clone(),
            value,
            memo,
        ));

        Ok(())
    }
}

impl<T> BalancesTrait<T::AccountId, T::Balance, NegativeImbalance<T>> for Module<T>
where
    T: Trait,
{
    fn withdraw(
        who: &T::AccountId,
        value: T::Balance,
        reasons: WithdrawReasons,
        liveness: ExistenceRequirement,
    ) -> sp_std::result::Result<NegativeImbalance<T>, DispatchError> {
        <Self as Currency<T::AccountId>>::withdraw(who, value, reasons, liveness)
    }
}

// Polymesh modified code. Managed BRR related functions.
impl<T: Trait> BlockRewardsReserveCurrency<T::Balance, NegativeImbalance<T>> for Module<T> {
    // Polymesh modified code. Drop behavious modified to reduce BRR balance instead of inflating total supply.
    fn drop_positive_imbalance(mut amount: T::Balance) {
        if amount.is_zero() {
            return;
        }
        let brr = Self::block_rewards_reserve();
        let _ = Self::try_mutate_account(&brr, |account, _| -> DispatchResult {
            if account.free > Zero::zero() {
                let old_brr_free_balance = account.free;
                let new_brr_free_balance = old_brr_free_balance.saturating_sub(amount);
                account.free = new_brr_free_balance;
                // Calculate how much amount to mint that is not available with the Brr
                // eg. amount = 100 and the account.free = 60 then `amount_to_mint` = 40
                amount -= old_brr_free_balance - new_brr_free_balance;
            }
            <TotalIssuance<T>>::mutate(|v| *v = v.saturating_add(amount));
            Ok(())
        });
    }

    fn drop_negative_imbalance(amount: T::Balance) {
        <TotalIssuance<T>>::mutate(|v| *v = v.saturating_sub(amount));
    }

    // Polymesh modified code. Instead of minting new tokens, this function tries to transfer tokens from BRR to the beneficiary.
    // If BRR does not have enough free funds, new tokens are issued.
    fn issue_using_block_rewards_reserve(mut amount: T::Balance) -> NegativeImbalance<T> {
        if amount.is_zero() {
            return NegativeImbalance::zero();
        }
        let brr = Self::block_rewards_reserve();
        Self::try_mutate_account(&brr, |account, _| -> Result<NegativeImbalance<T>, ()> {
            let amount_to_mint = if account.free > Zero::zero() {
                let old_brr_free_balance = account.free;
                let new_brr_free_balance = old_brr_free_balance.saturating_sub(amount);
                account.free = new_brr_free_balance;
                // Calculate how much amount to mint that is not available with the Brr
                // eg. amount = 100 and the account.free = 60 then `amount_to_mint` = 40
                amount - (old_brr_free_balance - new_brr_free_balance)
            } else {
                amount
            };
            <TotalIssuance<T>>::mutate(|issued| {
                *issued = issued.checked_add(&amount_to_mint).unwrap_or_else(|| {
                    amount = T::Balance::max_value() - *issued;
                    T::Balance::max_value()
                })
            });
            Ok(NegativeImbalance::new(amount))
        })
        .unwrap_or_else(|_x| NegativeImbalance::new(Zero::zero()))
    }

    // Polymesh modified code. Returns balance of BRR
    fn block_rewards_reserve_balance() -> T::Balance {
        let brr = Self::block_rewards_reserve();
        <Self as Currency<T::AccountId>>::free_balance(&brr)
    }
}

impl<T: Trait> Currency<T::AccountId> for Module<T>
where
    T::Balance: MaybeSerializeDeserialize + Debug,
{
    type Balance = T::Balance;
    type PositiveImbalance = PositiveImbalance<T>;
    type NegativeImbalance = NegativeImbalance<T>;

    fn total_balance(who: &T::AccountId) -> Self::Balance {
        Self::account(who).total()
    }

    // Check if `value` amount of free balance can be slashed from `who`.
    fn can_slash(who: &T::AccountId, value: Self::Balance) -> bool {
        if value.is_zero() {
            return true;
        }
        Self::free_balance(who) >= value
    }

    fn total_issuance() -> Self::Balance {
        <TotalIssuance<T>>::get()
    }

    fn minimum_balance() -> Self::Balance {
        Zero::zero()
    }

    fn free_balance(who: &T::AccountId) -> Self::Balance {
        Self::account(who).free
    }

    // Burn funds from the total issuance, returning a positive imbalance for the amount burned.
    // Is a no-op if amount to be burned is zero.
    fn burn(mut amount: Self::Balance) -> Self::PositiveImbalance {
        if amount.is_zero() {
            return PositiveImbalance::zero();
        }
        <TotalIssuance<T>>::mutate(|issued| {
            *issued = issued.checked_sub(&amount).unwrap_or_else(|| {
                amount = *issued;
                Zero::zero()
            });
        });
        PositiveImbalance::new(amount)
    }

    // Create new funds into the total issuance, returning a negative imbalance
    // for the amount issued.
    // Is a no-op if amount to be issued it zero.
    fn issue(mut amount: Self::Balance) -> Self::NegativeImbalance {
        if amount.is_zero() {
            return NegativeImbalance::zero();
        }
        <TotalIssuance<T>>::mutate(|issued| {
            *issued = issued.checked_add(&amount).unwrap_or_else(|| {
                amount = Self::Balance::max_value() - *issued;
                Self::Balance::max_value()
            })
        });
        NegativeImbalance::new(amount)
    }

    // Ensure that an account can withdraw from their free balance given any existing withdrawal
    // restrictions like locks and vesting balance.
    // Is a no-op if amount to be withdrawn is zero.
    //
    // # <weight>
    // Despite iterating over a list of locks, they are limited by the number of
    // lock IDs, which means the number of runtime modules that intend to use and create locks.
    // # </weight>
    fn ensure_can_withdraw(
        who: &T::AccountId,
        amount: T::Balance,
        reasons: WithdrawReasons,
        new_balance: T::Balance,
    ) -> DispatchResult {
        if amount.is_zero() {
            return Ok(());
        }
        let min_balance = Self::account(who).frozen(reasons.into());
        ensure!(
            new_balance >= min_balance,
            Error::<T>::LiquidityRestrictions
        );
        Ok(())
    }

    // Important-Note - Use the transfer carefully as this function is not resilient for the cdd check of receiver.
    // Transfer some free balance from `transactor` to `dest`.
    // Is a no-op if value to be transferred is zero or the `transactor` is the same as `dest`.
    fn transfer(
        transactor: &T::AccountId,
        dest: &T::AccountId,
        value: Self::Balance,
        existence_requirement: ExistenceRequirement,
    ) -> DispatchResult {
        // Calling `transfer_core()` instead of the `safe_transfer_core()` to support the
        // transfer to the smart extensions using the pallet-contracts.
        Self::transfer_core(transactor, dest, value, None, existence_requirement)?;
        Ok(())
    }

    /// Slash a target account `who`, returning the negative imbalance created and any left over
    /// amount that could not be slashed.
    ///
    /// Is a no-op if `value` to be slashed is zero.
    ///
    /// NOTE: `slash()` prefers free balance, but assumes that reserve balance can be drawn
    /// from in extreme circumstances. `can_slash()` should be used prior to `slash()` to avoid having
    /// to draw from reserved funds, however we err on the side of punishment if things are inconsistent
    /// or `can_slash` wasn't used appropriately.
    fn slash(who: &T::AccountId, value: Self::Balance) -> (Self::NegativeImbalance, Self::Balance) {
        if value.is_zero() {
            return (NegativeImbalance::zero(), Zero::zero());
        }

        Self::mutate_account(who, |account| {
            let free_slash = cmp::min(account.free, value);
            account.free -= free_slash;

            let remaining_slash = value - free_slash;
            if !remaining_slash.is_zero() {
                let reserved_slash = cmp::min(account.reserved, remaining_slash);
                account.reserved -= reserved_slash;
                (
                    NegativeImbalance::new(free_slash + reserved_slash),
                    remaining_slash - reserved_slash,
                )
            } else {
                (NegativeImbalance::new(value), Zero::zero())
            }
        })
    }

    /// Deposit some `value` into the free balance of an existing target account `who`.
    ///
    /// Is a no-op if the `value` to be deposited is zero.
    fn deposit_into_existing(
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Result<Self::PositiveImbalance, DispatchError> {
        if value.is_zero() {
            return Ok(PositiveImbalance::zero());
        }

        Self::try_mutate_account(
            who,
            |account, _| -> Result<Self::PositiveImbalance, DispatchError> {
                // Polymesh modified code. Removed existential deposit requirements.
                // This function is now logically equivalent to `deposit_creating`.

                account.free = account
                    .free
                    .checked_add(&value)
                    .ok_or(Error::<T>::Overflow)?;
                Ok(PositiveImbalance::new(value))
            },
        )
    }

    /// Deposit some `value` into the free balance of `who`, possibly creating a new account.
    ///
    /// This function is a no-op if:
    /// - the `value` to be deposited is zero; or
    /// - `value` is so large it would cause the balance of `who` to overflow.
    fn deposit_creating(who: &T::AccountId, value: Self::Balance) -> Self::PositiveImbalance {
        if value.is_zero() {
            return Self::PositiveImbalance::zero();
        }

        Self::try_mutate_account(
            who,
            |account, _| -> Result<Self::PositiveImbalance, Self::PositiveImbalance> {
                // Polymesh modified code. Removed existential deposit requirements.
                // This function is now logically equivalent to `deposit_into_existing`.
                account.free = account
                    .free
                    .checked_add(&value)
                    .ok_or_else(Self::PositiveImbalance::zero)?;

                Ok(PositiveImbalance::new(value))
            },
        )
        .unwrap_or_else(|x| x)
    }

    /// Withdraw some free balance from an account
    ///
    /// Is a no-op if value to be withdrawn is zero.
    fn withdraw(
        who: &T::AccountId,
        value: Self::Balance,
        reasons: WithdrawReasons,
        _liveness: ExistenceRequirement,
    ) -> result::Result<Self::NegativeImbalance, DispatchError> {
        if value.is_zero() {
            return Ok(NegativeImbalance::zero());
        }

        Self::try_mutate_account(
            who,
            |account, _| -> Result<Self::NegativeImbalance, DispatchError> {
                let new_free_account = account
                    .free
                    .checked_sub(&value)
                    .ok_or(Error::<T>::InsufficientBalance)?;

                // Polymesh modified code. Removed existential deposit requirements.

                Self::ensure_can_withdraw(who, value, reasons, new_free_account)?;

                account.free = new_free_account;

                Ok(NegativeImbalance::new(value))
            },
        )
    }

    /// Force the new free balance of a target account `who` to some new value `balance`.
    fn make_free_balance_be(
        who: &T::AccountId,
        value: Self::Balance,
    ) -> SignedImbalance<Self::Balance, Self::PositiveImbalance> {
        Self::try_mutate_account(
            who,
            |account, _| -> Result<SignedImbalance<Self::Balance, Self::PositiveImbalance>, ()> {
                // Polymesh modified code. Removed existential deposit requirements.

                let imbalance = if account.free <= value {
                    SignedImbalance::Positive(PositiveImbalance::new(value - account.free))
                } else {
                    SignedImbalance::Negative(NegativeImbalance::new(account.free - value))
                };
                account.free = value;
                Ok(imbalance)
            },
        )
        .unwrap_or_else(|_| SignedImbalance::Positive(Self::PositiveImbalance::zero()))
    }
}

impl<T: Trait> ReservableCurrency<T::AccountId> for Module<T>
where
    T::Balance: MaybeSerializeDeserialize + Debug,
{
    /// Check if `who` can reserve `value` from their free balance.
    ///
    /// Always `true` if value to be reserved is zero.
    fn can_reserve(who: &T::AccountId, value: Self::Balance) -> bool {
        if value.is_zero() {
            return true;
        }
        Self::account(who)
            .free
            .checked_sub(&value)
            .map_or(false, |new_balance| {
                Self::ensure_can_withdraw(who, value, WithdrawReason::Reserve.into(), new_balance)
                    .is_ok()
            })
    }

    fn reserved_balance(who: &T::AccountId) -> Self::Balance {
        Self::account(who).reserved
    }

    /// Move `value` from the free balance from `who` to their reserved balance.
    ///
    /// Is a no-op if value to be reserved is zero.
    fn reserve(who: &T::AccountId, value: Self::Balance) -> DispatchResult {
        if value.is_zero() {
            return Ok(());
        }

        Self::try_mutate_account(who, |account, _| -> DispatchResult {
            account.free = account
                .free
                .checked_sub(&value)
                .ok_or(Error::<T>::InsufficientBalance)?;
            account.reserved = account
                .reserved
                .checked_add(&value)
                .ok_or(Error::<T>::Overflow)?;
            Self::ensure_can_withdraw(who, value, WithdrawReason::Reserve.into(), account.free)
        })?;
        Self::deposit_event(RawEvent::Reserved(who.clone(), value));
        Ok(())
    }

    /// Unreserve some funds, returning any amount that was unable to be unreserved.
    ///
    /// Is a no-op if the value to be unreserved is zero.
    fn unreserve(who: &T::AccountId, value: Self::Balance) -> Self::Balance {
        if value.is_zero() {
            return Zero::zero();
        }

        let actual = Self::mutate_account(who, |account| {
            let actual = cmp::min(account.reserved, value);
            account.reserved -= actual;
            // defensive only: this can never fail since total issuance which is at least free+reserved
            // fits into the same data type.
            account.free = account.free.saturating_add(actual);
            actual
        });

        Self::deposit_event(RawEvent::Unreserved(who.clone(), actual));
        value - actual
    }

    /// Slash from reserved balance, returning the negative imbalance created,
    /// and any amount that was unable to be slashed.
    ///
    /// Is a no-op if the value to be slashed is zero.
    fn slash_reserved(
        who: &T::AccountId,
        value: Self::Balance,
    ) -> (Self::NegativeImbalance, Self::Balance) {
        if value.is_zero() {
            return (NegativeImbalance::zero(), Zero::zero());
        }

        Self::mutate_account(who, |account| {
            // underflow should never happen, but it if does, there's nothing to be done here.
            let actual = cmp::min(account.reserved, value);
            account.reserved -= actual;
            (NegativeImbalance::new(actual), value - actual)
        })
    }

    /// Move the reserved balance of one account into the balance of another, according to `status`.
    ///
    /// Is a no-op if:
    /// - the value to be moved is zero; or
    /// - the `slashed` id equal to `beneficiary` and the `status` is `Reserved`.
    fn repatriate_reserved(
        slashed: &T::AccountId,
        beneficiary: &T::AccountId,
        value: Self::Balance,
        status: Status,
    ) -> Result<Self::Balance, DispatchError> {
        if value.is_zero() {
            return Ok(Zero::zero());
        }

        if slashed == beneficiary {
            return match status {
                Status::Free => Ok(Self::unreserve(slashed, value)),
                Status::Reserved => Ok(value.saturating_sub(Self::reserved_balance(slashed))),
            };
        }

        let actual = Self::try_mutate_account(
            beneficiary,
            |to_account, _| -> Result<Self::Balance, DispatchError> {
                // Polymesh modified code. Removed existential deposit requirements.
                Self::try_mutate_account(
                    slashed,
                    |from_account, _| -> Result<Self::Balance, DispatchError> {
                        let actual = cmp::min(from_account.reserved, value);
                        match status {
                            Status::Free => {
                                to_account.free = to_account
                                    .free
                                    .checked_add(&actual)
                                    .ok_or(Error::<T>::Overflow)?
                            }
                            Status::Reserved => {
                                to_account.reserved = to_account
                                    .reserved
                                    .checked_add(&actual)
                                    .ok_or(Error::<T>::Overflow)?
                            }
                        }
                        from_account.reserved -= actual;
                        Ok(actual)
                    },
                )
            },
        )?;
        Self::deposit_event(RawEvent::ReserveRepatriated(
            slashed.clone(),
            beneficiary.clone(),
            actual,
            status,
        ));
        Ok(value - actual)
    }
}

impl<T: Trait> LockableCurrency<T::AccountId> for Module<T>
where
    T::Balance: MaybeSerializeDeserialize + Debug,
{
    type Moment = T::BlockNumber;

    type MaxLocks = T::MaxLocks;

    // Polymesh-note: The implementations below differ from substrate in terms
    // of performance (ours uses in-place modification), but are functionally equivalent.

    // Set a lock on the balance of `who`.
    // Is a no-op if lock amount is zero or `reasons` `is_none()`.
    fn set_lock(
        id: LockIdentifier,
        who: &T::AccountId,
        amount: T::Balance,
        reasons: WithdrawReasons,
    ) {
        if amount.is_zero() || reasons.is_none() {
            return;
        }
        let new_lock = BalanceLock {
            id,
            amount,
            reasons: reasons.into(),
        };
        let mut locks = Self::locks(who);
        if let Some(pos) = locks.iter().position(|l| l.id == id) {
            locks[pos] = new_lock;
        } else {
            locks.push(new_lock);
        }
        Self::update_locks(who, &locks[..]);
    }

    // Extend a lock on the balance of `who`.
    // Is a no-op if lock amount is zero or `reasons` `is_none()`.
    fn extend_lock(
        id: LockIdentifier,
        who: &T::AccountId,
        amount: T::Balance,
        reasons: WithdrawReasons,
    ) {
        if amount.is_zero() || reasons.is_none() {
            return;
        }
        let reasons = reasons.into();
        let mut locks = Self::locks(who);
        if let Some(pos) = locks.iter().position(|l| l.id == id) {
            let slot = &mut locks[pos];
            slot.amount = slot.amount.max(amount);
            slot.reasons = slot.reasons | reasons;
        } else {
            locks.push(BalanceLock {
                id,
                amount,
                reasons,
            });
        }
        Self::update_locks(who, &locks[..]);
    }

    fn remove_lock(id: LockIdentifier, who: &T::AccountId) {
        let mut locks = Self::locks(who);
        locks.retain(|l| l.id != id);
        Self::update_locks(who, &locks[..]);
    }
}

impl<T: Trait> LockableCurrencyExt<T::AccountId> for Module<T>
where
    T::Balance: MaybeSerializeDeserialize + Debug,
{
    fn reduce_lock(id: LockIdentifier, who: &T::AccountId, amount: T::Balance) -> DispatchResult {
        if amount.is_zero() {
            return Ok(());
        }
        let mut locks = Self::locks(who);
        locks
            .iter()
            .position(|l| l.id == id)
            .and_then(|p| {
                let slot = &mut locks[p].amount;
                let new = slot.checked_sub(&amount).map(|n| *slot = n);
                if slot.is_zero() {
                    locks.swap_remove(p);
                }
                new
            })
            .ok_or(Error::<T>::InsufficientBalance)?;
        Self::update_locks(who, &locks[..]);
        Ok(())
    }

    fn increase_lock(
        id: LockIdentifier,
        who: &T::AccountId,
        amount: T::Balance,
        reasons: WithdrawReasons,
        check_sum: impl FnOnce(T::Balance) -> DispatchResult,
    ) -> DispatchResult {
        if amount.is_zero() || reasons.is_none() {
            return Ok(());
        }
        let reasons = reasons.into();
        let mut locks = Self::locks(who);
        check_sum(if let Some(pos) = locks.iter().position(|l| l.id == id) {
            let slot = &mut locks[pos];
            slot.amount = slot
                .amount
                .checked_add(&amount)
                .ok_or(Error::<T>::Overflow)?;
            slot.reasons = slot.reasons | reasons;
            slot.amount
        } else {
            locks.push(BalanceLock {
                id,
                amount,
                reasons,
            });
            amount
        })?;
        Self::update_locks(who, &locks[..]);
        Ok(())
    }
}

impl<T: Trait> IsDeadAccount<T::AccountId> for Module<T>
where
    T::Balance: MaybeSerializeDeserialize + Debug,
{
    fn is_dead_account(who: &T::AccountId) -> bool {
        // this should always be exactly equivalent to `Self::account(who).total().is_zero()`
        !T::AccountStore::is_explicit(who)
    }
}
