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

//! # Pips Module
//!
//! Polymesh Improvement Proposals (PIPs) are dispatchables that can be `propose`d for execution.
//! These PIPs can either be proposed by a committee, or they can be proposed by a community member,
//! in which case they can `vote`d on by all POLYX token holders.
//!
//! Voting, or rather "signalling", which currently scales linearly with POLX,
//! in this system is used to direct the Governance Councils (GCs)
//! attention by moving proposals up and down a review queue, specific to community proposals.
//!
//! From time to time, the GC will take a `snapshot` of this queue,
//! meet and review PIPs, and reject, approve, or skip the proposal (via `enact_snapshot_results`).
//! Any approved PIPs from this snapshot will then be scheduled,
//! in order of signal value, to be executed automatically on the blockchain.
//! However, using `reschedule_execution`, a special Release Coordinator (RC), a member of the GC,
//! can reschedule approved PIPs at will, except for a PIP to replace the RC.
//! Once no longer relevant, the snapshot can be cleared by the GC through `clear_snapshot`.
//!
//! As aforementioned, the GC can skip a PIP, which will increments its "skipped count".
//! Should a configurable limit for the skipped count be exceeded, a PIP can no longer be skipped.
//!
//! Committee proposals, as noted before, do not enter the snapshot or receive votes.
//! However, the GC can at any moment approve such a PIP via `approve_committee_proposal`.
//!
//! Should the GC want to reject an active (scheduled or pending) proposal,
//! they can do so at any time using `reject_proposal`.
//! For garbage collection purposes, it is also possible to use `prune_proposal`,
//! which will, without any restrictions on its state, remove the PIP's storage.
//!
//!
//! ## Overview
//!
//! The Pips module provides functions for:
//!
//! - Proposing and amending PIPs
//! - Signalling (voting) on them for adjusting priority in the review queue
//! - Taking and clearing snapshots of the queue
//! - Approving, rejecting, skipping, and rescheduling PIPs
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! #### Configuration changes
//!
//! - `set_prune_historical_pips` change whether historical PIPs are pruned
//! - `set_min_proposal_deposit` change min deposit to create a proposal
//! - `set_default_enactment_period` change the period after enactment after which the proposal is executed
//! - `set_max_pip_skip_count` change the maximum times a PIP can be skipped
//! - `set_active_pip_limit` change the maximum number of concurrently active PIPs
//!
//! #### Other
//!
//! - `propose` - token holders can propose a new PIP.
//! - `amend_proposal` - allows the creator of a proposal to amend the proposal details
//! - `cancel_proposal` - allows the creator of a proposal to cancel the proposal
//! - `vote` - token holders, including the PIP's proposer, can vote on a PIP.
//! - `approve_committee_proposal` - allows the GC to approve a committee proposal
//! - `reject_proposal` - reject an active proposal and refund deposits
//! - `prune_proposal` - prune all storage associated with proposal and refund deposits
//! - `reschedule_execution` - release coordinator can reschedule a PIPs execution
//! - `clear_snapshot` - clears the snapshot
//! - `snapshot` - takes a new snapshot of the review queue
//! - `enact_snapshot_results` - enters results (approve, reject, and skip) for PIPs in snapshot
//!
//! ### Public Functions
//!
//! - `end_block` - executes scheduled proposals

#![cfg_attr(not(feature = "std"), no_std)]
#![feature(const_option)]
#![feature(or_patterns)]
#![feature(bool_to_option)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

use codec::{Decode, Encode, FullCodec};
use core::{cmp::Ordering, mem};
use frame_support::{
    debug, decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchResult, DispatchResultWithPostInfo},
    ensure, parameter_types,
    storage::IterableStorageMap,
    traits::{
        schedule::{DispatchTime, Named as ScheduleNamed, HARD_DEADLINE},
        Currency, EnsureOrigin, Get, LockIdentifier, WithdrawReasons,
    },
    weights::{constants::WEIGHT_PER_SECOND, Weight},
    StorageValue,
};
use frame_system::{self as system, ensure_root, ensure_signed, RawOrigin};
use pallet_identity::{self as identity, PermissionedCallOriginData};
use pallet_treasury::TreasuryTrait;
use polymesh_common_utilities::{
    constants::{schedule_name_prefix::*, PIP_MAX_REPORTING_SIZE},
    identity::Trait as IdentityTrait,
    protocol_fee::{ChargeProtocolFee, ProtocolOp},
    traits::{
        balances::LockableCurrencyExt, governance_group::GovernanceGroupTrait, group::GroupTrait,
        pip::PipId,
    },
    with_transaction, CommonTrait, Context, MaybeBlock, GC_DID,
};
use polymesh_primitives::IdentityId;
use polymesh_primitives_derive::VecU8StrongTyped;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::H256;
use sp_runtime::traits::{
    BlakeTwo256, CheckedAdd, CheckedSub, Dispatchable, Hash, One, Saturating, Zero,
};
use sp_std::{convert::From, prelude::*};
use sp_version::RuntimeVersion;

const PIPS_LOCK_ID: LockIdentifier = *b"pips    ";

pub trait WeightInfo {
    fn set_prune_historical_pips() -> Weight;
    fn set_min_proposal_deposit() -> Weight;
    fn set_default_enactment_period() -> Weight;
    fn set_pending_pip_expiry() -> Weight;
    fn set_max_pip_skip_count() -> Weight;
    fn set_active_pip_limit() -> Weight;
    fn propose_from_community() -> Weight;
    fn propose_from_committee() -> Weight;
    fn vote() -> Weight;
    fn approve_committee_proposal() -> Weight;
    fn reject_proposal() -> Weight;
    fn prune_proposal() -> Weight;
    fn reschedule_execution() -> Weight;
    fn clear_snapshot() -> Weight;
    fn snapshot() -> Weight;
    fn enact_snapshot_results(a: u32, r: u32, s: u32) -> Weight;
    fn execute_scheduled_pip() -> Weight;
    fn expire_scheduled_pip() -> Weight;
}

/// Balance
type BalanceOf<T> =
    <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

/// A wrapper for a proposal url.
#[derive(
    Decode, Encode, Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, VecU8StrongTyped,
)]
pub struct Url(pub Vec<u8>);

/// A wrapper for a proposal description.
#[derive(
    Decode, Encode, Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, VecU8StrongTyped,
)]
pub struct PipDescription(pub Vec<u8>);

/// Represents a proposal
#[derive(Encode, Decode, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Pip<T: Trait> {
    /// The proposal's unique id.
    pub id: PipId,
    /// The proposal being voted on.
    pub proposal: T::Proposal,
    /// The latest state
    pub state: ProposalState,
    /// The issuer of `propose`.
    pub proposer: Proposer<T::AccountId>,
}

/// A result of execution of get_votes.
#[derive(Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub enum VoteCount<Balance> {
    /// Proposal was found and has the following votes.
    ProposalFound {
        /// Stake for
        ayes: Balance,
        /// Stake against
        nays: Balance,
    },
    /// Proposal was not for given index.
    ProposalNotFound,
}

/// Either the entire proposal encoded as a byte vector or its hash. The latter represents large
/// proposals.
#[derive(Encode, Decode, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProposalData {
    /// The hash of the proposal.
    Hash(H256),
    /// The entire proposal.
    Proposal(Vec<u8>),
}

/// The various sorts of committees that can make a PIP.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug)]
pub enum Committee {
    /// The technical committee.
    Technical,
    /// The upgrade committee tends to propose chain upgrades.
    Upgrade,
}

/// The proposer of a certain PIP.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug)]
pub enum Proposer<AccountId> {
    /// The proposer is of the community.
    Community(AccountId),
    /// The proposer is a committee.
    Committee(Committee),
}

