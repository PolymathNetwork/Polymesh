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
//! - `batch_force_handle_bridge_tx`: Forces handling a vector of transactions.
//! - `propose_bridge_tx`: Proposes a bridge transaction, which amounts to making a multisig
//! - `batch_propose_bridge_tx`: Proposes a vector of bridge transactions.
//! - `handle_bridge_tx`: Handles an approved bridge transaction proposal.
//! - `batch_handle_bridge_tx`: Handles a vector of approved bridge transaction proposals.
//! - `freeze_txs`: Freezes given bridge transactions.
//! - `unfreeze_txs`: Unfreezes given bridge transactions.

use codec::{Decode, Encode};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    storage::StorageDoubleMap,
    traits::{Currency, Get},
    weights::{DispatchClass, Pays, Weight},
};
use frame_system::{self as system, ensure_signed};
use pallet_balances as balances;
use pallet_identity as identity;
use pallet_multisig as multisig;
use polymesh_common_utilities::{
    traits::{balances::CheckCdd, identity::Trait as IdentityTrait, CommonTrait},
    Context, GC_DID,
};
use polymesh_primitives::{IdentityId, Signatory};
use sp_core::H256;
use sp_runtime::traits::{CheckedAdd, One, SaturatedConversion, Zero};
use sp_std::{convert::TryFrom, prelude::*};

type Identity<T> = identity::Module<T>;

