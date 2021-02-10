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

//! # Bridge from Ethereum to Polymesh
//!
//! This module implements a one-way bridge between Polymath Classic on the Ethereum side, and
//! Polymesh native. It mints POLYX on Polymesh in return for permanently locked ERC20 POLY tokens.
//!
//! ## Overview
//!
//! The bridge module provides extrinsics that - when used in conjunction with the sudo or
//! [multisig](../../pallet_multisig/index.html) pallets - allow issuing tokens on Polymesh in
//! response to [bridge transactions](BridgeTx).
//!
//! ### Terminology
//!
//! - **bridge transaction**: an immutable data structure constructed by bridge signers containing a
//! unique nonce, the recipient account, the transaction value and the Ethereum transaction hash.
//!
//! - **bridge transaction status**: any bridge transaction has a unique status which is one of the
//! following:
//!   - **absent**: No such transaction is recorded in the bridge module.
//!   - **pending**: The transaction is pending a valid CDD check after a set amount of blocks.
//!   - **frozen**: The transaction has been frozen by the admin.
//!   - **timelocked**: The transaction has been added to the bridge processing queue and is
//!   currently pending its first execution. During this wait the admin can freeze the transaction.
//!   - **handled**: The transaction has been handled successfully and the tokens have been credited
//!   to the recipient account.
//!
//! - **bridge transaction queue**: a single queue of transactions, each identified with the block
//! number at which the transaction will be retried.
//!
//! - **bridge limit**: The maximum number of bridged POLYX per identity within a set interval of
//! blocks.
//!
//! - **bridge limit exempted**: Identities not constrained by the bridge limit.
//!
//! ### Transaction State Transitions
//!
//! Although the bridge is not implemented as a state machine in the strict sense, the status of a
//! bridge transition can be viewed as its state in the abstract state machine diagram below:
//!
//! ```ignore
//!         +------------+      timelock == 0       +------------+
//!         |            |      happy path          |            |
//!         |   absent   +-------------------------->  handled   |
//!         |            +------------+             |            |
//!         +-----+--^---+   admin    |             +------^-----+
//!               |  |                |                    |
//!               |  |          +-----v------+             |
//! timelock != 0 |  | admin    |            |             |
//! or no CDD or  |  +----------+   frozen   |             | happy path
//! limit reached |             |            |             |
//!               |             +----^-^-----+             |
//!               |                  | |                   |
//!         +-----v------+   admin   | |   admin    +------+-----+
//!         |            +-----------+ +------------+            <-----+
//!         | timelocked +-------------------------->  pending   |     |retry
//!         |            |    timelock expired      |            +-----+
//!         +------------+                          +------------+
//! ```
//!
//! **Absent** is the initial state. **Handled** is the final state. Note that there is a feature
//! allowing the admin to introduce new transactions by freezing them since there is an admin
//! transition from **absent** to **frozen**.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `change_controller`: Changes the controller account as admin.
//! - `change_admin`: Changes the bridge admin key.
//! - `change_timelock`: Changes the timelock period.
//! - `freeze`: Freezes transaction handling in the bridge module if it is not already frozen.
//! - `unfreeze`: Unfreezes transaction handling in the bridge module if it is frozen.
//! - `change_bridge_limit`: Changes the bridge limits.
//! - `change_bridge_exempted`: Changes the bridge limit exempted.
//! - `force_handle_bridge_tx`: Forces handling a transaction by bypassing the bridge limit and
//! timelock.
//! - `batch_propose_bridge_tx`: Proposes a vector of bridge transactions.
//! - `propose_bridge_tx`: Proposes a bridge transaction, which amounts to making a multisig
//! - `handle_bridge_tx`: Handles an approved bridge transaction proposal.
//! - `freeze_txs`: Freezes given bridge transactions.
//! - `unfreeze_txs`: Unfreezes given bridge transactions.

#![cfg_attr(not(feature = "std"), no_std)]
#![feature(const_option)]