/// Represents a proposal metadata
#[derive(Encode, Decode, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PipsMetadata<T: Trait> {
    /// The proposal's unique id.
    pub id: PipId,
    /// The proposal url for proposal discussion.
    pub url: Option<Url>,
    /// The proposal description.
    pub description: Option<PipDescription>,
    /// The block when the PIP was made.
    pub created_at: T::BlockNumber,
    /// Assuming the runtime has a given `rv: RuntimeVersion` at the point of `Pips::propose`,
    /// then this field contains `rv.transaction_version`.
    ///
    /// Currently, this is only used for off-chain purposes to highlight any differences
    /// in the proposal's transaction version from the current one.
    pub transaction_version: u32,
    /// The point, if any, at which this PIP, if still in a `Pending` state,
    /// is expired, and thus no longer valid.
    ///
    /// This field has no operational on-chain effect and is provided for UI purposes only.
    /// On-chain effects are instead handled via scheduling.
    pub expiry: MaybeBlock<T::BlockNumber>,
}

/// For keeping track of proposal being voted on.
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct VotingResult<Balance> {
    /// The current set of voters that approved with their stake.
    pub ayes_count: u32,
    pub ayes_stake: Balance,
    /// The current set of voters that rejected with their stake.
    pub nays_count: u32,
    pub nays_stake: Balance,
}

/// A "vote" or "signal" on a PIP to move it up or down the review queue.
#[derive(PartialEq, Eq, Copy, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct Vote<Balance>(
    /// `true` if there's agreement.
    pub bool,
    /// How strongly do they feel about it?
    pub Balance,
);

#[derive(PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct VoteByPip<VoteType> {
    pub pip: PipId,
    pub vote: VoteType,
}

pub type HistoricalVotingByAddress<VoteType> = Vec<VoteByPip<VoteType>>;
pub type HistoricalVotingById<AccountId, VoteType> =
    Vec<(AccountId, HistoricalVotingByAddress<VoteType>)>;

/// The state a PIP is in.
#[derive(Encode, Decode, Copy, Clone, Eq, PartialEq, Debug)]
pub enum ProposalState {
    /// Initial state. Proposal is open to voting.
    Pending,
    /// Proposal was rejected by the GC.
    Rejected,
    /// Proposal has been approved by the GC and scheduled for execution.
    Scheduled,
    /// Proposal execution was attempted by failed.
    Failed,
    /// Proposal was successfully executed.
    Executed,
    /// Proposal has expired. Only previously `Pending` PIPs may end up here.
    Expired,
}

impl Default for ProposalState {
    fn default() -> Self {
        ProposalState::Pending
    }
}

/// Information about deposit.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct DepositInfo<AccountId, Balance> {
    /// Owner of the deposit.
    pub owner: AccountId,
    /// Amount deposited.
    pub amount: Balance,
}

/// ID of the taken snapshot in a sequence.
pub type SnapshotId = u32;

/// A snapshot's metadata, containing when it was created and who triggered it.
/// The priority queue is stored separately (see `SnapshottedPip`).
#[derive(Encode, Decode, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct SnapshotMetadata<T: Trait> {
    /// The block when the snapshot was made.
    pub created_at: T::BlockNumber,
    /// Who triggered this snapshot? Should refer to someone in the GC.
    pub made_by: T::AccountId,
    /// Unique ID of this snapshot.
    pub id: SnapshotId,
}

/// A PIP in the snapshot's priority queue for consideration by the GC.
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct SnapshottedPip<Balance> {
    /// Identifies the PIP this refers to.
    pub id: PipId,
    /// Weight of the proposal in the snapshot's priority queue.
    /// Higher weights come before lower weights.
    /// The `bool` denotes the sign, where `true` siginfies a positive number.
    pub weight: (bool, Balance),
}

/// Defines sorting order for PIP priority queues, with highest priority *last*.
/// Having higher prio last allows efficient tail popping, so we have a LIFO structure.
fn compare_spip<B: Ord + Copy>(l: &SnapshottedPip<B>, r: &SnapshottedPip<B>) -> Ordering {
    let (l_dir, l_stake): (bool, B) = l.weight;
    let (r_dir, r_stake): (bool, B) = r.weight;
    l_dir
        .cmp(&r_dir) // Negative has lower prio.
        .then_with(|| match l_dir {
            true => l_stake.cmp(&r_stake), // Higher stake, higher prio...
            // Unless negative stake, in which case lower abs stake, higher prio.
            false => r_stake.cmp(&l_stake),
        })
        // Lower id was made first, so assigned higher prio.
        // This also gives us sorting stability through a total order.
        // Moreover, as `queue` should be in by-id order originally.
        .then(r.id.cmp(&l.id))
}

/// A result to enact for one or many PIPs in the snapshot queue.
// This type is only here due to `enact_snapshot_results`.
#[derive(codec::Encode, codec::Decode, Copy, Clone, PartialEq, Eq, Debug)]
pub enum SnapshotResult {
    /// Approve the PIP and move it to the execution queue.
    Approve,
    /// Reject the PIP, removing it from future consideration.
    Reject,
    /// Skip the PIP, bumping the `skipped_count`,
    /// or fail if the threshold for maximum skips is exceeded.
    Skip,
}

/// The number of times a PIP has been skipped.
pub type SkippedCount = u8;

type Identity<T> = identity::Module<T>;

parameter_types! {
    /// The maximum weight of the pips extrinsic `enact_snapshot_results` which equals to
    /// `MaximumBlockWeight * AvailableBlockRatio`.
    pub const PipsEnactSnapshotMaximumWeight: Weight = WEIGHT_PER_SECOND * 2 * 75 / 100;
}

/// The module's configuration trait.
pub trait Trait:
    frame_system::Trait + pallet_timestamp::Trait + IdentityTrait + CommonTrait
{
    /// Currency type for this module.
    // TODO(centril): Remove `ReservableCurrency` bound once `on_runtime_upgrade` is nixed.
    type Currency: LockableCurrencyExt<Self::AccountId, Moment = Self::BlockNumber>
        + frame_support::traits::ReservableCurrency<Self::AccountId>;

    /// Origin for enacting results for PIPs (reject, approve, skip, etc.).
    type VotingMajorityOrigin: EnsureOrigin<Self::Origin>;

    /// Committee
    type GovernanceCommittee: GovernanceGroupTrait<<Self as pallet_timestamp::Trait>::Moment>;

    /// Voting majority origin for Technical Committee.
    type TechnicalCommitteeVMO: EnsureOrigin<Self::Origin>;

    /// Voting majority origin for Upgrade Committee.
    type UpgradeCommitteeVMO: EnsureOrigin<Self::Origin>;

    type Treasury: TreasuryTrait<<Self as CommonTrait>::Balance>;

    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

    /// Weight calaculation.
    type WeightInfo: WeightInfo;

    /// Scheduler of executed or expired proposals. Since the scheduler module does not have
    /// instances, the names of scheduled tasks should be guaranteed to be unique in this
    /// pallet. Names cannot be just PIP IDs because names of executed and expired PIPs should be
    /// different.
    type Scheduler: ScheduleNamed<Self::BlockNumber, Self::SchedulerCall, Self::SchedulerOrigin>;

    /// A call type for identity-mapping the `Call` enum type. Used by the scheduler.
    type SchedulerCall: From<Call<Self>> + Into<<Self as IdentityTrait>::Proposal>;
}