pub trait Trait: multisig::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type Proposal: From<Call<Self>> + Into<<Self as IdentityTrait>::Proposal>;
    /// The maximum number of timelocked bridge transactions that can be scheduled to be
    /// executed in a single block. Any excess bridge transactions are scheduled in later
    /// blocks.
    type MaxTimelockedTxsPerBlock: Get<u32>;
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
    pub(crate) fn handle_bridge_tx_later<T: Trait>(count: u64) -> Weight {
        let db = T::DbWeight::get();
        db.reads_writes(4, 2)
            .saturating_add(500_000_000) // base value
            .saturating_add(count.saturating_mul(500_00)) // for one loop
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// The bridge controller address is not set.
        ControllerNotSet,
        /// The signer does not have an identity.
        IdentityMissing,
        /// Failure to credit the recipient account or identity.
        CannotCreditRecipient,
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
        /// There is no proposal corresponding to a given bridge transaction.
        NoSuchProposal,
        /// All the blocks in the timelock block range are full.
        TimelockBlockRangeFull,
        /// The identity's minted total has reached the bridge limit.
        BridgeLimitReached,
        /// The identity's minted total has overflowed.
        Overflow,
        /// The block interval duration is zero. Cannot divide.
        DivisionByZero,
        /// The transaction is timelocked.
        TimelockedTx,
        /// Missing Current Identity
        MissingCurrentIdentity
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as Bridge {
        /// The multisig account of the bridge controller. The genesis signers accept their
        /// authorizations and are able to get their proposals delivered. The bridge creator
        /// transfers some POLY to their identity.
        Controller get(fn controller) build(|config: &GenesisConfig<T>| {
            use frame_support::debug;

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
                vec![],
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

        /// The list of timelocked transactions with the block numbers in which those transactions
        /// become unlocked. Pending transactions are also included here to be retried
        /// automatically.
        TimelockedTxs get(fn timelocked_txs):
            map hasher(twox_64_concat) T::BlockNumber => Vec<BridgeTx<T::AccountId, T::Balance>>;

        /// The maximum number of bridged POLYX per identity within a set interval of
        /// blocks. Fields: POLYX amount and the block interval duration.
        BridgeLimit get(fn bridge_limit) config(): (T::Balance, T::BlockNumber);

        /// Amount of POLYX bridged by the identity in last block interval. Fields: the bridged
        /// amount and the last interval number.
        PolyxBridged get(fn polyx_bridged): map hasher(twox_64_concat) IdentityId => (T::Balance, T::BlockNumber);

        /// Identities not constrained by the bridge limit.
        BridgeLimitExempted get(fn bridge_exempted): map hasher(twox_64_concat) IdentityId => bool;
    }
    add_extra_genesis {
        // TODO: Remove multisig creator and add systematic CDD for the bridge multisig.
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
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        const MaxTimelockedTxsPerBlock: u32 = T::MaxTimelockedTxsPerBlock::get();

        fn deposit_event() = default;

        /// Issues tokens in timelocked transactions.
        fn on_initialize(block_number: T::BlockNumber) -> Weight {
            Self::handle_timelocked_txs(block_number)
        }

        /// Changes the controller account as admin.
        #[weight = (300_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn change_controller(origin, controller: T::AccountId) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            ensure!(sender == Self::admin(), Error::<T>::BadAdmin);
            <Controller<T>>::put(controller.clone());
            let current_did = Context::current_identity_or::<Identity<T>>(&sender)?;
            Self::deposit_event(RawEvent::ControllerChanged(current_did, controller));
            Ok(())
        }

        /// Changes the bridge admin key.
        #[weight = (300_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn change_admin(origin, admin: T::AccountId) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            ensure!(sender == Self::admin(), Error::<T>::BadAdmin);
            <Admin<T>>::put(admin.clone());
            let current_did = Context::current_identity_or::<Identity<T>>(&sender)?;
            Self::deposit_event(RawEvent::AdminChanged(current_did, admin));
            Ok(())
        }

        /// Changes the timelock period.
        #[weight = (300_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn change_timelock(origin, timelock: T::BlockNumber) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            ensure!(sender == Self::admin(), Error::<T>::BadAdmin);
            <Timelock<T>>::put(timelock);
            let current_did = Context::current_identity_or::<Identity<T>>(&sender)?;
            Self::deposit_event(RawEvent::TimelockChanged(current_did, timelock));
            Ok(())
        }

        /// Freezes transaction handling in the bridge module if it is not already frozen. When the
        /// bridge is frozen, attempted transactions get postponed instead of getting handled.
        #[weight = (300_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn freeze(origin) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            let current_did = Context::current_identity_or::<Identity<T>>(&sender)?;
            ensure!(sender == Self::admin(), Error::<T>::BadAdmin);
            ensure!(!Self::frozen(), Error::<T>::Frozen);
            <Frozen>::put(true);
            Self::deposit_event(RawEvent::Frozen(current_did));
            Ok(())
        }

        /// Unfreezes transaction handling in the bridge module if it is frozen.
        #[weight = (300_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn unfreeze(origin) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            let current_did = Context::current_identity_or::<Identity<T>>(&sender)?;
            ensure!(sender == Self::admin(), Error::<T>::BadAdmin);
            ensure!(Self::frozen(), Error::<T>::NotFrozen);
            <Frozen>::put(false);
            Self::deposit_event(RawEvent::Unfrozen(current_did));
            Ok(())
        }

        /// Changes the bridge limits.
        #[weight = (500_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn change_bridge_limit(origin, amount: T::Balance, duration: T::BlockNumber) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            let current_did = Context::current_identity_or::<Identity<T>>(&sender)?;
            ensure!(sender == Self::admin(), Error::<T>::BadAdmin);
            <BridgeLimit<T>>::put((amount, duration));
            Self::deposit_event(RawEvent::BridgeLimitUpdated(current_did, amount, duration));
            Ok(())
        }

        /// Changes the bridge limit exempted list.
        #[weight = (500_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn change_bridge_exempted(origin, exempted: Vec<(IdentityId, bool)>) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            let current_did = Context::current_identity_or::<Identity<T>>(&sender)?;
            ensure!(sender == Self::admin(), Error::<T>::BadAdmin);
            for (did, exempt) in exempted {
                <BridgeLimitExempted>::insert(did, exempt);
                Self::deposit_event(RawEvent::ExemptedUpdated(current_did, did, exempt));
            }
            Ok(())
        }

        /// Forces handling a transaction by bypassing the bridge limit and timelock.
        #[weight = (600_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn force_handle_bridge_tx(origin, bridge_tx: BridgeTx<T::AccountId, T::Balance>) -> DispatchResult {
            // NB: To avoid code duplication, this uses a hacky approach of temporarily exempting the did
            let sender = ensure_signed(origin)?;
            ensure!(sender == Self::admin(), Error::<T>::BadAdmin);
            Self::force_handle_signed_bridge_tx(bridge_tx)
        }

        /// Forces handling a vector of transactions by bypassing the bridge limit and timelock.
        /// It collects results of processing every transaction in the given vector and outputs
        /// the vector of results (In event) which has the same length as the `bridge_txs` have
        ///
        /// # Weight
        /// `600_000_000 + 200_000 * bridge_txs.len()`
        #[weight = (600_000_000 + 200_000 * u64::try_from(bridge_txs.len()).unwrap_or_default(), DispatchClass::Operational, Pays::Yes)]
        pub fn batch_force_handle_bridge_tx(origin, bridge_txs: Vec<BridgeTx<T::AccountId, T::Balance>>) ->
            DispatchResult
        {
            let sender = ensure_signed(origin)?;
            ensure!(sender == Self::admin(), Error::<T>::BadAdmin);
            let stati = Self::apply_handler(
                Self::force_handle_signed_bridge_tx,
                bridge_txs
            );
            Self::deposit_event(RawEvent::TxsHandled(stati));
            Ok(())
        }

        /// Proposes a bridge transaction, which amounts to making a multisig proposal for the
        /// bridge transaction if the transaction is new or approving an existing proposal if the
        /// transaction has already been proposed.
        #[weight = (500_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn propose_bridge_tx(origin, bridge_tx: BridgeTx<T::AccountId, T::Balance>) ->
            DispatchResult
        {
            ensure!(Self::controller() != Default::default(), Error::<T>::ControllerNotSet);
            let sender = ensure_signed(origin)?;
            Self::propose_signed_bridge_tx(sender, bridge_tx)
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
            ensure!(Self::controller() != Default::default(), Error::<T>::ControllerNotSet);
            let sender = ensure_signed(origin)?;
            Self::batch_propose_signed_bridge_tx(sender, bridge_txs)
        }

        /// Handles an approved bridge transaction proposal.
        #[weight = (900_000_000, DispatchClass::Operational, Pays::Yes)]
        pub fn handle_bridge_tx(origin, bridge_tx: BridgeTx<T::AccountId, T::Balance>) ->
            DispatchResult
        {
            let sender = ensure_signed(origin)?;
            Self::handle_signed_bridge_tx(&sender, bridge_tx)
        }

        /// Handles a vector of approved bridge transaction proposals.
        /// It deposits an event (i.e TxsHandled) which consist the result of every BridgeTx.
        ///
        /// # Weight
        /// `900_000_000 + 2_000_000 * bridge_txs.len()`
        #[weight = (
            900_000_000 + 2_000_000 * u64::try_from(bridge_txs.len()).unwrap_or_default(),
            DispatchClass::Operational,
            Pays::Yes
        )]
        pub fn batch_handle_bridge_tx(origin, bridge_txs: Vec<BridgeTx<T::AccountId, T::Balance>>) ->
            DispatchResult
        {
            let sender = ensure_signed(origin)?;
            let stati = Self::apply_handler(
                |tx| Self::handle_signed_bridge_tx(&sender, tx),
                bridge_txs
            );
            Self::deposit_event(RawEvent::TxsHandled(stati));
            Ok(())
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
        pub fn batch_freeze_tx(origin, bridge_txs: Vec<BridgeTx<T::AccountId, T::Balance>>) ->
            DispatchResult
        {
            let sender = ensure_signed(origin)?;
            let current_did = Context::current_identity_or::<Identity<T>>(&sender)?;
            ensure!(sender == Self::admin(), Error::<T>::BadAdmin);
            for bridge_tx in bridge_txs {
                let tx_details = Self::bridge_tx_details(&bridge_tx.recipient, &bridge_tx.nonce);
                if tx_details.status != BridgeTxStatus::Handled {
                    <BridgeTxDetails<T>>::mutate(&bridge_tx.recipient, &bridge_tx.nonce, |tx_detail| tx_detail.status = BridgeTxStatus::Frozen);
                    Self::deposit_event(RawEvent::FrozenTx(current_did, bridge_tx));
                }
            }
            Ok(())
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
        pub fn batch_unfreeze_tx(origin, bridge_txs: Vec<BridgeTx<T::AccountId, T::Balance>>) ->
            DispatchResult
        {
            // NB: An admin can call Freeze + Unfreeze on a transaction to bypass the timelock
            let sender = ensure_signed(origin)?;
            let current_did = Context::current_identity_or::<Identity<T>>(&sender)?;
            ensure!(sender == Self::admin(), Error::<T>::BadAdmin);
            for bridge_tx in bridge_txs {
                let tx_details = Self::bridge_tx_details(&bridge_tx.recipient, &bridge_tx.nonce);
                if tx_details.status == BridgeTxStatus::Frozen {
                    <BridgeTxDetails<T>>::mutate(&bridge_tx.recipient, &bridge_tx.nonce, |tx_detail| tx_detail.status = BridgeTxStatus::Absent);
                    Self::deposit_event(RawEvent::UnfrozenTx(current_did, bridge_tx.clone()));
                    if let Err(e) = Self::handle_bridge_tx_now(bridge_tx, true) {
                        sp_runtime::print(e);
                    }
                }
            }
            Ok(())
        }

    }
}