use codec::{Decode, Encode};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    storage::StorageDoubleMap,
    traits::{
        schedule::{Anon as ScheduleAnon, DispatchTime, LOWEST_PRIORITY},
        Currency,
    },
    weights::{DispatchClass, Pays, Weight},
};
use frame_system::{self as system, ensure_root, ensure_signed, RawOrigin};
use pallet_balances as balances;
use pallet_identity as identity;
use pallet_multisig as multisig;
use pallet_scheduler as scheduler;
use polymesh_common_utilities::{
    traits::{
        balances::{CheckCdd, Trait as BalancesTrait},
        identity::Trait as IdentityTrait,
        CommonTrait,
    },
    Context, GC_DID,
};
use polymesh_primitives::{storage_migrate_on, storage_migration_ver, IdentityId, Signatory};
use sp_core::H256;
use sp_runtime::traits::{CheckedAdd, One, Zero};
use sp_std::{convert::TryFrom, prelude::*};

type Identity<T> = identity::Module<T>;

pub trait Trait: multisig::Trait + scheduler::Trait + BalancesTrait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type Proposal: From<Call<Self>> + Into<<Self as IdentityTrait>::Proposal>;
    /// Scheduler of timelocked bridge transactions.
    type Scheduler: ScheduleAnon<
        Self::BlockNumber,
        <Self as Trait>::Proposal,
        Self::SchedulerOrigin,
    >;
}

/// The status of a bridge transaction.
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BridgeTxStatus {
    /// No such transaction in the system.
    Absent,
    /// The transaction is missing a CDD or the bridge module is frozen.  The `u8` parameter is the
    /// capped number of times the module tried processing this transaction.  It will be retried
    /// automatically. Anyone can retry these manually.
    Pending(u8),
    /// The transaction is frozen by the admin. It will not be retried automatically.
    Frozen,
    /// The transaction is pending its first execution. These can not be manually triggered by
    /// normal accounts.
    Timelocked,
    /// The transaction has been successfully credited.
    Handled,
}

impl Default for BridgeTxStatus {
    fn default() -> Self {
        BridgeTxStatus::Absent
    }
}

/// A unique lock-and-mint bridge transaction.
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct BridgeTx<Account, Balance> {
    /// A single transaction hash can have multiple locks. This nonce differentiates between them.
    pub nonce: u32,
    /// The recipient account of POLYX on Polymesh.
    pub recipient: Account,
    /// Amount of POLYX tokens to credit.
    pub amount: Balance,
    /// Ethereum token lock transaction hash. It is not used internally in the bridge and is kept
    /// here for compatibility reasons only.
    pub tx_hash: H256,
}

/// Additional details of a bridge transaction.
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct BridgeTxDetail<Balance, BlockNumber> {
    /// Amount of POLYX tokens to credit.
    pub amount: Balance,
    /// Status of the bridge transaction.
    pub status: BridgeTxStatus,
    /// Block number at which this transaction was executed or is planned to be executed.
    pub execution_block: BlockNumber,
    /// Ethereum token lock transaction hash. It is not used internally in the bridge and is kept
    /// here for compatibility reasons only.
    pub tx_hash: H256,
}

/// The status of a handled transaction for reporting purposes.
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandledTxStatus {
    /// The transaction has been successfully handled.
    Success,
    /// Handling the transaction has failed, with the encoding of the error.
    Error(Vec<u8>),
}

impl Default for HandledTxStatus {
    fn default() -> Self {
        HandledTxStatus::Success
    }
}

pub mod weight_for {
    use super::Trait;
    use frame_support::{traits::Get, weights::Weight};

    /// <weight>
    /// * Read operation - 1 for read block no. + 1 for reading bridge txn details.
    /// * Write operation - 1 for updating the bridge tx status.
    /// </weight>
    pub(crate) fn handle_bridge_tx<T: Trait>() -> Weight {
        let db = T::DbWeight::get();
        db.reads_writes(2, 1)
            .saturating_add(700_000_000) // base fee for the handle bridge tx
            .saturating_add(800_000) // base value for issue function
            .saturating_add(db.reads_writes(3, 1)) // read and write for the issue() function
            .saturating_add(db.reads_writes(1, 1)) // read and write for the deposit_creating() function under issue() call
    }