// This module's storage items.
decl_storage! {
    trait Store for Module<T: Trait> as Pips {
        /// Determines whether historical PIP data is persisted or removed
        pub PruneHistoricalPips get(fn prune_historical_pips) config(): bool;

        /// The minimum amount to be used as a deposit for community PIP creation.
        pub MinimumProposalDeposit get(fn min_proposal_deposit) config(): BalanceOf<T>;

        /// Default enactment period that will be use after a proposal is accepted by GC.
        pub DefaultEnactmentPeriod get(fn default_enactment_period) config(): T::BlockNumber;

        /// How many blocks will it take, after a `Pending` PIP expires,
        /// assuming it has not transitioned to another `ProposalState`?
        pub PendingPipExpiry get(fn pending_pip_expiry) config(): MaybeBlock<T::BlockNumber>;

        /// Maximum times a PIP can be skipped before triggering `CannotSkipPip` in `enact_snapshot_results`.
        pub MaxPipSkipCount get(fn max_pip_skip_count) config(): SkippedCount;

        /// The maximum allowed number for `ActivePipCount`.
        /// Once reached, new PIPs cannot be proposed by community members.
        pub ActivePipLimit get(fn active_pip_limit) config(): u32;

        /// Proposals so far. id can be used to keep track of PIPs off-chain.
        PipIdSequence get(fn pip_id_sequence): u32;

        /// Snapshots so far. id can be used to keep track of snapshots off-chain.
        SnapshotIdSequence get(fn snapshot_id_sequence): u32;

        /// Total count of current pending or scheduled PIPs.
        ActivePipCount get(fn active_pip_count): u32;

        /// The metadata of the active proposals.
        pub ProposalMetadata get(fn proposal_metadata): map hasher(twox_64_concat) PipId => Option<PipsMetadata<T>>;

        /// Those who have locked a deposit.
        /// proposal (id, proposer) -> deposit
        pub Deposits get(fn deposits): double_map hasher(twox_64_concat) PipId, hasher(twox_64_concat) T::AccountId => DepositInfo<T::AccountId, BalanceOf<T>>;

        /// Actual proposal for a given id, if it's current.
        /// proposal id -> proposal
        pub Proposals get(fn proposals): map hasher(twox_64_concat) PipId => Option<Pip<T>>;

        /// PolymeshVotes on a given proposal, if it is ongoing.
        /// proposal id -> vote count
        pub ProposalResult get(fn proposal_result): map hasher(twox_64_concat) PipId => VotingResult<BalanceOf<T>>;

        /// Votes per Proposal and account. Used to avoid double vote issue.
        /// (proposal id, account) -> Vote
        pub ProposalVotes get(fn proposal_vote): double_map hasher(twox_64_concat) PipId, hasher(twox_64_concat) T::AccountId => Option<Vote<BalanceOf<T>>>;

        /// Maps PIPs to the block at which they will be executed, if any.
        pub PipToSchedule get(fn pip_to_schedule): map hasher(twox_64_concat) PipId => Option<T::BlockNumber>;

        /// A live priority queue (lowest priority at index 0)
        /// of pending PIPs up to the active limit.
        /// Priority is defined by the `weight` in the `SnapshottedPip`.
        ///
        /// Unlike `SnapshotQueue`, this queue is live, getting updated with each vote cast.
        /// The snapshot is therefore essentially a point-in-time clone of this queue.
        pub LiveQueue get(fn live_queue): Vec<SnapshottedPip<BalanceOf<T>>>;

        /// The priority queue (lowest priority at index 0) of PIPs at the point of snapshotting.
        /// Priority is defined by the `weight` in the `SnapshottedPip`.
        ///
        /// A queued PIP can be skipped. Doing so bumps the `pip_skip_count`.
        /// Once a (configurable) threshhold is exceeded, a PIP cannot be skipped again.
        pub SnapshotQueue get(fn snapshot_queue): Vec<SnapshottedPip<BalanceOf<T>>>;

        /// The metadata of the snapshot, if there is one.
        pub SnapshotMeta get(fn snapshot_metadata): Option<SnapshotMetadata<T>>;

        /// The number of times a certain PIP has been skipped.
        /// Once a (configurable) threshhold is exceeded, a PIP cannot be skipped again.
        pub PipSkipCount get(fn pip_skip_count): map hasher(twox_64_concat) PipId => SkippedCount;

        /// All existing PIPs where the proposer is a committee.
        /// This list is a cache of all ids in `Proposals` with `Proposer::Committee(_)`.
        pub CommitteePips get(fn committee_pips): Vec<PipId>;
    }
}