impl<T: Trait> Module<T> {
    pub fn controller_key() -> T::AccountId {
        Self::controller()
    }

    /// Issues the transacted amount to the recipient.
    fn issue(recipient: &T::AccountId, amount: &T::Balance) -> DispatchResult {
        if let Some(did) = T::CddChecker::get_key_cdd_did(&recipient) {
            if !Self::bridge_exempted(did) {
                let current_block_number = <system::Module<T>>::block_number();
                let (limit, interval_duration) = Self::bridge_limit();
                ensure!(!interval_duration.is_zero(), Error::<T>::DivisionByZero);
                let current_interval = current_block_number / interval_duration;
                let (bridged, last_interval) = Self::polyx_bridged(did);
                let mut total_mint = *amount;
                if last_interval == current_interval {
                    total_mint = total_mint
                        .checked_add(&bridged)
                        .ok_or(Error::<T>::Overflow)?;
                }
                ensure!(total_mint <= limit, Error::<T>::BridgeLimitReached);
                <PolyxBridged<T>>::insert(did, (total_mint, current_interval))
            }
        } else {
            return Err(Error::<T>::NoValidCdd.into());
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
        let mut unlock_block_number =
            current_block_number + timelock + T::BlockNumber::from(2u32.pow(already_tried.into()));
        let max_timelocked_txs_per_block = T::MaxTimelockedTxsPerBlock::get() as usize;
        let old_unlock_count_number = unlock_block_number;
        while Self::timelocked_txs(unlock_block_number).len() >= max_timelocked_txs_per_block {
            unlock_block_number += One::one();
        }
        let calculated_weight = weight_for::handle_bridge_tx_later::<T>(
            (unlock_block_number - old_unlock_count_number).saturated_into::<u64>(),
        );
        tx_details.execution_block = unlock_block_number;
        <BridgeTxDetails<T>>::insert(&bridge_tx.recipient, &bridge_tx.nonce, tx_details);
        <TimelockedTxs<T>>::mutate(&unlock_block_number, |txs| {
            txs.push(bridge_tx.clone());
        });
        let current_did = Context::current_identity::<Identity<T>>().unwrap_or_else(|| GC_DID);
        Self::deposit_event(RawEvent::BridgeTxScheduled(
            current_did,
            bridge_tx,
            unlock_block_number,
        ));

        Ok(calculated_weight)
    }

    /// Handles the timelocked transactions that are set to unlock at the given block number.
    fn handle_timelocked_txs(block_number: T::BlockNumber) -> Weight {
        let mut weight = 0;
        let txs = <TimelockedTxs<T>>::take(block_number);
        for tx in txs {
            weight += match Self::handle_bridge_tx_now(tx, false) {
                Ok(weight) => weight,
                Err(e) => {
                    sp_runtime::print(e);
                    Zero::zero()
                }
            };
        }
        weight
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

    /// Handles an approved bridge transaction proposal.
    fn handle_signed_bridge_tx(
        sender: &T::AccountId,
        bridge_tx: BridgeTx<T::AccountId, T::Balance>,
    ) -> DispatchResult {
        let mut tx_details = Self::bridge_tx_details(&bridge_tx.recipient, &bridge_tx.nonce);
        match tx_details.status {
            // New bridge tx
            BridgeTxStatus::Absent => {
                //TODO: Review admin permissions to handle bridge txs before mainnet
                ensure!(
                    sender == &Self::controller() || sender == &Self::admin(),
                    Error::<T>::BadCaller
                );

                let timelock = Self::timelock();
                if timelock.is_zero() {
                    let _ = Self::handle_bridge_tx_now(bridge_tx, false)?;
                } else {
                    let _ = Self::handle_bridge_tx_later(bridge_tx, timelock)?;
                }

                Ok(())
            }
            // Pending cdd bridge tx
            BridgeTxStatus::Pending(_) => {
                let _ = Self::handle_bridge_tx_now(bridge_tx, true)?;
                Ok(())
            }
            // Pre frozen tx. We just set the correct amount.
            BridgeTxStatus::Frozen => {
                //TODO: Review admin permissions to handle bridge txs before mainnet
                ensure!(
                    sender == &Self::controller() || sender == &Self::admin(),
                    Error::<T>::BadCaller
                );
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
        if let Some(did) = T::CddChecker::get_key_cdd_did(&bridge_tx.recipient) {
            if !Self::bridge_exempted(did) {
                // Exempt the did temporarily
                <BridgeLimitExempted>::insert(did, true);
                let _ = Self::handle_bridge_tx_now(bridge_tx, false)?;
                <BridgeLimitExempted>::insert(did, false);
            } else {
                // Already exempted
                let _ = Self::handle_bridge_tx_now(bridge_tx, false)?;
                return Ok(());
            }
        } else {
            return Err(Error::<T>::NoValidCdd.into());
        }
        Ok(())
    }

    /// Applies a handler `f` to a vector of transactions `bridge_txs` and outputs a vector of
    /// processing results.
    fn apply_handler<F>(
        f: F,
        bridge_txs: Vec<BridgeTx<T::AccountId, T::Balance>>,
    ) -> Vec<(u32, HandledTxStatus)>
    where
        F: Fn(BridgeTx<T::AccountId, T::Balance>) -> DispatchResult,
    {
        let g = |tx: BridgeTx<T::AccountId, T::Balance>| {
            let nonce = tx.nonce;
            (
                nonce,
                if let Err(e) = f(tx) {
                    HandledTxStatus::Error(e.encode())
                } else {
                    HandledTxStatus::Success
                },
            )
        };
        bridge_txs.into_iter().map(g).collect()
    }
}