    /// <weight>
    /// * Read operation - 4 where 1 is for reading bridge txn details & 3 for general operations
    /// * Write operation - 2
    /// * Base value - 500_000_000
    /// </weight>
    pub(crate) fn handle_bridge_tx_later<T: Trait>() -> Weight {
        let db = T::DbWeight::get();
        db.reads_writes(4, 2).saturating_add(500_000_000) // base value
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// The bridge controller address is not set.
        ControllerNotSet,
        /// The origin is not the controller or the admin address.
        BadCaller,
        /// The origin is not the admin address.
        BadAdmin,
        /// The recipient DID has no valid CDD.
        NoValidCdd,
        /// The bridge transaction proposal has already been handled and the funds minted.
        ProposalAlreadyHandled,
        /// Unauthorized to perform an operation.
        Unauthorized,
        /// The bridge is already frozen.
        Frozen,
        /// The bridge is not frozen.
        NotFrozen,
        /// The transaction is frozen.
        FrozenTx,
        /// The identity's minted total has reached the bridge limit.
        BridgeLimitReached,
        /// The identity's minted total has overflowed.
        Overflow,
        /// The block interval duration is zero. Cannot divide.
        DivisionByZero,
        /// The transaction is timelocked.
        TimelockedTx,
    }
}

// A value placed in storage that represents the current version of the this storage. This value
// is used by the `on_runtime_upgrade` logic to determine whether we run storage migration logic.
storage_migration_ver!(1);

decl_storage! {
    trait Store for Module<T: Trait> as Bridge {
        /// The multisig account of the bridge controller. The genesis signers accept their
        /// authorizations and are able to get their proposals delivered. The bridge creator
        /// transfers some POLY to their identity.
        Controller get(fn controller) build(|config: &GenesisConfig<T>| {
            use frame_support::debug;
            use polymesh_primitives::Permissions;

            if config.signatures_required > u64::try_from(config.signers.len()).unwrap_or_default()
            {
                panic!("too many signatures required");
            }
            if config.signatures_required == 0 {
                // Default to the empty signer set.
                return Default::default();
            }
            let multisig_id = <multisig::Module<T>>::create_multisig_account(
                config.creator.clone(),
                config.signers.as_slice(),
                config.signatures_required
            ).expect("cannot create the bridge multisig");
            debug::info!("Created bridge multisig {}", multisig_id);
            for signer in &config.signers {
                debug::info!("Accepting bridge signer auth for {:?}", signer);
                let last_auth = <identity::Authorizations<T>>::iter_prefix_values(signer)
                    .next()
                    .expect("cannot find bridge signer auth")
                    .auth_id;
                <multisig::Module<T>>::unsafe_accept_multisig_signer(signer.clone(), last_auth)
                    .expect("cannot accept bridge signer auth");
            }
            let creator_did = Context::current_identity_or::<identity::Module<T>>(&config.creator)
                .expect("bridge creator account has no identity");
            <identity::Module<T>>::unsafe_join_identity(
                creator_did,
                Permissions::default(),
                Signatory::Account(multisig_id.clone())
            ).expect("cannot link the bridge multisig");
            debug::info!("Joined identity {} as signer {}", creator_did, multisig_id);
            multisig_id
        }): T::AccountId;

        /// Details of bridge transactions identified with pairs of the recipient account and the
        /// bridge transaction nonce.
        BridgeTxDetails get(fn bridge_tx_details):
            double_map
                hasher(blake2_128_concat) T::AccountId,
                hasher(blake2_128_concat) u32
            =>
                BridgeTxDetail<T::Balance, T::BlockNumber>;

        /// The admin key.
        Admin get(fn admin) config(): T::AccountId;

        /// Whether or not the bridge operation is frozen.
        Frozen get(fn frozen): bool;

        /// The bridge transaction timelock period, in blocks, since the acceptance of the
        /// transaction proposal during which the admin key can freeze the transaction.
        Timelock get(fn timelock) config(): T::BlockNumber;

        /// The maximum number of bridged POLYX per identity within a set interval of
        /// blocks. Fields: POLYX amount and the block interval duration.
        BridgeLimit get(fn bridge_limit) config(): (T::Balance, T::BlockNumber);

        /// Amount of POLYX bridged by the identity in last block interval. Fields: the bridged
        /// amount and the last interval number.
        PolyxBridged get(fn polyx_bridged): map hasher(identity) IdentityId => (T::Balance, T::BlockNumber);

        /// Identities not constrained by the bridge limit.
        BridgeLimitExempted get(fn bridge_exempted): map hasher(twox_64_concat) IdentityId => bool;

        /// Storage version.
        StorageVersion get(fn storage_version) build(|_| Version::new(1).unwrap()): Version;
    }
    add_extra_genesis {
        /// AccountId of the multisig creator.
        config(creator): T::AccountId;
        /// The set of initial signers from which a multisig address is created at genesis time.
        config(signers): Vec<Signatory<T::AccountId>>;
        /// The number of required signatures in the genesis signer set.
        config(signatures_required): u64;
    }
}