decl_event!(
    pub enum Event<T>
    where
        Balance = BalanceOf<T>,
        <T as frame_system::Trait>::AccountId,
        <T as frame_system::Trait>::BlockNumber,
    {
        /// Pruning Historical PIPs is enabled or disabled (caller DID, old value, new value)
        HistoricalPipsPruned(IdentityId, bool, bool),
        /// A PIP was made with a `Balance` stake.
        ///
        /// # Parameters:
        ///
        /// Caller DID, Proposer, PIP ID, deposit, URL, description, expiry time, proposal data.
        ProposalCreated(
            IdentityId,
            Proposer<AccountId>,
            PipId,
            Balance,
            Option<Url>,
            Option<PipDescription>,
            MaybeBlock<BlockNumber>,
            ProposalData,
        ),
        /// Triggered each time the state of a proposal is amended
        ProposalStateUpdated(IdentityId, PipId, ProposalState),
        /// `AccountId` voted `bool` on the proposal referenced by `PipId`
        Voted(IdentityId, AccountId, PipId, bool, Balance),
        /// Pip has been closed, bool indicates whether data is pruned
        PipClosed(IdentityId, PipId, bool),
        /// Execution of a PIP has been scheduled at specific block.
        ExecutionScheduled(IdentityId, PipId, BlockNumber),
        /// Default enactment period (in blocks) has been changed.
        /// (caller DID, old period, new period)
        DefaultEnactmentPeriodChanged(IdentityId, BlockNumber, BlockNumber),
        /// Minimum deposit amount modified
        /// (caller DID, old amount, new amount)
        MinimumProposalDepositChanged(IdentityId, Balance, Balance),
        /// Amount of blocks after which a pending PIP expires.
        /// (caller DID, old expiry, new expiry)
        PendingPipExpiryChanged(IdentityId, MaybeBlock<BlockNumber>, MaybeBlock<BlockNumber>),
        /// The maximum times a PIP can be skipped was changed.
        /// (caller DID, old value, new value)
        MaxPipSkipCountChanged(IdentityId, SkippedCount, SkippedCount),
        /// The maximum number of active PIPs was changed.
        /// (caller DID, old value, new value)
        ActivePipLimitChanged(IdentityId, u32, u32),
        /// Refund proposal
        /// (id, total amount)
        ProposalRefund(IdentityId, PipId, Balance),
        /// The snapshot was cleared.
        SnapshotCleared(IdentityId, SnapshotId),
        /// A new snapshot was taken.
        SnapshotTaken(IdentityId, SnapshotId, Vec<SnapshottedPip<Balance>>),
        /// A PIP in the snapshot queue was skipped.
        /// (gc_did, pip_id, new_skip_count)
        PipSkipped(IdentityId, PipId, SkippedCount),
        /// Results (e.g., approved, rejected, and skipped), were enacted for some PIPs.
        /// (gc_did, snapshot_id_opt, skipped_pips_with_new_count, rejected_pips, approved_pips)
        SnapshotResultsEnacted(IdentityId, Option<SnapshotId>, Vec<(PipId, SkippedCount)>, Vec<PipId>, Vec<PipId>),
        /// Scheduling of the PIP for execution failed in the scheduler pallet.
        ExecutionSchedulingFailed(IdentityId, PipId, BlockNumber),
        /// The PIP has been scheduled for expiry.
        ExpiryScheduled(IdentityId, PipId, BlockNumber),
        /// Scheduling of the PIP for expiry failed in the scheduler pallet.
        ExpirySchedulingFailed(IdentityId, PipId, BlockNumber),
        /// Cancelling the PIP execution failed in the scheduler pallet.
        ExecutionCancellingFailed(PipId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// Only the GC release coordinator is allowed to reschedule proposal execution.
        RescheduleNotByReleaseCoordinator,
        /// The given dispatchable call is not valid for this proposal.
        /// The proposal must be from the community, but isn't.
        NotFromCommunity,
        /// The given dispatchable call is not valid for this proposal.
        /// The proposal must be by community, but isn't.
        NotByCommittee,
        /// The current number of active (pending | scheduled) PIPs exceed the maximum
        /// and the proposal is not by a committee.
        TooManyActivePips,
        /// Proposer specifies an incorrect deposit
        IncorrectDeposit,
        /// Proposer can't afford to lock minimum deposit
        InsufficientDeposit,
        /// The proposal does not exist.
        NoSuchProposal,
        /// Not part of governance committee.
        NotACommitteeMember,
        /// When a block number is less than current block number.
        InvalidFutureBlockNumber,
        /// When number of votes overflows.
        NumberOfVotesExceeded,
        /// When stake amount of a vote overflows.
        StakeAmountOfVotesExceeded,
        /// Missing current DID
        MissingCurrentIdentity,
        /// Proposal is not in the correct state
        IncorrectProposalState,
        /// When enacting snapshot results, an unskippable PIP was skipped.
        CannotSkipPip,
        /// Tried to enact results for the snapshot queue overflowing its length.
        SnapshotResultTooLarge,
        /// Tried to enact result for PIP with id different from that at the position in the queue.
        SnapshotIdMismatch,
        /// Execution of a scheduled proposal failed because it is missing.
        ScheduledProposalDoesntExist,
        /// A proposal that is not in a scheduled state cannot be executed.
        ProposalNotInScheduledState,
    }
}

// The module's dispatchable functions.
decl_module! {
    /// The module declaration.
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        fn on_runtime_upgrade() -> Weight {

            // Recompute the live queue.
            <LiveQueue<T>>::set(Self::compute_live_queue());

            // Done; we've cleared all V1 storage needed; V2 can now be filled in.
            // As for the weight, clearing costs much more than this, but let's pretend.
            0
        }

        /// Change whether completed PIPs are pruned.
        /// Can only be called by root.
        ///
        /// # Arguments
        /// * `prune` specifies whether completed PIPs should be pruned.
        #[weight = <T as Trait>::WeightInfo::set_prune_historical_pips()]
        pub fn set_prune_historical_pips(origin, prune: bool) {
            Self::config::<PruneHistoricalPips, _, _>(origin, prune, RawEvent::HistoricalPipsPruned)?;
        }

        /// Change the minimum proposal deposit amount required to start a proposal.
        /// Can only be called by root.
        ///
        /// # Arguments
        /// * `deposit` the new min deposit required to start a proposal
        #[weight = <T as Trait>::WeightInfo::set_min_proposal_deposit()]
        pub fn set_min_proposal_deposit(origin, deposit: BalanceOf<T>) {
            Self::config::<MinimumProposalDeposit<T>, _, _>(origin, deposit, RawEvent::MinimumProposalDepositChanged)?;
        }

        /// Change the default enactment period.
        /// Can only be called by root.
        ///
        /// # Arguments
        /// * `duration` the new default enactment period it takes for a scheduled PIP to be executed.
        #[weight = <T as Trait>::WeightInfo::set_default_enactment_period()]
        pub fn set_default_enactment_period(origin, duration: T::BlockNumber) {
            Self::config::<DefaultEnactmentPeriod<T>, _, _>(origin, duration, RawEvent::DefaultEnactmentPeriodChanged)?;
        }

        /// Change the amount of blocks after which a pending PIP is expired.
        /// If `expiry` is `None` then PIPs never expire.
        /// Can only be called by root.
        ///
        /// # Arguments
        /// * `expiry` the block-time it takes for a still-`Pending` PIP to expire.
        #[weight = <T as Trait>::WeightInfo::set_pending_pip_expiry()]
        pub fn set_pending_pip_expiry(origin, expiry: MaybeBlock<T::BlockNumber>) {
            Self::config::<PendingPipExpiry<T>, _, _>(origin, expiry, RawEvent::PendingPipExpiryChanged)?;
        }

        /// Change the maximum skip count (`max_pip_skip_count`).
        /// Can only be called by root.
        ///
        /// # Arguments
        /// * `max` skips before a PIP cannot be skipped by GC anymore.
        #[weight = <T as Trait>::WeightInfo::set_max_pip_skip_count()]
        pub fn set_max_pip_skip_count(origin, max: SkippedCount) {
            Self::config::<MaxPipSkipCount, _, _>(origin, max, RawEvent::MaxPipSkipCountChanged)?;
        }

        /// Change the maximum number of active PIPs before community members cannot propose anything.
        /// Can only be called by root.
        ///
        /// # Arguments
        /// * `limit` of concurrent active PIPs.
        #[weight = <T as Trait>::WeightInfo::set_active_pip_limit()]
        pub fn set_active_pip_limit(origin, limit: u32) {
            Self::config::<ActivePipLimit, _, _>(origin, limit, RawEvent::ActivePipLimitChanged)?;
        }

        /// A network member creates a PIP by submitting a dispatchable which
        /// changes the network in someway. A minimum deposit is required to open a new proposal.
        ///
        /// # Arguments
        /// * `proposer` is either a signing key or committee.
        ///    Used to understand whether this is a committee proposal and verified against `origin`.
        /// * `proposal` a dispatchable call
        /// * `deposit` minimum deposit value, which is ignored if `proposer` is a committee.
        /// * `url` a link to a website for proposal discussion
        #[weight = <T as Trait>::WeightInfo::propose_from_community()]
        pub fn propose(
            origin,
            proposal: Box<T::Proposal>,
            deposit: BalanceOf<T>,
            url: Option<Url>,
            description: Option<PipDescription>,
        ) {
            // 1. Infer the proposer from `origin`.
            let proposer = Self::ensure_infer_proposer(origin)?;

            let did = Self::current_did_or_missing()?;

            // 2. Add a deposit for community PIPs.
            if let Proposer::Community(ref proposer) = proposer {
                // ...but first make sure active PIP limit isn't crossed.
                // This doesn't apply to committee PIPs.
                // `0` is special and denotes no limit.
                let limit = ActivePipLimit::get();
                ensure!(limit == 0 || ActivePipCount::get() < limit, Error::<T>::TooManyActivePips);

                // Pre conditions: caller must have min balance.
                ensure!(deposit >= Self::min_proposal_deposit(), Error::<T>::IncorrectDeposit);

                // Lock the deposit.
                Self::increase_lock(proposer, deposit)?;
            } else {
                // Committee PIPs cannot have a deposit.
                ensure!(deposit.is_zero(), Error::<T>::NotFromCommunity);
            }

            // 3. Charge protocol fees, even for committee PIPs.
            <T as IdentityTrait>::ProtocolFee::charge_fee(ProtocolOp::PipsPropose)?;

            // 4. Construct and add PIP to storage.
            let id = Self::next_pip_id();
            let created_at = <system::Module<T>>::block_number();
            let expiry = Self::pending_pip_expiry() + created_at;
            let transaction_version = <T::Version as Get<RuntimeVersion>>::get().transaction_version;
            let proposal_data = Self::reportable_proposal_data(&*proposal);
            <ProposalMetadata<T>>::insert(id, PipsMetadata {
                id,
                created_at,
                url: url.clone(),
                description: description.clone(),
                transaction_version,
                expiry,
            });
            <Proposals<T>>::insert(id, Pip {
                id,
                proposal: *proposal,
                state: ProposalState::Pending,
                proposer: proposer.clone(),
            });
            ActivePipCount::mutate(|count| *count += 1);

            // 5. Schedule for expiry, as long as `Pending`, at block with number `expiring_at`.
            if let MaybeBlock::Some(expiring_at) = expiry {
                Self::schedule_pip_for_expiry(id, expiring_at);
            }

            // 6. Record the deposit and as a signal if we have a community PIP.
            if let Proposer::Community(ref proposer) = proposer {
                <Deposits<T>>::insert(id, &proposer, DepositInfo {
                    owner: proposer.clone(),
                    amount: deposit
                });

                // Add vote and update voting counter.
                // INTERNAL: It is impossible to overflow counters in the first vote.
                Self::unsafe_vote(id, proposer.clone(), Vote(true, deposit))
                    .map_err(|vote_error| {
                        debug::error!("The counters of voting (id={}) have an overflow during the 1st vote", id);
                        vote_error
                    })?;

                // Adjust live queue.
                Self::insert_live_queue(id);
            } else {
                CommitteePips::append(id);
            }

            // 7. Emit the event.
            Self::deposit_event(RawEvent::ProposalCreated(
                did,
                proposer,
                id,
                deposit,
                url,
                description,
                expiry,
                proposal_data,
            ));
        }

        /// Vote either in favor (`aye_or_nay` == true) or against a PIP with `id`.
        /// The "convinction" or strength of the vote is given by `deposit`, which is reserved.
        ///
        /// Note that `vote` is *not* additive.
        /// That is, `vote(id, true, 50)` followed by `vote(id, true, 40)`
        /// will first reserve `50` and then refund `50 - 10`, ending up with `40` in deposit.
        /// To add atop of existing votes, you'll need `existing_deposit + addition`.
        ///
        /// # Arguments
        /// * `id`, proposal id
        /// * `aye_or_nay`, a bool representing for or against vote
        /// * `deposit`, the "conviction" with which the vote is made.
        ///
        /// # Errors
        /// * `NoSuchProposal` if `id` doesn't reference a valid PIP.
        /// * `NotFromCommunity` if proposal was made by a committee.
        /// * `IncorrectProposalState` if PIP isn't pending.
        /// * `InsufficientDeposit` if `origin` cannot reserve `deposit - old_deposit`.
        #[weight = <T as Trait>::WeightInfo::vote()]
        pub fn vote(origin, id: PipId, aye_or_nay: bool, deposit: BalanceOf<T>) {
            let voter = ensure_signed(origin)?;
            let pip = Self::proposals(id).ok_or(Error::<T>::NoSuchProposal)?;

            // Proposal must be from the community.
            let proposer = match pip.proposer {
                Proposer::Committee(_) => return Err(Error::<T>::NotFromCommunity.into()),
                Proposer::Community(p) => p,
            };

            if proposer == voter {
                // a) Deposit must be above minimum.
                // Note that proposer can still vote against their own PIP.
                ensure!(deposit >= Self::min_proposal_deposit(), Error::<T>::IncorrectDeposit);
            }

            // Proposal must be pending.
            Self::is_proposal_state(id, ProposalState::Pending)?;

            let current_did = Self::current_did_or_missing()?;

            let old_res = Self::aggregate_result(id);

            with_transaction(|| {
                // Reserve the deposit, or refund if needed.
                let curr_deposit = Self::deposits(id, &voter).amount;
                if deposit < curr_deposit {
                    Self::reduce_lock(&voter, curr_deposit - deposit)?;
                } else {
                    Self::increase_lock(&voter, deposit - curr_deposit)?;
                }
                // Save the vote.
                Self::unsafe_vote(id, voter.clone(), Vote(aye_or_nay, deposit))
            })?;

            // Adjust live queue.
            Self::adjust_live_queue(id, old_res);

            <Deposits<T>>::insert(id, &voter, DepositInfo {
                owner: voter.clone(),
                amount: deposit,
            });

            // Emit event.
            Self::deposit_event(RawEvent::Voted(current_did, voter, id, aye_or_nay, deposit));
        }

        /// Approves the pending committee PIP given by the `id`.
        ///
        /// # Errors
        /// * `BadOrigin` unless a GC voting majority executes this function.
        /// * `NoSuchProposal` if the PIP with `id` doesn't exist.
        /// * `IncorrectProposalState` if the proposal isn't pending.
        /// * `NotByCommittee` if the proposal isn't by a committee.
        #[weight = <T as Trait>::WeightInfo::approve_committee_proposal()]
        pub fn approve_committee_proposal(origin, id: PipId) {
            // 1. Only GC can do this.
            T::VotingMajorityOrigin::ensure_origin(origin)?;

            // 2. Proposal must be pending.
            Self::is_proposal_state(id, ProposalState::Pending)?;

            // 3. Proposal must be by committee.
            let pip = Self::proposals(id).ok_or_else(|| Error::<T>::NoSuchProposal)?;
            ensure!(matches!(pip.proposer, Proposer::Committee(_)), Error::<T>::NotByCommittee);

            // 4. All is good, schedule PIP for execution.
            Self::schedule_pip_for_execution(GC_DID, id, None);
        }

        /// Rejects the PIP given by the `id`, refunding any bonded funds,
        /// assuming it hasn't been cancelled or executed.
        /// Note that proposals scheduled-for-execution can also be rejected.
        ///
        /// # Errors
        /// * `BadOrigin` unless a GC voting majority executes this function.
        /// * `NoSuchProposal` if the PIP with `id` doesn't exist.
        /// * `IncorrectProposalState` if the proposal was cancelled or executed.
        #[weight = <T as Trait>::WeightInfo::reject_proposal()]
        pub fn reject_proposal(origin, id: PipId) {
            T::VotingMajorityOrigin::ensure_origin(origin)?;
            let proposal = Self::proposals(id).ok_or_else(|| Error::<T>::NoSuchProposal)?;
            ensure!(Self::is_active(proposal.state), Error::<T>::IncorrectProposalState);
            Self::maybe_unschedule_pip(id, proposal.state);
            Self::maybe_unsnapshot_pip(id, proposal.state);
            Self::unsafe_reject_proposal(GC_DID, id);
        }

        /// Prune the PIP given by the `id`, refunding any funds not already refunded.
        /// The PIP may not be active
        ///
        /// This function is intended for storage garbage collection purposes.
        ///
        /// # Errors
        /// * `BadOrigin` unless a GC voting majority executes this function.
        /// * `NoSuchProposal` if the PIP with `id` doesn't exist.
        /// * `IncorrectProposalState` if the proposal is active.
        #[weight = <T as Trait>::WeightInfo::prune_proposal()]
        pub fn prune_proposal(origin, id: PipId) {
            T::VotingMajorityOrigin::ensure_origin(origin)?;
            let proposal = Self::proposals(id).ok_or(Error::<T>::NoSuchProposal)?;
            ensure!(!Self::is_active(proposal.state), Error::<T>::IncorrectProposalState);
            Self::prune_data(GC_DID, id, proposal.state, true);
        }

        /// Updates the execution schedule of the PIP given by `id`.
        ///
        /// # Arguments
        /// * `until` defines the future block where the enactment period will finished.
        ///    `None` value means that enactment period is going to finish in the next block.
        ///
        /// # Errors
        /// * `RescheduleNotByReleaseCoordinator` unless triggered by release coordinator.
        /// * `IncorrectProposalState` unless the proposal was in a scheduled state.
        #[weight = <T as Trait>::WeightInfo::reschedule_execution()]
        pub fn reschedule_execution(origin, id: PipId, until: Option<T::BlockNumber>) {
            let did = Identity::<T>::ensure_perms(origin)?;

            // 1. Only release coordinator
            ensure!(
                Some(did) == T::GovernanceCommittee::release_coordinator(),
                Error::<T>::RescheduleNotByReleaseCoordinator
            );

            Self::is_proposal_state(id, ProposalState::Scheduled)?;

            // 2. New value should be valid block number.
            let next_block = <system::Module<T>>::block_number() + 1u32.into();
            let new_until = until.unwrap_or(next_block);
            ensure!(new_until >= next_block, Error::<T>::InvalidFutureBlockNumber);

            // 3. Update enactment period & reschedule it.
            <PipToSchedule<T>>::insert(id, new_until);

            // TODO: When we upgrade Substrate to a release containing `reschedule_named` in
            // `schedule::Named`, use that instead of discrete unscheduling and scheduling.
            Self::unschedule_pip(id);
            Self::schedule_pip_for_execution(GC_DID, id, Some(new_until));
        }

        /// Clears the snapshot and emits the event `SnapshotCleared`.
        ///
        /// # Errors
        /// * `NotACommitteeMember` - triggered when a non-GC-member executes the function.
        #[weight = <T as Trait>::WeightInfo::clear_snapshot()]
        pub fn clear_snapshot(origin) {
            // 1. Check that a GC member is executing this.
            let did = Identity::<T>::ensure_perms(origin)?;
            ensure!(T::GovernanceCommittee::is_member(&did), Error::<T>::NotACommitteeMember);

            if let Some(meta) = <SnapshotMeta<T>>::get() {
                // 2. Clear the snapshot.
                <SnapshotMeta<T>>::kill();
                <SnapshotQueue<T>>::kill();

                // 3. Emit event.
                Self::deposit_event(RawEvent::SnapshotCleared(did, meta.id));
            }
        }

        /// Takes a new snapshot of the current list of active && pending PIPs.
        /// The PIPs are then sorted into a priority queue based on each PIP's weight.
        ///
        /// # Errors
        /// * `NotACommitteeMember` - triggered when a non-GC-member executes the function.
        #[weight = <T as Trait>::WeightInfo::snapshot()]
        pub fn snapshot(origin) {
            // Ensure a GC member is executing this.
            let PermissionedCallOriginData {
                sender: made_by,
                primary_did: did,
                ..
            } = Identity::<T>::ensure_origin_call_permissions(origin)?;
            ensure!(T::GovernanceCommittee::is_member(&did), Error::<T>::NotACommitteeMember);

            // Commit the new snapshot.
            let id = SnapshotIdSequence::mutate(|id| mem::replace(id, *id + 1));
            let created_at = <system::Module<T>>::block_number();
            <SnapshotMeta<T>>::set(Some(SnapshotMetadata { created_at, made_by, id }));
            let queue = <LiveQueue<T>>::get();
            <SnapshotQueue<T>>::set(queue.clone());

            // Emit event.
            Self::deposit_event(RawEvent::SnapshotTaken(did, id, queue));
        }

        /// Enacts `results` for the PIPs in the snapshot queue.
        /// The snapshot will be available for further enactments until it is cleared.
        ///
        /// The `results` are encoded a list of `(id, result)` where `result` is applied to `id`.
        /// Note that the snapshot priority queue is encoded with the *lowest priority first*.
        /// so `results = [(id, Approve)]` will approve `SnapshotQueue[SnapshotQueue.len() - 1]`.
        ///
        /// # Errors
        /// * `BadOrigin` - unless a GC voting majority executes this function.
        /// * `CannotSkipPip` - a given PIP has already been skipped too many times.
        /// * `SnapshotResultTooLarge` - on len(results) > len(snapshot_queue).
        /// * `SnapshotIdMismatch` - if:
        ///   ```text
        ///    ∃ (i ∈ 0..SnapshotQueue.len()).
        ///      results[i].0 ≠ SnapshotQueue[SnapshotQueue.len() - i].id
        ///   ```
        ///    This is protects against clearing queue while GC is voting.
        #[weight = {
            use SnapshotResult::*;

            let mut approves = 0;
            let mut rejects = 0;
            let mut skips = 0;
            for r in results.iter().map(|result| result.1) {
                match r {
                    Approve => approves += 1,
                    Reject => rejects += 1,
                    Skip => skips += 1,
                }
            }
            let weight = <T as Trait>::WeightInfo::enact_snapshot_results(approves, rejects, skips);
            weight.min(PipsEnactSnapshotMaximumWeight::get())
        }]
        pub fn enact_snapshot_results(origin, results: Vec<(PipId, SnapshotResult)>) -> DispatchResult {
            T::VotingMajorityOrigin::ensure_origin(origin)?;

            let max_pip_skip_count = Self::max_pip_skip_count();

            <SnapshotQueue<T>>::try_mutate(|queue| {
                let mut to_bump_skipped = Vec::new();
                // Default after-first-push capacity is 4, we bump this slightly.
                // Rationale: GC are humans sitting together and reaching conensus.
                // This is time consuming, so considering 20 PIPs in total might take few hours.
                let speculative_capacity = queue.len().min(results.len()).min(10);
                let mut to_reject = Vec::with_capacity(speculative_capacity);
                let mut to_approve = Vec::with_capacity(speculative_capacity);

                // Go over each result...
                for (id, action) in results.iter().copied() {
                    match queue.pop() { // ...and "zip" with the queue in reverse.
                        // An action is missing a corresponding PIP in the queue, bail!
                        None => return Err(Error::<T>::SnapshotResultTooLarge.into()),
                        // The id at queue position vs. results mismatches.
                        Some(p) if p.id != id => return Err(Error::<T>::SnapshotIdMismatch.into()),
                        // All is right...
                        Some(_) => {}
                    }
                    match action {
                        // Make sure the PIP can be skipped and enqueue bumping of skip.
                        SnapshotResult::Skip => {
                            let count = PipSkipCount::get(id);
                            ensure!(count < max_pip_skip_count, Error::<T>::CannotSkipPip);
                            to_bump_skipped.push((id, count + 1));
                        }
                        // Mark PIP as rejected.
                        SnapshotResult::Reject => to_reject.push(id),
                        // Approve PIP.
                        SnapshotResult::Approve => to_approve.push(id),
                    }
                }

                // Update skip counts.
                for (pip_id, new_count) in to_bump_skipped.iter().copied() {
                    PipSkipCount::insert(pip_id, new_count);
                    Self::deposit_event(RawEvent::PipSkipped(GC_DID, pip_id, new_count));
                }

                // Adjust the live queue, removing scheduled and rejected PIPs.
                <LiveQueue<T>>::mutate(|live| {
                    live.retain(|e| !(to_reject.contains(&e.id) || to_approve.contains(&e.id)));
                });

                // Reject proposals as instructed & refund.
                for pip_id in to_reject.iter().copied() {
                    Self::unsafe_reject_proposal(GC_DID, pip_id);
                }

                // Approve proposals as instructed.
                for pip_id in to_approve.iter().copied() {
                    Self::schedule_pip_for_execution(GC_DID, pip_id, None);
                }

                let id = Self::snapshot_metadata().map(|m| m.id);
                let event = RawEvent::SnapshotResultsEnacted(GC_DID, id, to_bump_skipped, to_reject, to_approve);
                Self::deposit_event(event);

                Ok(())
            })
        }

        /// Internal dispatchable that handles execution of a PIP.
        #[weight = <T as Trait>::WeightInfo::execute_scheduled_pip()]
        pub fn execute_scheduled_pip(origin, id: PipId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            <PipToSchedule<T>>::remove(id);
            Self::execute_proposal(id)
        }

        /// Internal dispatchable that handles expiration of a PIP.
        #[weight = <T as Trait>::WeightInfo::expire_scheduled_pip()]
        pub fn expire_scheduled_pip(origin, did: IdentityId, id: PipId) {
            ensure_root(origin)?;
            if Self::is_proposal_state(id, ProposalState::Pending).is_ok() {
                Self::maybe_unsnapshot_pip(id, ProposalState::Pending);
                Self::maybe_prune(did, id, ProposalState::Expired);
            }
        }
    }
}

impl<T: Trait> Module<T> {
    fn config<SV, X, E>(origin: T::Origin, new: X, event: E) -> DispatchResult
    where
        SV: StorageValue<X, Query = X>,
        X: FullCodec + Clone,
        E: FnOnce(IdentityId, X, X) -> Event<T>,
    {
        ensure_root(origin)?;
        let prev = SV::mutate(|slot| mem::replace(slot, new.clone()));
        Self::deposit_event(event(GC_DID, prev, new));
        Ok(())
    }

    /// Ensure that `origin` represents one of:
    /// - a signed extrinsic (i.e. transaction), and infer the account id, as a community proposer.
    /// - a committee, where the committee is also inferred.
    ///
    /// Returns the inferred proposer.
    ///
    /// # Errors
    /// * `BadOrigin` if not a signed extrinsic.
    fn ensure_infer_proposer(
        origin: T::Origin,
    ) -> Result<Proposer<T::AccountId>, sp_runtime::traits::BadOrigin> {
        ensure_signed(origin.clone())
            .map(Proposer::Community)
            .or_else(|_| {
                T::TechnicalCommitteeVMO::ensure_origin(origin.clone())
                    .map(|_| Committee::Technical)
                    .or_else(|_| {
                        T::UpgradeCommitteeVMO::ensure_origin(origin).map(|_| Committee::Upgrade)
                    })
                    .map(Proposer::Committee)
            })
    }