decl_event! {
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
        Balance = <T as CommonTrait>::Balance,
        BlockNumber = <T as frame_system::Trait>::BlockNumber,
    {
        /// Confirmation of a signer set change.
        ControllerChanged(IdentityId, AccountId),
        /// Confirmation of Admin change.
        AdminChanged(IdentityId, AccountId),
        /// Confirmation of default timelock change.
        TimelockChanged(IdentityId, BlockNumber),
        /// Confirmation of POLYX upgrade on Polymesh from POLY tokens on Ethereum
        Bridged(IdentityId, BridgeTx<AccountId, Balance>),
        /// Notification of freezing the bridge.
        Frozen(IdentityId),
        /// Notification of unfreezing the bridge.
        Unfrozen(IdentityId),
        /// Notification of freezing a transaction.
        FrozenTx(IdentityId, BridgeTx<AccountId, Balance>),
        /// Notification of unfreezing a transaction.
        UnfrozenTx(IdentityId, BridgeTx<AccountId, Balance>),
        /// Exemption status of an identity has been updated.
        ExemptedUpdated(IdentityId, IdentityId, bool),
        /// Bridge limit has been updated
        BridgeLimitUpdated(IdentityId, Balance, BlockNumber),
        /// An event emitted after a vector of transactions is handled. The parameter is a vector of
        /// nonces of all processed transactions, each with either the "success" code 0 or its
        /// failure reason (greater than 0).
        TxsHandled(Vec<(u32, HandledTxStatus)>),
        /// Bridge Tx Scheduled
        BridgeTxScheduled(IdentityId, BridgeTx<AccountId, Balance>, BlockNumber),
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: <T as frame_system::Trait>::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        fn on_runtime_upgrade() -> Weight {
            use frame_support::{migration::StorageKeyIterator, Twox64Concat};

            let storage_ver = StorageVersion::get();
            storage_migrate_on!(storage_ver, 1, {
                let now = frame_system::Module::<T>::block_number();

                // Migrate timelocked transactions.
                StorageKeyIterator::<T::BlockNumber, Vec::<BridgeTx<T::AccountId, T::Balance>>, Twox64Concat>::new(b"Bridge", b"TimelockedTxs")
                    .drain()
                    .for_each(|(block_number, txs)| {
                        // Schedule only for future blocks.
                        let block_number = T::BlockNumber::max(block_number, now + One::one());
                        for tx in txs {
                            Self::schedule_call(block_number, tx);
                        }
                    });
            });

            // No need to calculate correct weight for testnet
            0
        }

        /// Changes the controller account as admin.
        #[weight = (300_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn change_controller(origin, controller: T::AccountId) {
            let did = Self::ensure_admin_did(origin)?;
            <Controller<T>>::put(controller.clone());
            Self::deposit_event(RawEvent::ControllerChanged(did, controller));
        }

        /// Changes the bridge admin key.
        #[weight = (300_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn change_admin(origin, admin: T::AccountId) {
            let did = Self::ensure_admin_did(origin)?;
            <Admin<T>>::put(admin.clone());
            Self::deposit_event(RawEvent::AdminChanged(did, admin));
        }

        /// Changes the timelock period.
        #[weight = (300_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn change_timelock(origin, timelock: T::BlockNumber) {
            let did = Self::ensure_admin_did(origin)?;
            <Timelock<T>>::put(timelock);
            Self::deposit_event(RawEvent::TimelockChanged(did, timelock));
        }

        /// Freezes transaction handling in the bridge module if it is not already frozen. When the
        /// bridge is frozen, attempted transactions get postponed instead of getting handled.
        #[weight = (300_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn freeze(origin) {
            let did = Self::ensure_admin_did(origin)?;
            ensure!(!Self::frozen(), Error::<T>::Frozen);
            Frozen::put(true);
            Self::deposit_event(RawEvent::Frozen(did));
        }

        /// Unfreezes transaction handling in the bridge module if it is frozen.
        #[weight = (300_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn unfreeze(origin) {
            let did = Self::ensure_admin_did(origin)?;
            ensure!(Self::frozen(), Error::<T>::NotFrozen);
            Frozen::put(false);
            Self::deposit_event(RawEvent::Unfrozen(did));
        }

        /// Changes the bridge limits.
        #[weight = (500_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn change_bridge_limit(origin, amount: T::Balance, duration: T::BlockNumber) {
            let did = Self::ensure_admin_did(origin)?;
            <BridgeLimit<T>>::put((amount, duration));
            Self::deposit_event(RawEvent::BridgeLimitUpdated(did, amount, duration));
        }

        /// Changes the bridge limit exempted list.
        #[weight = (500_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn change_bridge_exempted(origin, exempted: Vec<(IdentityId, bool)>) {
            let did = Self::ensure_admin_did(origin)?;
            for (exempt_did, exempt) in exempted {
                BridgeLimitExempted::insert(exempt_did, exempt);
                Self::deposit_event(RawEvent::ExemptedUpdated(did, exempt_did, exempt));
            }
        }

        /// Forces handling a transaction by bypassing the bridge limit and timelock.
        #[weight = (600_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn force_handle_bridge_tx(origin, bridge_tx: BridgeTx<T::AccountId, T::Balance>) -> DispatchResult {
            // NB: To avoid code duplication, this uses a hacky approach of temporarily exempting the did
            Self::ensure_admin(origin)?;
            Self::force_handle_signed_bridge_tx(bridge_tx)
        }

        /// Proposes a vector of bridge transactions. The vector is processed until the first
        /// proposal which causes an error, in which case the error is returned and the rest of
        /// proposals are not processed.
        ///
        /// # Weight
        /// `500_000_000 + 7_000_000 * bridge_txs.len()`
        #[weight =(
            500_000_000 + 7_000_000 * u64::try_from(bridge_txs.len()).unwrap_or_default(),
            DispatchClass::Operational,
            Pays::Yes
        )]
        pub fn batch_propose_bridge_tx(origin, bridge_txs: Vec<BridgeTx<T::AccountId, T::Balance>>) ->
            DispatchResult
        {
            Self::ensure_controller_set()?;
            let sender = ensure_signed(origin)?;
            Self::batch_propose_signed_bridge_tx(sender, bridge_txs)
        }

        /// Proposes a bridge transaction, which amounts to making a multisig proposal for the
        /// bridge transaction if the transaction is new or approving an existing proposal if the
        /// transaction has already been proposed.
        #[weight = (500_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn propose_bridge_tx(origin, bridge_tx: BridgeTx<T::AccountId, T::Balance>) ->
            DispatchResult
        {
            Self::ensure_controller_set()?;
            let sender = ensure_signed(origin)?;
            Self::propose_signed_bridge_tx(sender, bridge_tx)
        }

        /// Handles an approved bridge transaction proposal.
        #[weight = (900_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn handle_bridge_tx(origin, bridge_tx: BridgeTx<T::AccountId, T::Balance>) ->
            DispatchResult
        {
            let sender = ensure_signed(origin)?;
            Self::handle_signed_bridge_tx(&sender, bridge_tx)
        }

        /// Freezes given bridge transactions.
        /// If any bridge txn is already handled then this function will just ignore it and process next one.
        ///
        /// # Weight
        /// `400_000_000 + 2_000_000 * bridge_txs.len()`
        #[weight = (
            400_000_000 + 2_000_000 * u64::try_from(bridge_txs.len()).unwrap_or_default(),
            DispatchClass::Operational,
            Pays::Yes
        )]
        pub fn freeze_txs(origin, bridge_txs: Vec<BridgeTx<T::AccountId, T::Balance>>) {
            let did = Self::ensure_admin_did(origin)?;
            for bridge_tx in bridge_txs {
                let tx_details = Self::bridge_tx_details(&bridge_tx.recipient, &bridge_tx.nonce);
                if tx_details.status != BridgeTxStatus::Handled {
                    Self::update_status(&bridge_tx, BridgeTxStatus::Frozen);
                    Self::deposit_event(RawEvent::FrozenTx(did, bridge_tx));
                }
            }
        }

        /// Unfreezes given bridge transactions.
        /// If any bridge txn is already handled then this function will just ignore it and process next one.
        ///
        /// # Weight
        /// `400_000_000 + 7_000_000 * bridge_txs.len()`
        #[weight = (
            400_000_000 + 7_000_000 * u64::try_from(bridge_txs.len()).unwrap_or_default(),
            DispatchClass::Operational,
            Pays::Yes
        )]
        pub fn unfreeze_txs(origin, bridge_txs: Vec<BridgeTx<T::AccountId, T::Balance>>) {
            // NB: An admin can call Freeze + Unfreeze on a transaction to bypass the timelock
            let did = Self::ensure_admin_did(origin)?;
            for bridge_tx in bridge_txs {
                let tx_details = Self::bridge_tx_details(&bridge_tx.recipient, &bridge_tx.nonce);
                if tx_details.status == BridgeTxStatus::Frozen {
                    Self::update_status(&bridge_tx, BridgeTxStatus::Absent);
                    Self::deposit_event(RawEvent::UnfrozenTx(did, bridge_tx.clone()));
                    if let Err(e) = Self::handle_bridge_tx_now(bridge_tx, true) {
                        sp_runtime::print(e);
                    }
                }
            }
        }

        /// Root callable extrinsic, used as an internal call to handle a scheduled timelocked bridge transaction.
        #[weight = (
            500_000_000,
            DispatchClass::Operational,
            Pays::Yes
        )]
        fn handle_scheduled_bridge_tx(origin, bridge_tx: BridgeTx<T::AccountId, T::Balance>) {
            ensure_root(origin)?;
            let _ = Self::handle_bridge_tx_now(bridge_tx, false)?;
        }
    }
}