    /// Returns the current identity or emits `MissingCurrentIdentity`.
    fn current_did_or_missing() -> Result<IdentityId, Error<T>> {
        Context::current_identity::<Identity<T>>().ok_or_else(|| Error::<T>::MissingCurrentIdentity)
    }

    /// Rejects the given `id`, refunding the deposit, and possibly pruning the proposal's data.
    fn unsafe_reject_proposal(did: IdentityId, id: PipId) {
        Self::maybe_prune(did, id, ProposalState::Rejected);
    }

    /// Refunds any tokens used to vote or bond a proposal.
    ///
    /// This operation is idempotent wrt. chain state,
    /// i.e., once run, refunding again will refund nothing.
    fn refund_proposal(did: IdentityId, id: PipId) {
        let total_refund =
            <Deposits<T>>::iter_prefix_values(id).fold(0u32.into(), |acc, depo_info| {
                Self::reduce_lock(&depo_info.owner, depo_info.amount).unwrap();
                depo_info.amount.saturating_add(acc)
            });
        <Deposits<T>>::remove_prefix(id);
        Self::deposit_event(RawEvent::ProposalRefund(did, id, total_refund));
    }

    /// Unschedule PIP with given `id` if it's scheduled for execution.
    fn maybe_unschedule_pip(id: PipId, state: ProposalState) {
        if let ProposalState::Scheduled = state {
            Self::unschedule_pip(id);
        }
    }

    /// Remove the PIP with `id` from the `ExecutionSchedule` at `block_no`.
    fn unschedule_pip(id: PipId) {
        <PipToSchedule<T>>::remove(id);
        if let Err(_) = T::Scheduler::cancel_named(Self::pip_execution_name(id)) {
            Self::deposit_event(RawEvent::ExecutionCancellingFailed(id));
        }
    }

    /// Remove the PIP with `id` from the snapshot if it is there.
    fn maybe_unsnapshot_pip(id: PipId, state: ProposalState) {
        if let ProposalState::Pending = state {
            // Pending so therefore in live queue; evict `id`.
            <LiveQueue<T>>::mutate(|queue| queue.retain(|i| i.id != id));

            if <SnapshotMeta<T>>::get().is_some() {
                // Proposal is pending and wasn't when snapshot was made.
                // Hence, it is in the snapshot and filtering it out will have an effect.
                // Note: These checks are not strictly necessary, but are done to avoid work.
                <SnapshotQueue<T>>::mutate(|queue| queue.retain(|i| i.id != id));
            }
        }
    }

    /// Prunes (nearly) all data associated with a proposal, removing it from storage.
    ///
    /// For efficiency, some data (e.g., re. execution schedules) is not removed in this function,
    /// but is removed in functions executing this one.
    fn prune_data(did: IdentityId, id: PipId, state: ProposalState, prune: bool) {
        Self::refund_proposal(did, id);
        Self::decrement_count_if_active(state);
        if prune {
            <ProposalResult<T>>::remove(id);
            <ProposalVotes<T>>::remove_prefix(id);
            <ProposalMetadata<T>>::remove(id);
            if let Some(Proposer::Committee(_)) = Self::proposals(id).map(|p| p.proposer) {
                CommitteePips::mutate(|list| list.retain(|&i| i != id));
            }
            <Proposals<T>>::remove(id);
            PipSkipCount::remove(id);
        }
        Self::deposit_event(RawEvent::PipClosed(did, id, prune));
    }

    /// First set the state to `new_state`
    /// and then possibly prune (nearly) all the PIP data, if configuration allows.
    fn maybe_prune(did: IdentityId, id: PipId, new_state: ProposalState) {
        Self::update_proposal_state(did, id, new_state);
        Self::prune_data(did, id, new_state, Self::prune_historical_pips());
    }

    /// Adds a PIP execution call to the PIP execution schedule.
    // TODO: the `maybe_at` argument is only required until we upgrade to a version of Substrate
    // containing `schedule::Named::reschedule_named`.
    fn schedule_pip_for_execution(did: IdentityId, id: PipId, maybe_at: Option<T::BlockNumber>) {
        let at = maybe_at.unwrap_or_else(|| {
            let period = Self::default_enactment_period();
            // The enactment period is at least 1 block. This is de to the fact that it's only
            // possible to schedule calls for future blocks.
            let corrected_period = if period > Zero::zero() {
                period
            } else {
                One::one()
            };
            <system::Module<T>>::block_number() + corrected_period
        });
        Self::update_proposal_state(did, id, ProposalState::Scheduled);
        <PipToSchedule<T>>::insert(id, at);

        let call = Call::<T>::execute_scheduled_pip(id).into();
        let event = match T::Scheduler::schedule_named(
            Self::pip_execution_name(id),
            DispatchTime::At(at),
            None,
            HARD_DEADLINE,
            RawOrigin::Root.into(),
            call,
        ) {
            Err(_) => RawEvent::ExecutionSchedulingFailed(did, id, at),
            Ok(_) => RawEvent::ExecutionScheduled(did, id, at),
        };
        Self::deposit_event(event);
    }

    /// Adds a PIP expiry call to the PIP expiry schedule.
    fn schedule_pip_for_expiry(id: PipId, at: T::BlockNumber) {
        let did = GC_DID;
        let call = Call::<T>::expire_scheduled_pip(did, id).into();
        let event = match T::Scheduler::schedule_named(
            Self::pip_expiry_name(id),
            DispatchTime::At(at),
            None,
            HARD_DEADLINE,
            RawOrigin::Root.into(),
            call,
        ) {
            Err(_) => RawEvent::ExpirySchedulingFailed(did, id, at),
            Ok(_) => RawEvent::ExpiryScheduled(did, id, at),
        };
        Self::deposit_event(event);
    }

    /// Execute the PIP given by `id`.
    /// Panics if the PIP doesn't exist or isn't scheduled.
    fn execute_proposal(id: PipId) -> DispatchResultWithPostInfo {
        let proposal = Self::proposals(id).ok_or(Error::<T>::ScheduledProposalDoesntExist)?;
        ensure!(
            proposal.state == ProposalState::Scheduled,
            Error::<T>::ProposalNotInScheduledState
        );
        let res = proposal.proposal.dispatch(system::RawOrigin::Root.into());
        let weight = res.unwrap_or_else(|e| e.post_info).actual_weight;
        let new_state = res.map_or(ProposalState::Failed, |_| ProposalState::Executed);
        let did = Context::current_identity::<Identity<T>>().unwrap_or_default();
        Self::maybe_prune(did, id, new_state);
        Ok(Some(weight.unwrap_or(0)).into())
    }

    /// Update the proposal state of `did` setting it to `new_state`.
    fn update_proposal_state(
        did: IdentityId,
        id: PipId,
        new_state: ProposalState,
    ) -> ProposalState {
        <Proposals<T>>::mutate(id, |proposal| {
            if let Some(ref mut proposal) = proposal {
                if (proposal.state, new_state) != (ProposalState::Pending, ProposalState::Scheduled)
                {
                    Self::decrement_count_if_active(proposal.state);
                }
                proposal.state = new_state;
            }
        });
        Self::deposit_event(RawEvent::ProposalStateUpdated(did, id, new_state));
        new_state
    }

    /// Returns `Ok(_)` iff `id` has `state`.
    fn is_proposal_state(id: PipId, state: ProposalState) -> DispatchResult {
        let proposal = Self::proposals(id).ok_or(Error::<T>::NoSuchProposal)?;
        ensure!(proposal.state == state, Error::<T>::IncorrectProposalState);
        Ok(())
    }

    /// Returns `true` if `state` is `Pending | Scheduled`.
    fn is_active(state: ProposalState) -> bool {
        matches!(state, ProposalState::Pending | ProposalState::Scheduled)
    }

    /// Decrement active proposal count if `state` signifies it is active.
    fn decrement_count_if_active(state: ProposalState) {
        if Self::is_active(state) {
            // The performance impact of a saturating sub is negligible and caution is good.
            ActivePipCount::mutate(|count| *count = count.saturating_sub(1));
        }
    }

    /// Converts a PIP ID into a name of a PIP scheduled for execution.
    fn pip_execution_name(id: PipId) -> Vec<u8> {
        Self::pip_schedule_name(&PIP_EXECUTION[..], id)
    }

    /// Converts a PIP ID into a name of a PIP scheduled for expiry.
    fn pip_expiry_name(id: PipId) -> Vec<u8> {
        Self::pip_schedule_name(&PIP_EXPIRY[..], id)
    }

    /// Common method to create a unique name for the scheduler for a PIP.
    fn pip_schedule_name(prefix: &[u8], id: PipId) -> Vec<u8> {
        let mut name = Vec::with_capacity(prefix.len() + id.size_hint());
        name.extend_from_slice(prefix);
        id.encode_to(&mut name);
        name
    }
}