impl<T: Trait> Module<T> {
    pub fn controller_key() -> T::AccountId {
        Self::controller()
    }

    fn ensure_admin_did(
        origin: <T as frame_system::Trait>::Origin,
    ) -> Result<IdentityId, DispatchError> {
        let sender = Self::ensure_admin(origin)?;
        Context::current_identity_or::<Identity<T>>(&sender)
    }

    fn ensure_admin(
        origin: <T as frame_system::Trait>::Origin,
    ) -> Result<T::AccountId, DispatchError> {
        let sender = ensure_signed(origin)?;
        ensure!(sender == Self::admin(), Error::<T>::BadAdmin);
        Ok(sender)
    }

    fn ensure_controller_set() -> DispatchResult {
        ensure!(
            Self::controller() != Default::default(),
            Error::<T>::ControllerNotSet
        );
        Ok(())
    }

    fn update_status(tx: &BridgeTx<T::AccountId, T::Balance>, status: BridgeTxStatus) {
        <BridgeTxDetails<T>>::mutate(&tx.recipient, &tx.nonce, |detail| detail.status = status);
    }

    /// Issues the transacted amount to the recipient.
    fn issue(recipient: &T::AccountId, amount: &T::Balance) -> DispatchResult {
        let did = T::CddChecker::get_key_cdd_did(&recipient).ok_or(Error::<T>::NoValidCdd)?;
        if !Self::bridge_exempted(did) {
            let (limit, interval_duration) = Self::bridge_limit();
            ensure!(!interval_duration.is_zero(), Error::<T>::DivisionByZero);
            let current_interval = <system::Module<T>>::block_number() / interval_duration;
            let (bridged, last_interval) = Self::polyx_bridged(did);
            let total_mint = if last_interval == current_interval {
                amount.checked_add(&bridged).ok_or(Error::<T>::Overflow)?
            } else {
                *amount
            };
            ensure!(total_mint <= limit, Error::<T>::BridgeLimitReached);
            <PolyxBridged<T>>::insert(did, (total_mint, current_interval))
        }

        let _pos_imbalance = <balances::Module<T>>::deposit_creating(&recipient, *amount);

        Ok(())
    }

    /// Handles a bridge transaction proposal immediately.
    fn handle_bridge_tx_now(
        bridge_tx: BridgeTx<T::AccountId, T::Balance>,
        untrusted_manual_retry: bool,
    ) -> Result<Weight, DispatchError> {
        let mut tx_details = Self::bridge_tx_details(&bridge_tx.recipient, &bridge_tx.nonce);
        // NB: This function does not care if a transaction is timelocked. Therefore, this should only be called
        // after timelock has expired or timelock is to be bypassed by an admin.
        ensure!(
            tx_details.status != BridgeTxStatus::Handled,
            Error::<T>::ProposalAlreadyHandled
        );
        ensure!(
            tx_details.status != BridgeTxStatus::Frozen,
            Error::<T>::FrozenTx
        );

        if Self::frozen() {
            // Un-trusted manual retries not allowed during frozen state.
            ensure!(!untrusted_manual_retry, Error::<T>::Frozen);
            // Bridge module frozen. Retry this tx again later.
            return Self::handle_bridge_tx_later(bridge_tx, Self::timelock());
        }

        let amount = if untrusted_manual_retry {
            // NB: The amount should be fetched from storage since the amount in `bridge_tx`
            // may be altered in a manual retry
            tx_details.amount
        } else {
            bridge_tx.amount
        };
        if Self::issue(&bridge_tx.recipient, &amount).is_ok() {
            tx_details.status = BridgeTxStatus::Handled;
            tx_details.execution_block = <system::Module<T>>::block_number();
            <BridgeTxDetails<T>>::insert(&bridge_tx.recipient, &bridge_tx.nonce, tx_details);
            let current_did = Context::current_identity::<Identity<T>>().unwrap_or_else(|| GC_DID);
            Self::deposit_event(RawEvent::Bridged(current_did, bridge_tx));
        } else if !untrusted_manual_retry {
            // NB: If this was a manual retry, tx's automated retry schedule is not updated.
            // Recipient missing CDD or limit reached. Retry this tx again later.
            return Self::handle_bridge_tx_later(bridge_tx, Self::timelock());
        }
        Ok(weight_for::handle_bridge_tx::<T>())
    }