impl<T: Trait> Module<T> {
    /// Increase `acc`'s locked deposit for all PIPs by `amount`,
    /// or fail if there's not enough free balance after adding `amount` to lock.
    fn increase_lock(acc: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
        <T as Trait>::Currency::increase_lock(
            PIPS_LOCK_ID,
            acc,
            amount,
            WithdrawReasons::all(),
            |sum| {
                <T as Trait>::Currency::free_balance(acc)
                    .checked_sub(&sum)
                    .ok_or(Error::<T>::InsufficientDeposit.into())
                    .map(drop)
            },
        )
    }

    /// Reduce `acc`'s locked deposit for all PIPs by `amount`,
    /// or fail if `amount` hasn't been locked for PIPs.
    fn reduce_lock(acc: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
        <T as Trait>::Currency::reduce_lock(PIPS_LOCK_ID, acc, amount)
    }

    /// Retrieve votes for a proposal represented by PipId `id`.
    pub fn get_votes(id: PipId) -> VoteCount<BalanceOf<T>>
    where
        T: Send + Sync,
        BalanceOf<T>: Send + Sync,
    {
        if !<ProposalResult<T>>::contains_key(id) {
            return VoteCount::ProposalNotFound;
        }

        let voting = Self::proposal_result(id);
        VoteCount::ProposalFound {
            ayes: voting.ayes_stake,
            nays: voting.nays_stake,
        }
    }

    /// Retrieve proposals made by `proposer`.
    pub fn proposed_by(proposer: Proposer<T::AccountId>) -> Vec<PipId> {
        <Proposals<T>>::iter()
            .filter(|(_, pip)| pip.proposer == proposer)
            .map(|(_, pip)| pip.id)
            .collect()
    }

    /// Retrieve proposals `address` voted on
    pub fn voted_on(address: T::AccountId) -> Vec<PipId> {
        <Proposals<T>>::iter()
            .filter_map(|(_, pip)| Self::proposal_vote(pip.id, &address).map(|_| pip.id))
            .collect::<Vec<_>>()
    }

    /// Retrieve historical voting of `who` account.
    pub fn voting_history_by_address(
        who: T::AccountId,
    ) -> HistoricalVotingByAddress<Vote<BalanceOf<T>>> {
        <Proposals<T>>::iter()
            .filter_map(|(_, pip)| {
                Some(VoteByPip {
                    pip: pip.id,
                    vote: Self::proposal_vote(pip.id, &who)?,
                })
            })
            .collect::<Vec<_>>()
    }

    /// Retrieve historical voting of `who` identity.
    /// It fetches all its keys recursively and it returns the voting history for each of them.
    pub fn voting_history_by_id(
        who: IdentityId,
    ) -> HistoricalVotingById<T::AccountId, Vote<BalanceOf<T>>> {
        let flatten_keys = <Identity<T>>::flatten_keys(who, 1);
        flatten_keys
            .into_iter()
            .map(|key| (key.clone(), Self::voting_history_by_address(key)))
            .collect::<HistoricalVotingById<_, _>>()
    }

    /// Returns the id to use for the next PIP to be made.
    /// Invariant: `next_pip_id() == next_pip_id() + 1`.
    fn next_pip_id() -> u32 {
        <PipIdSequence>::mutate(|id| mem::replace(id, *id + 1))
    }

    /// Changes the vote of `voter` to `vote`, if any.
    fn unsafe_vote(id: PipId, voter: T::AccountId, vote: Vote<BalanceOf<T>>) -> DispatchResult {
        let mut stats = Self::proposal_result(id);

        // Update the vote and get the old one, if any, in which case also remove it from stats.
        if let Some(Vote(direction, deposit)) = <ProposalVotes<T>>::get(id, voter.clone()) {
            let (count, stake) = match direction {
                true => (&mut stats.ayes_count, &mut stats.ayes_stake),
                false => (&mut stats.nays_count, &mut stats.nays_stake),
            };
            *count -= 1;
            *stake -= deposit;
        }

        // Add new vote to stats.
        let Vote(direction, deposit) = vote;
        let (count, stake) = match direction {
            true => (&mut stats.ayes_count, &mut stats.ayes_stake),
            false => (&mut stats.nays_count, &mut stats.nays_stake),
        };
        *count = count
            .checked_add(1)
            .ok_or(Error::<T>::NumberOfVotesExceeded)?;
        *stake = stake
            .checked_add(&deposit)
            .ok_or(Error::<T>::StakeAmountOfVotesExceeded)?;

        // Commit all changes.
        <ProposalResult<T>>::insert(id, stats);
        <ProposalVotes<T>>::insert(id, voter, vote);

        Ok(())
    }

    /// Construct a `SnapshottedPip` from a `PipId`.
    /// `true` denotes a positive sign.
    fn aggregate_result(id: PipId) -> SnapshottedPip<BalanceOf<T>> {
        let VotingResult {
            ayes_stake,
            nays_stake,
            ..
        } = <ProposalResult<T>>::get(id);
        let weight = if ayes_stake >= nays_stake {
            (true, ayes_stake - nays_stake)
        } else {
            (false, nays_stake - ayes_stake)
        };
        SnapshottedPip { id, weight }
    }

    /// Adjust the live queue under the assumption that `id` should be moved up or down the queue.
    fn adjust_live_queue(id: PipId, old: SnapshottedPip<BalanceOf<T>>) {
        let new = Self::aggregate_result(id);
        <LiveQueue<T>>::mutate(|queue| {
            // Remove the old element.
            //
            // Under normal conditions, we can assume its in the list and findable,
            // as the list is sorted, updated, and old is taken before modification.
            // However, we still prefer to be defensive here, and same below.
            if let Ok(old_pos) = queue.binary_search_by(|res| compare_spip(res, &old)) {
                queue.remove(old_pos);
            }

            // Insert the new element.
            if let Err(new_pos) = queue.binary_search_by(|res| compare_spip(res, &new)) {
                queue.insert(new_pos, new);
            }
        });
    }

    /// Insert a new PIP into the live queue.
    ///
    /// The `id` should not exist in the queue previously.
    /// Panics if it did.
    fn insert_live_queue(id: PipId) {
        let new = Self::aggregate_result(id);
        <LiveQueue<T>>::mutate(|queue| {
            // Inserting a new PIP entails that `id` is nowhere to be found.
            // It follows that binary search will return `Err(_)`.
            let pos = queue
                .binary_search_by(|res| compare_spip(res, &new))
                .unwrap_err();
            queue.insert(pos, new);
        });
    }

    /// Recompute the live queue from all existing PIPs.
    pub fn compute_live_queue() -> Vec<SnapshottedPip<BalanceOf<T>>> {
        // Fetch intersection of pending && aggregate their votes.
        let mut queue = <Proposals<T>>::iter_values()
            // Only keep pending community PIPs.
            .filter(|pip| matches!(pip.state, ProposalState::Pending))
            .filter(|pip| matches!(pip.proposer, Proposer::Community(_)))
            .map(|pip| pip.id)
            // Aggregate the votes; `true` denotes a positive sign.
            .map(Self::aggregate_result)
            .collect::<Vec<_>>();

        // Sort pips into priority queue, with highest priority *last*.
        // Having higher prio last allows efficient tail popping, so we have a LIFO structure.
        queue.sort_unstable_by(compare_spip);

        queue
    }

    /// Returns a reportable representation of a proposal taking care that the reported data are not
    /// too large.
    fn reportable_proposal_data(proposal: &T::Proposal) -> ProposalData {
        let encoded_proposal = proposal.encode();
        let proposal_data = if encoded_proposal.len() > PIP_MAX_REPORTING_SIZE {
            ProposalData::Hash(BlakeTwo256::hash(encoded_proposal.as_slice()))
        } else {
            ProposalData::Proposal(encoded_proposal)
        };
        proposal_data
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn compare_spip_works() {
        let mk = |id, sign, power| super::SnapshottedPip {
            id,
            weight: (sign, power),
        };
        let a = mk(4, true, 50);
        let b = mk(3, true, 50);
        let c = mk(5, true, 50);
        let d = mk(6, false, 0);
        let e = mk(7, true, 0);
        let f = mk(8, false, 50);
        let g = mk(9, true, 100);
        let mut queue = vec![a, c, d, b, e, g, f];
        queue.sort_unstable_by(super::compare_spip);
        assert_eq!(queue, vec![f, d, e, c, a, b, g]);
    }
}