    /// Handles a bridge transaction proposal after `timelock` blocks.
    fn handle_bridge_tx_later(
        bridge_tx: BridgeTx<T::AccountId, T::Balance>,
        timelock: T::BlockNumber,
    ) -> Result<Weight, DispatchError> {
        let mut already_tried = 0;
        let mut tx_details = Self::bridge_tx_details(&bridge_tx.recipient, &bridge_tx.nonce);
        match tx_details.status {
            BridgeTxStatus::Absent => {
                tx_details.status = BridgeTxStatus::Timelocked;
                tx_details.amount = bridge_tx.amount;
            }
            BridgeTxStatus::Pending(x) => {
                tx_details.status = BridgeTxStatus::Pending(x + 1);
                already_tried = x + 1;
            }
            BridgeTxStatus::Timelocked => {
                tx_details.status = BridgeTxStatus::Pending(1);
                already_tried = 1;
            }
            BridgeTxStatus::Frozen => {
                return Err(Error::<T>::FrozenTx.into());
            }
            BridgeTxStatus::Handled => {
                return Err(Error::<T>::ProposalAlreadyHandled.into());
            }
        }
        tx_details.tx_hash = bridge_tx.tx_hash;

        if already_tried > 24 {
            // Limits the exponential backoff to *almost infinity* (~180 years)
            already_tried = 24;
        }

        let current_block_number = <system::Module<T>>::block_number();
        let unlock_block_number =
            current_block_number + timelock + T::BlockNumber::from(2u32.pow(already_tried.into()));
        tx_details.execution_block = unlock_block_number;
        <BridgeTxDetails<T>>::insert(&bridge_tx.recipient, &bridge_tx.nonce, tx_details);

        Self::schedule_call(unlock_block_number, bridge_tx);

        Ok(weight_for::handle_bridge_tx_later::<T>())
    }

    /// Proposes a vector of bridge transaction. The bridge controller must be set.
    fn batch_propose_signed_bridge_tx(
        sender: T::AccountId,
        bridge_txs: Vec<BridgeTx<T::AccountId, T::Balance>>,
    ) -> DispatchResult {
        let sender_signer = Signatory::Account(sender);
        let propose = |tx| {
            let proposal = <T as Trait>::Proposal::from(Call::<T>::handle_bridge_tx(tx));
            let boxed_proposal = Box::new(proposal.into());
            <multisig::Module<T>>::create_or_approve_proposal(
                Self::controller(),
                sender_signer.clone(),
                boxed_proposal,
                None,
                true,
            )
        };
        let stati = Self::apply_handler(propose, bridge_txs);
        Self::deposit_event(RawEvent::TxsHandled(stati));
        Ok(())
    }

    /// Proposes a bridge transaction. The bridge controller must be set.
    fn propose_signed_bridge_tx(
        sender: T::AccountId,
        bridge_tx: BridgeTx<T::AccountId, T::Balance>,
    ) -> DispatchResult {
        let sender_signer = Signatory::Account(sender);
        let proposal = <T as Trait>::Proposal::from(Call::<T>::handle_bridge_tx(bridge_tx));
        let boxed_proposal = Box::new(proposal.into());
        <multisig::Module<T>>::create_or_approve_proposal(
            Self::controller(),
            sender_signer,
            boxed_proposal,
            None,
            true,
        )
    }

    /// Handles an approved bridge transaction proposal.
    fn handle_signed_bridge_tx(
        sender: &T::AccountId,
        bridge_tx: BridgeTx<T::AccountId, T::Balance>,
    ) -> DispatchResult {
        let ensure_caller = || -> DispatchResult {
            //TODO: Review admin permissions to handle bridge txs before mainnet
            ensure!(
                sender == &Self::controller() || sender == &Self::admin(),
                Error::<T>::BadCaller
            );
            Ok(())
        };

        let mut tx_details = Self::bridge_tx_details(&bridge_tx.recipient, &bridge_tx.nonce);
        match tx_details.status {
            // New bridge tx
            BridgeTxStatus::Absent => {
                ensure_caller()?;
                let timelock = Self::timelock();
                if timelock.is_zero() {
                    Self::handle_bridge_tx_now(bridge_tx, false)
                } else {
                    Self::handle_bridge_tx_later(bridge_tx, timelock)
                }
                .map(drop)
            }
            // Pending cdd bridge tx
            BridgeTxStatus::Pending(_) => Self::handle_bridge_tx_now(bridge_tx, true).map(drop),
            // Pre frozen tx. We just set the correct amount.
            BridgeTxStatus::Frozen => {
                ensure_caller()?;
                tx_details.amount = bridge_tx.amount;
                <BridgeTxDetails<T>>::insert(&bridge_tx.recipient, &bridge_tx.nonce, tx_details);
                Ok(())
            }
            BridgeTxStatus::Timelocked => Err(Error::<T>::TimelockedTx.into()),
            BridgeTxStatus::Handled => Err(Error::<T>::ProposalAlreadyHandled.into()),
        }
    }

    /// Forces handling a transaction by bypassing the bridge limit and timelock.
    fn force_handle_signed_bridge_tx(
        bridge_tx: BridgeTx<T::AccountId, T::Balance>,
    ) -> DispatchResult {
        // NB: To avoid code duplication, this uses a hacky approach of temporarily exempting the did
        let did =
            T::CddChecker::get_key_cdd_did(&bridge_tx.recipient).ok_or(Error::<T>::NoValidCdd)?;
        if !Self::bridge_exempted(did) {
            // Exempt the did temporarily
            BridgeLimitExempted::insert(did, true);
            let _ = Self::handle_bridge_tx_now(bridge_tx, false)?;
            BridgeLimitExempted::insert(did, false);
        } else {
            // Already exempted
            let _ = Self::handle_bridge_tx_now(bridge_tx, false)?;
            return Ok(());
        }
        Ok(())
    }

    /// Applies a handler `f` to a vector of transactions `bridge_txs` and outputs a vector of
    /// processing results.
    fn apply_handler(
        f: impl Fn(BridgeTx<T::AccountId, T::Balance>) -> DispatchResult,
        bridge_txs: Vec<BridgeTx<T::AccountId, T::Balance>>,
    ) -> Vec<(u32, HandledTxStatus)> {
        let g = |tx: BridgeTx<_, _>| {
            (
                tx.nonce,
                if let Err(e) = f(tx) {
                    HandledTxStatus::Error(e.encode())
                } else {
                    HandledTxStatus::Success
                },
            )
        };
        bridge_txs.into_iter().map(g).collect()
    }

    /// Schedules a timelocked transaction call with constant arguments and emits an event on success or
    /// prints an error message on failure.
    // TODO: handle errors.
    fn schedule_call(block_number: T::BlockNumber, bridge_tx: BridgeTx<T::AccountId, T::Balance>) {
        // Schedule the transaction as a dispatchable call.
        let call = Call::<T>::handle_scheduled_bridge_tx(bridge_tx.clone()).into();
        if let Err(e) = <T as Trait>::Scheduler::schedule(
            DispatchTime::At(block_number),
            None,
            LOWEST_PRIORITY,
            RawOrigin::Root.into(),
            call,
        ) {
            <Identity<T>>::emit_unexpected_error(Some(e));
        } else {
            let current_did = Context::current_identity::<Identity<T>>().unwrap_or_else(|| GC_DID);
            Self::deposit_event(RawEvent::BridgeTxScheduled(
                current_did,
                bridge_tx,
                block_number,
            ));
        }
    }
}
