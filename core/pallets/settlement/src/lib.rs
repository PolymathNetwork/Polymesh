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

//! # Settlement Module
//!
//! Settlement module manages all kinds of transfers and settlements of assets
//!
//! ## Overview
//!
//! The settlement module provides functionality to settle onchain as well as offchain trades between multiple parties.
//! All trades are settled under venues. A token issuer can allow/block certain venues from settling trades that involve their tokens.
//! An atomic settlement is called an Instruction. An instruction can contain multiple legs. Legs are essentially simple one to one transfers.
//! When an instruction is settled, either all legs are executed successfully or none are. In other words, if one of the leg fails due to
//! compliance failure, all other legs will also fail.
//!
//! An instruction must be authorized by all the counter parties involved for it to be executed.
//! An instruction can be set to automatically execute in the next block when all authorizations are received or at a particular block number.
//!
//! Offchain settlements are represented via receipts. If a leg has a receipt attached to it, it will not be executed onchain.
//! All other legs will be executed onchain during settlement.
//!
//! ## Dispatchable Functions
//!
//! - `create_venue` - Registers a new venue.
//! - `add_instruction` - Adds a new instruction.
//! - `affirm_instruction` - Provides affirmation to an existing instruction.
//! - `withdraw_affirmation` - Withdraw an existing affirmation to given instruction.
//! - `reject_instruction` - Rejects an existing instruction.
//! - `claim_receipt` - Claims a signed receipt.
//! - `unclaim_receipt` - Unclaims a previously claimed receipt.
//! - `set_venue_filtering` - Enables or disabled venue filtering for a token.
//! - `allow_venues` - Allows additional venues to create instructions involving an asset.
//! - `disallow_venues` - Revokes permission given to venues for creating instructions involving a particular asset.
//!
#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]
#![feature(const_option)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

use codec::{Decode, Encode};
use core::mem;
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure, storage,
    traits::schedule::{DispatchTime, Named as ScheduleNamed},
    weights::Weight,
    IterableStorageDoubleMap, StorageHasher, Twox128,
};
use frame_system::{self as system, ensure_root, RawOrigin};
use pallet_asset as asset;
use pallet_identity::{self as identity, PermissionedCallOriginData};
use polymesh_common_utilities::{
    constants::{
        queue_priority::SETTLEMENT_INSTRUCTION_EXECUTION_PRIORITY,
        schedule_name_prefix::SETTLEMENT_INSTRUCTION_EXECUTION,
    },
    traits::{identity::Trait as IdentityTrait, portfolio::PortfolioSubTrait, CommonTrait},
    with_transaction,
    SystematicIssuers::Settlement as SettlementDID,
};
use polymesh_primitives::{
    storage_migrate_on, storage_migration_ver, IdentityId, PortfolioId, SecondaryKey, Ticker,
};
use polymesh_primitives_derive::VecU8StrongTyped;
use sp_runtime::traits::{One, Verify, Zero};
use sp_std::{collections::btree_set::BTreeSet, convert::TryFrom, prelude::*};

type Identity<T> = identity::Module<T>;
type System<T> = frame_system::Module<T>;
type Asset<T> = asset::Module<T>;

pub trait Trait:
    frame_system::Trait
    + CommonTrait
    + IdentityTrait
    + pallet_timestamp::Trait
    + asset::Trait
    + pallet_compliance_manager::Trait
{
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// Scheduler of settlement instructions.
    type Scheduler: ScheduleNamed<Self::BlockNumber, Self::SchedulerCall, Self::SchedulerOrigin>;
    /// A call type for identity-mapping the `Call` enum type. Used by the scheduler.
    type SchedulerCall: From<Call<Self>> + Into<<Self as IdentityTrait>::Proposal>;
    /// Weight information for extrinsic of the settlement pallet.
    type WeightInfo: WeightInfo;
}

/// A wrapper for VenueDetails
#[derive(
    Decode, Encode, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, VecU8StrongTyped,
)]
pub struct VenueDetails(Vec<u8>);

/// Status of an instruction
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum InstructionStatus {
    /// Invalid instruction or details pruned
    Unknown,
    /// Instruction is pending execution
    Pending,
}

impl Default for InstructionStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Type of the venue. Used for offchain filtering.
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum VenueType {
    /// Default type - used for mixed and unknown types
    Other,
    /// Represents a primary distribution
    Distribution,
    /// Represents an offering/fund raiser
    Sto,
    /// Represents a match making service
    Exchange,
}

impl Default for VenueType {
    fn default() -> Self {
        Self::Other
    }
}

/// Status of a leg
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LegStatus<AccountId> {
    /// It is waiting for affirmation
    PendingTokenLock,
    /// It is waiting execution (tokens currently locked)
    ExecutionPending,
    /// receipt used, (receipt signer, receipt uid)
    ExecutionToBeSkipped(AccountId, u64),
}

impl<AccountId> Default for LegStatus<AccountId> {
    fn default() -> Self {
        Self::PendingTokenLock
    }
}

/// Status of an affirmation
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AffirmationStatus {
    /// Invalid affirmation
    Unknown,
    /// Pending user's consent
    Pending,
    /// Affirmed by the user
    Affirmed,
    /// Rejected by the user
    Rejected,
}

impl Default for AffirmationStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Type of settlement
#[derive(Encode, Decode, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettlementType<BlockNumber> {
    /// Instruction should be settled in the next block as soon as all affirmations are received.
    SettleOnAffirmation,
    /// Instruction should be settled on a particular block.
    SettleOnBlock(BlockNumber),
}

impl<BlockNumber> Default for SettlementType<BlockNumber> {
    fn default() -> Self {
        Self::SettleOnAffirmation
    }
}

/// Details about an instruction
#[derive(Encode, Decode, Default, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Instruction<Moment, BlockNumber> {
    /// Unique instruction id. It is an auto incrementing number
    pub instruction_id: u64,
    /// Id of the venue this instruction belongs to
    pub venue_id: u64,
    /// Status of the instruction
    pub status: InstructionStatus,
    /// Type of settlement used for this instruction
    pub settlement_type: SettlementType<BlockNumber>,
    /// Date at which this instruction was created
    pub created_at: Option<Moment>,
    /// Date from which this instruction is valid
    pub trade_date: Option<Moment>,
    /// Date after which the instruction should be settled (not enforced)
    pub value_date: Option<Moment>,
}

/// Details of a leg including the leg id in the instruction
#[derive(Encode, Decode, Default, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Leg<Balance> {
    /// Portfolio of the sender
    pub from: PortfolioId,
    /// Portfolio of the receiver
    pub to: PortfolioId,
    /// Ticker of the asset being transferred
    pub asset: Ticker,
    /// Amount being transferred
    pub amount: Balance,
}

/// Details about a venue
#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Venue {
    /// Identity of the venue's creator
    pub creator: IdentityId,
    /// instructions under this venue (Only needed for the UI)
    pub instructions: Vec<u64>,
    /// Additional details about this venue (Only needed for the UI)
    pub details: VenueDetails,
    /// Specifies type of the venue (Only needed for the UI)
    pub venue_type: VenueType,
}

/// Old venue details format. Used only for storage migration.
#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct OldVenue {
    /// Identity of the venue's creator
    pub creator: IdentityId,
    /// instructions under this venue (Only needed for the UI)
    pub instructions: Vec<u64>,
    /// Additional details about this venue (Only needed for the UI)
    pub details: VenueDetails,
}

impl Venue {
    pub fn new(creator: IdentityId, details: VenueDetails, venue_type: VenueType) -> Self {
        Self {
            creator,
            instructions: Vec::new(),
            details,
            venue_type,
        }
    }
}

/// Details about an offchain transaction receipt
#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Receipt<Balance> {
    /// Unique receipt number set by the signer for their receipts
    pub receipt_uid: u64,
    /// Identity of the sender
    pub from: PortfolioId,
    /// Identity of the receiver
    pub to: PortfolioId,
    /// Ticker of the asset being transferred
    pub asset: Ticker,
    /// Amount being transferred
    pub amount: Balance,
}

/// A wrapper for VenueDetails
#[derive(
    Decode, Encode, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, VecU8StrongTyped,
)]
pub struct ReceiptMetadata(Vec<u8>);

/// Details about an offchain transaction receipt that a user must input
#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct ReceiptDetails<AccountId, OffChainSignature> {
    /// Unique receipt number set by the signer for their receipts
    pub receipt_uid: u64,
    /// Target leg id
    pub leg_id: u64,
    /// Signer for this receipt
    pub signer: AccountId,
    /// signature confirming the receipt details
    pub signature: OffChainSignature,
    /// Generic text that can be used to attach messages to receipts
    pub metadata: ReceiptMetadata,
}

pub trait WeightInfo {
    fn create_venue(d: u32, u: u32) -> Weight;
    fn update_venue(d: u32) -> Weight;
    fn add_instruction(u: u32) -> Weight;
    fn add_and_affirm_instruction(u: u32) -> Weight;
    fn affirm_instruction(l: u32) -> Weight;
    fn withdraw_affirmation(u: u32) -> Weight;
    fn reject_instruction(l: u32) -> Weight;
    fn affirm_with_receipts(r: u32) -> Weight;
    fn claim_receipt() -> Weight;
    fn unclaim_receipt() -> Weight;
    fn set_venue_filtering() -> Weight;
    fn allow_venues(u: u32) -> Weight;
    fn disallow_venues(u: u32) -> Weight;
    fn execute_scheduled_instruction(l: u32) -> Weight;
    fn reject_instruction_with_no_pre_affirmations(l: u32) -> Weight;
    fn change_receipt_validity() -> Weight;

    // Some multiple paths based extrinsic.
    // TODO: Will be removed once we get the worst case weight.
    fn set_venue_filtering_disallow() -> Weight;
    fn withdraw_affirmation_with_receipt(u: u32) -> Weight;
    fn add_instruction_with_settle_on_block_type(u: u32) -> Weight;
    fn add_and_affirm_instruction_with_settle_on_block_type(u: u32) -> Weight;
}

decl_event!(
    pub enum Event<T>
    where
        Balance = <T as CommonTrait>::Balance,
        Moment = <T as pallet_timestamp::Trait>::Moment,
        BlockNumber = <T as frame_system::Trait>::BlockNumber,
        AccountId = <T as frame_system::Trait>::AccountId,
    {
        /// A new venue has been created (did, venue_id, details, type)
        VenueCreated(IdentityId, u64, VenueDetails, VenueType),
        /// An existing venue has been updated (did, venue_id, details, type)
        VenueUpdated(IdentityId, u64, VenueDetails, VenueType),
        /// A new instruction has been created
        /// (did, venue_id, instruction_id, settlement_type, trade_date, value_date, legs)
        InstructionCreated(
            IdentityId,
            u64,
            u64,
            SettlementType<BlockNumber>,
            Option<Moment>,
            Option<Moment>,
            Vec<Leg<Balance>>,
        ),
        /// An instruction has been affirmed (did, portfolio, instruction_id)
        InstructionAffirmed(IdentityId, PortfolioId, u64),
        /// An affirmation has been withdrawn (did, portfolio, instruction_id)
        AffirmationWithdrawn(IdentityId, PortfolioId, u64),
        /// An instruction has been rejected (did, instruction_id)
        InstructionRejected(IdentityId, u64),
        /// A receipt has been claimed (did, instruction_id, leg_id, receipt_uid, signer, receipt metadata)
        ReceiptClaimed(IdentityId, u64, u64, u64, AccountId, ReceiptMetadata),
        /// A receipt has been invalidated (did, signer, receipt_uid, validity)
        ReceiptValidityChanged(IdentityId, AccountId, u64, bool),
        /// A receipt has been unclaimed (did, instruction_id, leg_id, receipt_uid, signer)
        ReceiptUnclaimed(IdentityId, u64, u64, u64, AccountId),
        /// Venue filtering has been enabled or disabled for a ticker (did, ticker, filtering_enabled)
        VenueFiltering(IdentityId, Ticker, bool),
        /// Venues added to allow list (did, ticker, vec<venue_id>)
        VenuesAllowed(IdentityId, Ticker, Vec<u64>),
        /// Venues added to block list (did, ticker, vec<venue_id>)
        VenuesBlocked(IdentityId, Ticker, Vec<u64>),
        /// Execution of a leg failed (did, instruction_id, leg_id)
        LegFailedExecution(IdentityId, u64, u64),
        /// Instruction failed execution (did, instruction_id)
        InstructionFailed(IdentityId, u64),
        /// Instruction executed successfully(did, instruction_id)
        InstructionExecuted(IdentityId, u64),
        /// Venue unauthorized by ticker owner (did, Ticker, venue_id)
        VenueUnauthorized(IdentityId, Ticker, u64),
        /// Scheduling of instruction fails.
        SchedulingFailed(DispatchError),
    }
);

decl_error! {
    /// Errors for the Settlement module.
    pub enum Error for Module<T: Trait> {
        /// Venue does not exist.
        InvalidVenue,
        /// Sender does not have required permissions.
        Unauthorized,
        /// No pending affirmation for the provided instruction.
        NoPendingAffirm,
        /// Instruction has not been affirmed.
        InstructionNotAffirmed,
        /// Provided instruction is not pending execution.
        InstructionNotPending,
        /// Provided leg is not pending execution.
        LegNotPending,
        /// Signer is not authorized by the venue.
        UnauthorizedSigner,
        /// Receipt already used.
        ReceiptAlreadyClaimed,
        /// Receipt not used yet.
        ReceiptNotClaimed,
        /// Venue does not have required permissions.
        UnauthorizedVenue,
        /// While affirming the transfer, system failed to lock the assets involved.
        FailedToLockTokens,
        /// Instruction failed to execute.
        InstructionFailed,
        /// Instruction has invalid dates
        InstructionDatesInvalid,
        /// Instruction's target settle block reached.
        InstructionSettleBlockPassed,
        /// Offchain signature is invalid.
        InvalidSignature,
        /// Sender and receiver are the same.
        SameSenderReceiver,
        /// Portfolio in receipt does not match with portfolios provided by the user.
        PortfolioMismatch,
        /// The provided settlement block number is in the past and cannot be used by the scheduler.
        SettleOnPastBlock,
        /// Portfolio based actions require at least one portfolio to be provided as input.
        NoPortfolioProvided,
        /// The current instruction affirmation status does not support the requested action.
        UnexpectedAffirmationStatus,
        /// Scheduling of an instruction fails.
        FailedToSchedule,
        /// Legs count should matches with the total number of legs in which given portfolio act as `from_portfolio`.
        LegCountTooSmall
    }
}

// A value placed in storage that represents the current version of the this storage. This value
// is used by the `on_runtime_upgrade` logic to determine whether we run storage migration logic.
storage_migration_ver!(1);

decl_storage! {
    trait Store for Module<T: Trait> as Settlement {
        /// Info about a venue. venue_id -> venue_details
        pub VenueInfo get(fn venue_info): map hasher(twox_64_concat) u64 => Option<Venue>;
        /// Signers allowed by the venue. (venue_id, signer) -> bool
        VenueSigners get(fn venue_signers): double_map hasher(twox_64_concat) u64, hasher(twox_64_concat) T::AccountId => bool;
        /// Array of venues created by an identity. Only needed for the UI. IdentityId -> Vec<venue_id>
        UserVenues get(fn user_venues): map hasher(twox_64_concat) IdentityId => Vec<u64>;
        /// Details about an instruction. instruction_id -> instruction_details
        InstructionDetails get(fn instruction_details): map hasher(twox_64_concat) u64 => Instruction<T::Moment, T::BlockNumber>;
        /// Legs under an instruction. (instruction_id, leg_id) -> Leg
        pub InstructionLegs get(fn instruction_legs): double_map hasher(twox_64_concat) u64, hasher(twox_64_concat) u64 => Leg<T::Balance>;
        /// Status of a leg under an instruction. (instruction_id, leg_id) -> LegStatus
        InstructionLegStatus get(fn instruction_leg_status): double_map hasher(twox_64_concat) u64, hasher(twox_64_concat) u64 => LegStatus<T::AccountId>;
        /// Number of affirmations pending before instruction is executed. instruction_id -> affirm_pending
        InstructionAffirmsPending get(fn instruction_affirms_pending): map hasher(twox_64_concat) u64 => u64;
        /// Tracks affirmations received for an instruction. (instruction_id, counter_party) -> AffirmationStatus
        AffirmsReceived get(fn affirms_received): double_map hasher(twox_64_concat) u64, hasher(twox_64_concat) PortfolioId => AffirmationStatus;
        /// Helps a user track their pending instructions and affirmations (only needed for UI).
        /// (counter_party, instruction_id) -> AffirmationStatus
        UserAffirmations get(fn user_affirmations): double_map hasher(twox_64_concat) PortfolioId, hasher(twox_64_concat) u64 => AffirmationStatus;
        /// Tracks redemption of receipts. (signer, receipt_uid) -> receipt_used
        ReceiptsUsed get(fn receipts_used): double_map hasher(twox_64_concat) T::AccountId, hasher(blake2_128_concat) u64 => bool;
        /// Tracks if a token has enabled filtering venues that can create instructions involving their token. Ticker -> filtering_enabled
        VenueFiltering get(fn venue_filtering): map hasher(blake2_128_concat) Ticker => bool;
        /// Venues that are allowed to create instructions involving a particular ticker. Oly used if filtering is enabled.
        /// (ticker, venue_id) -> allowed
        VenueAllowList get(fn venue_allow_list): double_map hasher(blake2_128_concat) Ticker, hasher(twox_64_concat) u64 => bool;
        /// Number of venues in the system (It's one more than the actual number)
        VenueCounter get(fn venue_counter) build(|_| 1u64): u64;
        /// Number of instructions in the system (It's one more than the actual number)
        InstructionCounter get(fn instruction_counter) build(|_| 1u64): u64;
        /// Storage version.
        StorageVersion get(fn storage_version) build(|_| Version::new(1).unwrap()): Version;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: <T as frame_system::Trait>::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        fn on_runtime_upgrade() -> Weight {

            let storage_ver = StorageVersion::get();
            storage_migrate_on!(storage_ver, 1, {
                // Delete all settlement data that were stored at a wrong prefix.
                let prefix = Twox128::hash(b"StoCapped");
                storage::unhashed::kill_prefix(&prefix);

                // Set venue counter and instruction counter to 1 so that the id(s) start from 1 instead of 0
                VenueCounter::put(1);
                InstructionCounter::put(1);
            });

            1_000
        }

        /// Registers a new venue.
        ///
        /// * `details` - Extra details about a venue
        /// * `signers` - Array of signers that are allowed to sign receipts for this venue
        /// * `venue_type` - Type of venue being created
        ///
        /// # Weight
        /// `200_000_000 + 5_000_000 * signers.len()`
        #[weight = <T as Trait>::WeightInfo::create_venue(details.len() as u32, signers.len() as u32)]
        pub fn create_venue(origin, details: VenueDetails, signers: Vec<T::AccountId>, venue_type: VenueType) {
            let did = Identity::<T>::ensure_perms(origin)?;
            let venue = Venue::new(did, details, venue_type);
            // NB: Venue counter starts with 1.
            let venue_counter = VenueCounter::mutate(|c| mem::replace(c, *c + 1));
            VenueInfo::insert(venue_counter, venue.clone());
            for signer in signers {
                <VenueSigners<T>>::insert(venue_counter, signer, true);
            }
            UserVenues::append(did, venue_counter);
            Self::deposit_event(RawEvent::VenueCreated(did, venue_counter, venue.details, venue.venue_type));
        }

        /// Edit venue details and types.
        /// Both parameters are optional, they will be updated only if Some(value) is provided
        ///
        /// * `venue_id` - ID of the venue to edit
        /// * `details` - Extra details about a venue
        /// * `type` - Type of venue being created
        ///
        /// # Weight
        /// `200_000_000
        #[weight = <T as Trait>::WeightInfo::update_venue(details.as_ref().map( |d| d.len() as u32).unwrap_or_default())]
        pub fn update_venue(origin, venue_id: u64, details: Option<VenueDetails>, typ: Option<VenueType>) -> DispatchResult {
            let did = Identity::<T>::ensure_perms(origin)?;
            VenueInfo::try_mutate(venue_id, |venue| {
                // Ensure venue exists & that DID created it.
                let venue = venue.as_mut().ok_or(Error::<T>::InvalidVenue)?;
                ensure!(venue.creator == did, Error::<T>::Unauthorized);

                // Update details & type.
                if let Some(details) = details {
                    venue.details = details;
                }
                if let Some(typ) = typ {
                    venue.venue_type = typ;
                }

                Self::deposit_event(RawEvent::VenueUpdated(did, venue_id, venue.details.clone(), venue.venue_type.clone()));
                Ok(())
            })
        }

        /// Adds a new instruction.
        ///
        /// # Arguments
        /// * `venue_id` - ID of the venue this instruction belongs to.
        /// * `settlement_type` - Defines if the instruction should be settled
        ///    in the next block after receiving all affirmations or waiting till a specific block.
        /// * `trade_date` - Optional date from which people can interact with this instruction.
        /// * `value_date` - Optional date after which the instruction should be settled (not enforced)
        /// * `legs` - Legs included in this instruction.
        ///
        /// # Weight
        /// `950_000_000 + 1_000_000 * legs.len()`
        #[weight = <T as Trait>::WeightInfo::add_instruction_with_settle_on_block_type(legs.len() as u32)
        .saturating_add(
            <T as Trait>::WeightInfo::execute_scheduled_instruction(legs.len() as u32)
        )]
        pub fn add_instruction(
            origin,
            venue_id: u64,
            settlement_type: SettlementType<T::BlockNumber>,
            trade_date: Option<T::Moment>,
            value_date: Option<T::Moment>,
            legs: Vec<Leg<T::Balance>>
        ) {
            let did = Identity::<T>::ensure_perms(origin)?;
            Self::base_add_instruction(did, venue_id, settlement_type, trade_date, value_date, legs)?;
        }

        /// Adds and affirms a new instruction.
        ///
        /// # Arguments
        /// * `venue_id` - ID of the venue this instruction belongs to.
        /// * `settlement_type` - Defines if the instruction should be settled
        ///    in the next block after receiving all affirmations or waiting till a specific block.
        /// * `trade_date` - Optional date from which people can interact with this instruction.
        /// * `value_date` - Optional date after which the instruction should be settled (not enforced)
        /// * `legs` - Legs included in this instruction.
        /// * `portfolios` - Portfolios that the sender controls and wants to use in this affirmations.
        #[weight = <T as Trait>::WeightInfo::add_and_affirm_instruction_with_settle_on_block_type(legs.len() as u32)
        .saturating_add(
            <T as Trait>::WeightInfo::execute_scheduled_instruction(legs.len() as u32)
        )]
        pub fn add_and_affirm_instruction(
            origin,
            venue_id: u64,
            settlement_type: SettlementType<T::BlockNumber>,
            trade_date: Option<T::Moment>,
            value_date: Option<T::Moment>,
            legs: Vec<Leg<T::Balance>>,
            portfolios: Vec<PortfolioId>
        ) -> DispatchResult {
            let did = Identity::<T>::ensure_perms(origin.clone())?;
            with_transaction(|| {
                let portfolios_set = portfolios.into_iter().collect::<BTreeSet<_>>();
                let legs_count = legs.iter().filter(|l| portfolios_set.contains(&l.from)).count() as u32;
                let instruction_id = Self::base_add_instruction(did, venue_id, settlement_type, trade_date, value_date, legs)?;
                Self::affirm_and_maybe_schedule_instruction(origin, instruction_id, portfolios_set.into_iter(), legs_count)
            })
        }

        /// Provide affirmation to an existing instruction.
        ///
        /// # Arguments
        /// * `instruction_id` - Instruction id to affirm.
        /// * `portfolios` - Portfolios that the sender controls and wants to affirm this instruction.
        /// * `legs` - List of legs needs to affirmed.
        #[weight = <T as Trait>::WeightInfo::affirm_instruction(*max_legs_count as u32)]
        pub fn affirm_instruction(origin, instruction_id: u64, portfolios: Vec<PortfolioId>, max_legs_count: u32) -> DispatchResult {
            Self::affirm_and_maybe_schedule_instruction(origin, instruction_id, portfolios.into_iter(), max_legs_count)
        }

        /// Withdraw an affirmation for a given instruction.
        ///
        /// # Arguments
        /// * `instruction_id` - Instruction id for that affirmation get withdrawn.
        /// * `portfolios` - Portfolios that the sender controls and wants to withdraw affirmation.
        #[weight = <T as Trait>::WeightInfo::withdraw_affirmation(*max_legs_count as u32)]
        pub fn withdraw_affirmation(origin, instruction_id: u64, portfolios: Vec<PortfolioId>, max_legs_count: u32) {
            let (did, secondary_key) = Self::ensure_origin_perm_and_instruction_validity(origin, instruction_id)?;
            let portfolios_set = portfolios.into_iter().collect::<BTreeSet<_>>();

            // Withdraw an affirmation.
            Self::unsafe_withdraw_instruction_affirmation(did, instruction_id, portfolios_set, secondary_key.as_ref(), max_legs_count)?;
            if Self::instruction_details(instruction_id).settlement_type == SettlementType::SettleOnAffirmation {
                // Cancel the scheduled task for the execution of a given instruction.
                let _ = T::Scheduler::cancel_named((SETTLEMENT_INSTRUCTION_EXECUTION, instruction_id).encode());
            }
        }

        /// Rejects an existing instruction.
        ///
        /// # Arguments
        /// * `instruction_id` - Instruction id to reject.
        /// * `portfolios` - Portfolios that the sender controls and wants them to reject this instruction
        #[weight = <T as Trait>::WeightInfo::reject_instruction_with_no_pre_affirmations(*max_legs_count as u32)]
        pub fn reject_instruction(origin, instruction_id: u64, portfolios: Vec<PortfolioId>, max_legs_count: u32) {
            let (did, secondary_key) = Self::ensure_origin_perm_and_instruction_validity(origin, instruction_id)?;
            ensure!(!portfolios.is_empty(), Error::<T>::NoPortfolioProvided);

            let portfolios_set = portfolios.into_iter().collect::<BTreeSet<_>>();

            // If the instruction was affirmed by the portfolio, the affirmation must be withdrawn.
            // The sender must have custodian permission over the portfolio.
            let mut affirmed_portfolios = BTreeSet::new();
            for portfolio in &portfolios_set {
                let user_affirmation_status = Self::user_affirmations(portfolio, instruction_id);
                match user_affirmation_status {
                    AffirmationStatus::Affirmed => { affirmed_portfolios.insert(*portfolio); },
                    AffirmationStatus::Pending => T::Portfolio::ensure_portfolio_custody_and_permission(*portfolio, did, secondary_key.as_ref())?,
                    _ => return Err(Error::<T>::NoPendingAffirm.into())
                };
            }
            let legs_count = Self::unsafe_withdraw_instruction_affirmation(did, instruction_id, affirmed_portfolios, secondary_key.as_ref(), max_legs_count)?;

            // Updates storage to mark the instruction as rejected.
            for portfolio in portfolios_set {
                UserAffirmations::insert(portfolio, instruction_id, AffirmationStatus::Rejected);
                AffirmsReceived::insert(instruction_id, portfolio, AffirmationStatus::Rejected);
            }

            Self::deposit_event(RawEvent::InstructionRejected(did, instruction_id));
            // Schedule the instruction to execute in the next block only if it was meant to be executed on affirmation.
            Self::maybe_schedule_instruction(Zero::zero(), instruction_id, legs_count)
        }

        /// Accepts an instruction and claims a signed receipt.
        ///
        /// # Arguments
        /// * `instruction_id` - Target instruction id.
        /// * `leg_id` - Target leg id for the receipt
        /// * `receipt_uid` - Receipt ID generated by the signer.
        /// * `signer` - Signer of the receipt.
        /// * `signed_data` - Signed receipt.
        /// * `portfolios` - Portfolios that the sender controls and wants to accept this instruction with
        #[weight = <T as Trait>::WeightInfo::affirm_with_receipts(*max_legs_count as u32).max(<T as Trait>::WeightInfo::affirm_instruction(*max_legs_count as u32))]
        pub fn affirm_with_receipts(origin, instruction_id: u64, receipt_details: Vec<ReceiptDetails<T::AccountId, T::OffChainSignature>>, portfolios: Vec<PortfolioId>, max_legs_count: u32) -> DispatchResult {
            Self::affirm_with_receipts_and_maybe_schedule_instruction(origin, instruction_id, receipt_details, portfolios, max_legs_count)
        }

        /// Claims a signed receipt.
        ///
        /// # Arguments
        /// * `instruction_id` - Target instruction id for the receipt.
        /// * `leg_id` - Target leg id for the receipt
        /// * `receipt_uid` - Receipt ID generated by the signer.
        /// * `signer` - Signer of the receipt.
        /// * `signed_data` - Signed receipt.
        #[weight = <T as Trait>::WeightInfo::claim_receipt()]
        pub fn claim_receipt(origin, instruction_id: u64, receipt_details: ReceiptDetails<T::AccountId, T::OffChainSignature>) -> DispatchResult {
            let (primary_did, secondary_key) = Self::ensure_origin_perm_and_instruction_validity(origin, instruction_id)?;
            Self::unsafe_claim_receipt(
                primary_did,
                instruction_id,
                receipt_details,
                secondary_key.as_ref()
            )
        }

        /// Unclaims a previously claimed receipt.
        ///
        /// # Arguments
        /// * `instruction_id` - Target instruction id for the receipt.
        /// * `leg_id` - Target leg id for the receipt
        #[weight = <T as Trait>::WeightInfo::unclaim_receipt()]
        pub fn unclaim_receipt(origin, instruction_id: u64, leg_id: u64) {
            let (did, secondary_key) = Self::ensure_origin_perm_and_instruction_validity(origin, instruction_id)?;

            let (signer, receipt_uid) = match Self::instruction_leg_status(instruction_id, leg_id) {
                LegStatus::ExecutionToBeSkipped(s, r) => (s, r),
                _ => return Err(Error::<T>::ReceiptNotClaimed.into()),
            };
            let leg = Self::instruction_legs(instruction_id, leg_id);
            T::Portfolio::ensure_portfolio_custody_and_permission(leg.from, did, secondary_key.as_ref())?;
            // Lock tokens that are part of the leg
            Self::lock_via_leg(&leg)?;
            <ReceiptsUsed<T>>::insert(&signer, receipt_uid, false);
            <InstructionLegStatus<T>>::insert(instruction_id, leg_id, LegStatus::ExecutionPending);
            Self::deposit_event(RawEvent::ReceiptUnclaimed(did, instruction_id, leg_id, receipt_uid, signer));
        }

        /// Enables or disabled venue filtering for a token.
        ///
        /// # Arguments
        /// * `ticker` - Ticker of the token in question.
        /// * `enabled` - Boolean that decides if the filtering should be enabled.
        #[weight = <T as Trait>::WeightInfo::set_venue_filtering()]
        pub fn set_venue_filtering(origin, ticker: Ticker, enabled: bool) {
            let did = <Asset<T>>::ensure_perms_owner_asset(origin, &ticker)?;
            if enabled {
                VenueFiltering::insert(ticker, enabled);
            } else {
                VenueFiltering::remove(ticker);
            }
            Self::deposit_event(RawEvent::VenueFiltering(did, ticker, enabled));
        }

        /// Allows additional venues to create instructions involving an asset.
        ///
        /// * `ticker` - Ticker of the token in question.
        /// * `venues` - Array of venues that are allowed to create instructions for the token in question.
        ///
        /// # Weight
        /// `200_000_000 + 500_000 * venues.len()`
        #[weight = <T as Trait>::WeightInfo::allow_venues(venues.len() as u32)]
        pub fn allow_venues(origin, ticker: Ticker, venues: Vec<u64>) {
            let did = <Asset<T>>::ensure_perms_owner_asset(origin, &ticker)?;
            for venue in &venues {
                VenueAllowList::insert(&ticker, venue, true);
            }
            Self::deposit_event(RawEvent::VenuesAllowed(did, ticker, venues));
        }

        /// Revokes permission given to venues for creating instructions involving a particular asset.
        ///
        /// * `ticker` - Ticker of the token in question.
        /// * `venues` - Array of venues that are no longer allowed to create instructions for the token in question.
        ///
        /// # Weight
        /// `200_000_000 + 500_000 * venues.len()`
        #[weight = <T as Trait>::WeightInfo::disallow_venues(venues.len() as u32)]
        pub fn disallow_venues(origin, ticker: Ticker, venues: Vec<u64>) {
            let did = <Asset<T>>::ensure_perms_owner_asset(origin, &ticker)?;
            for venue in &venues {
                VenueAllowList::remove(&ticker, venue);
            }
            Self::deposit_event(RawEvent::VenuesBlocked(did, ticker, venues));
        }

        /// Marks a receipt issued by the caller as claimed or not claimed.
        /// This allows the receipt issuer to invalidate an already issued receipt or revalidate an already claimed receipt.
        ///
        /// * `receipt_uid` - Unique ID of the receipt.
        /// * `validity` - New validity of the receipt.
        #[weight = <T as Trait>::WeightInfo::change_receipt_validity()]
        pub fn change_receipt_validity(origin, receipt_uid: u64, validity: bool) {
            let PermissionedCallOriginData {
                primary_did,
                sender: signer,
                ..
            } = Identity::<T>::ensure_origin_call_permissions(origin)?;
            <ReceiptsUsed<T>>::insert(&signer, receipt_uid, !validity);
            Self::deposit_event(RawEvent::ReceiptValidityChanged(primary_did, signer, receipt_uid, validity));
        }

        /// Root callable extrinsic, used as an internal call to execute a scheduled settlement instruction.
        #[weight = <T as Trait>::WeightInfo::execute_scheduled_instruction(*legs_count)]
        fn execute_scheduled_instruction(origin, instruction_id: u64, legs_count: u32) {
            ensure_root(origin)?;
            Self::execute_instruction(instruction_id)?;
        }
    }
}

impl<T: Trait> Module<T> {
    fn lock_via_leg(leg: &Leg<T::Balance>) -> DispatchResult {
        T::Portfolio::lock_tokens(&leg.from, &leg.asset, &leg.amount)
    }

    fn unlock_via_leg(leg: &Leg<T::Balance>) -> DispatchResult {
        T::Portfolio::unlock_tokens(&leg.from, &leg.asset, &leg.amount)
    }

    /// Ensure origin call permission and the given instruction validity.
    fn ensure_origin_perm_and_instruction_validity(
        origin: <T as frame_system::Trait>::Origin,
        instruction_id: u64,
    ) -> Result<(IdentityId, Option<SecondaryKey<T::AccountId>>), DispatchError> {
        let PermissionedCallOriginData {
            primary_did,
            secondary_key,
            ..
        } = Identity::<T>::ensure_origin_call_permissions(origin)?;
        Self::ensure_instruction_validity(instruction_id)?;
        Ok((primary_did, secondary_key))
    }

    pub fn base_add_instruction(
        did: IdentityId,
        venue_id: u64,
        settlement_type: SettlementType<T::BlockNumber>,
        trade_date: Option<T::Moment>,
        value_date: Option<T::Moment>,
        legs: Vec<Leg<T::Balance>>,
    ) -> Result<u64, DispatchError> {
        // Ensure that the scheduled block number is in the future so that `T::Scheduler::schedule_named`
        // doesn't fail.
        if let SettlementType::SettleOnBlock(block_number) = &settlement_type {
            ensure!(
                *block_number > System::<T>::block_number(),
                Error::<T>::SettleOnPastBlock
            );
        }

        // Check if a venue exists and the sender is the creator of the venue
        let mut venue = Self::venue_info(venue_id).ok_or(Error::<T>::InvalidVenue)?;
        ensure!(venue.creator == did, Error::<T>::Unauthorized);

        // Prepare data to store in storage
        let mut counter_parties = BTreeSet::new();
        let mut tickers = BTreeSet::new();
        // This is done to create a list of unique CP and tickers involved in the instruction.
        for leg in &legs {
            ensure!(leg.from != leg.to, Error::<T>::SameSenderReceiver);
            counter_parties.insert(leg.from);
            counter_parties.insert(leg.to);
            tickers.insert(leg.asset);
        }

        // Check if the venue has required permissions from token owners
        for ticker in &tickers {
            if Self::venue_filtering(ticker) {
                ensure!(
                    Self::venue_allow_list(ticker, venue_id),
                    Error::<T>::UnauthorizedVenue
                );
            }
        }

        // NB Instruction counter starts from 1
        let instruction_counter = Self::instruction_counter();
        let instruction = Instruction {
            instruction_id: instruction_counter,
            venue_id,
            status: InstructionStatus::Pending,
            settlement_type,
            created_at: Some(<pallet_timestamp::Module<T>>::get()),
            trade_date,
            value_date,
        };

        // write data to storage
        for counter_party in &counter_parties {
            UserAffirmations::insert(
                counter_party,
                instruction_counter,
                AffirmationStatus::Pending,
            );
        }

        for (i, leg) in legs.iter().enumerate() {
            <InstructionLegs<T>>::insert(
                instruction_counter,
                u64::try_from(i).unwrap_or_default(),
                leg.clone(),
            );
        }

        if let SettlementType::SettleOnBlock(block_number) = settlement_type {
            Self::schedule_instruction(instruction_counter, block_number, legs.len() as u32);
        }

        <InstructionDetails<T>>::insert(instruction_counter, instruction);
        InstructionAffirmsPending::insert(
            instruction_counter,
            u64::try_from(counter_parties.len()).unwrap_or_default(),
        );
        venue.instructions.push(instruction_counter);
        VenueInfo::insert(venue_id, venue);
        InstructionCounter::put(instruction_counter + 1);
        Self::deposit_event(RawEvent::InstructionCreated(
            did,
            venue_id,
            instruction_counter,
            settlement_type,
            trade_date,
            value_date,
            legs,
        ));
        Ok(instruction_counter)
    }

    fn unsafe_withdraw_instruction_affirmation(
        did: IdentityId,
        instruction_id: u64,
        portfolios: BTreeSet<PortfolioId>,
        secondary_key: Option<&SecondaryKey<T::AccountId>>,
        max_legs_count: u32,
    ) -> Result<u32, DispatchError> {
        // checks custodianship of portfolios and affirmation status
        Self::ensure_portfolios_and_affirmation_status(
            instruction_id,
            &portfolios,
            did,
            secondary_key,
            &[AffirmationStatus::Affirmed],
        )?;
        // Unlock tokens that were previously locked during the affirmation
        let (total_leg_count, filtered_legs) =
            Self::filtered_legs(instruction_id, &portfolios, max_legs_count)?;
        for (leg_id, leg_details) in filtered_legs {
            match Self::instruction_leg_status(instruction_id, leg_id) {
                LegStatus::ExecutionToBeSkipped(signer, receipt_uid) => {
                    // Receipt was claimed for this instruction. Therefore, no token unlocking is required, we just unclaim the receipt.
                    <ReceiptsUsed<T>>::insert(&signer, receipt_uid, false);
                    Self::deposit_event(RawEvent::ReceiptUnclaimed(
                        did,
                        instruction_id,
                        leg_id,
                        receipt_uid,
                        signer,
                    ));
                }
                LegStatus::ExecutionPending => {
                    // Tokens are unlocked, need to be unlocked
                    Self::unlock_via_leg(&leg_details)?;
                }
                LegStatus::PendingTokenLock => {
                    return Err(Error::<T>::InstructionNotAffirmed.into())
                }
            };
            <InstructionLegStatus<T>>::insert(instruction_id, leg_id, LegStatus::PendingTokenLock);
        }

        // Updates storage
        for portfolio in &portfolios {
            UserAffirmations::insert(portfolio, instruction_id, AffirmationStatus::Pending);
            AffirmsReceived::remove(instruction_id, portfolio);
            Self::deposit_event(RawEvent::AffirmationWithdrawn(
                did,
                *portfolio,
                instruction_id,
            ));
        }

        InstructionAffirmsPending::mutate(instruction_id, |affirms_pending| {
            *affirms_pending += u64::try_from(portfolios.len()).unwrap_or_default()
        });

        Ok(total_leg_count)
    }

    fn ensure_instruction_validity(instruction_id: u64) -> DispatchResult {
        let details = Self::instruction_details(instruction_id);
        ensure!(
            details.status == InstructionStatus::Pending,
            Error::<T>::InstructionNotPending
        );
        if let (Some(trade_date), Some(value_date)) = (details.trade_date, details.value_date) {
            ensure!(
                value_date >= trade_date,
                Error::<T>::InstructionDatesInvalid
            );
        }
        if let SettlementType::SettleOnBlock(block_number) = details.settlement_type {
            ensure!(
                block_number > system::Module::<T>::block_number(),
                Error::<T>::InstructionSettleBlockPassed
            );
        }
        Ok(())
    }

    fn execute_instruction(instruction_id: u64) -> Result<u32, DispatchError> {
        let mut legs = <InstructionLegs<T>>::iter_prefix(instruction_id).collect::<Vec<_>>();
        // NB: Execution order doesn't matter in most cases but might matter in some edge cases around compliance
        // Example of an edge case: Consider a token with total supply 100 and maximum percentage ownership of 10%.
        // Alice owns 10 tokens, Bob owns 5 and Charlie owns 0.
        // Instruction has two legs: 1. Alice transfers 5 tokens to Charlie. 2. Bob transfers 5 tokens to Alice.
        // If the instruction is executed in order, it remains valid but if the second leg gets executed before first,
        // Alice will momentarily hold 15% of the asset and hence the settlement will fail compliance.
        legs.sort_by_key(|leg| leg.0);
        let instructions_processed: u32 = u32::try_from(legs.len()).unwrap_or_default();
        Self::unchecked_release_locks(instruction_id, &legs);
        let mut result = DispatchResult::Ok(());
        if Self::instruction_affirms_pending(instruction_id) > 0 {
            // Instruction rejected. Unlock any locked tokens and mark receipts as unused.
            // NB: Leg status is not updated because Instruction related details are deleted after settlement in any case.
            Self::unsafe_unclaim_receipts(instruction_id, &legs);
            Self::deposit_event(RawEvent::InstructionRejected(
                SettlementDID.as_id(),
                instruction_id,
            ));
            result = DispatchResult::Err(Error::<T>::InstructionFailed.into());
        } else {
            // Verify that the venue still has the required permissions for the tokens involved.
            let tickers: BTreeSet<Ticker> = legs.iter().map(|leg| leg.1.asset).collect();
            let venue_id = Self::instruction_details(instruction_id).venue_id;
            for ticker in &tickers {
                if Self::venue_filtering(ticker) && !Self::venue_allow_list(ticker, venue_id) {
                    Self::deposit_event(RawEvent::VenueUnauthorized(
                        SettlementDID.as_id(),
                        *ticker,
                        venue_id,
                    ));
                    result = DispatchResult::Err(Error::<T>::UnauthorizedVenue.into());
                }
            }

            if result.is_ok() {
                match with_transaction(|| {
                    for (leg_id, leg_details) in legs.iter().filter(|(leg_id, _)| {
                        let status = Self::instruction_leg_status(instruction_id, leg_id);
                        status == LegStatus::ExecutionPending
                    }) {
                        if <Asset<T>>::base_transfer(
                            leg_details.from,
                            leg_details.to,
                            &leg_details.asset,
                            leg_details.amount,
                        )
                        .is_err()
                        {
                            return Err(leg_id);
                        }
                    }
                    Ok(())
                }) {
                    Ok(_) => {
                        Self::deposit_event(RawEvent::InstructionExecuted(
                            SettlementDID.as_id(),
                            instruction_id,
                        ));
                    }
                    Err(leg_id) => {
                        Self::deposit_event(RawEvent::LegFailedExecution(
                            SettlementDID.as_id(),
                            instruction_id,
                            leg_id.clone(),
                        ));
                        Self::deposit_event(RawEvent::InstructionFailed(
                            SettlementDID.as_id(),
                            instruction_id,
                        ));
                        // We need to unclaim receipts for the failed transaction so that they can be reused
                        Self::unsafe_unclaim_receipts(instruction_id, &legs);
                        result = DispatchResult::Err(Error::<T>::InstructionFailed.into());
                    }
                }
            }
        };

        // Clean up instruction details to reduce chain bloat
        <InstructionLegs<T>>::remove_prefix(instruction_id);
        <InstructionDetails<T>>::remove(instruction_id);
        <InstructionLegStatus<T>>::remove_prefix(instruction_id);
        InstructionAffirmsPending::remove(instruction_id);
        AffirmsReceived::remove_prefix(instruction_id);
        Self::prune_user_affirmations(&legs, instruction_id);

        result.map_or_else(|e| Err(e), |_k| Ok(instructions_processed))
    }

    fn prune_user_affirmations(legs: &Vec<(u64, Leg<T::Balance>)>, instruction_id: u64) {
        // We remove duplicates in memory before triggering storage actions
        let mut counter_parties = Vec::with_capacity(legs.len() * 2);
        for (_, leg) in legs {
            counter_parties.push(leg.from);
            counter_parties.push(leg.to);
        }
        counter_parties.sort();
        counter_parties.dedup();
        for counter_party in counter_parties {
            UserAffirmations::remove(counter_party, instruction_id);
        }
    }

    pub fn unsafe_affirm_instruction(
        did: IdentityId,
        instruction_id: u64,
        portfolios: BTreeSet<PortfolioId>,
        max_legs_count: u32,
        secondary_key: Option<&SecondaryKey<T::AccountId>>,
    ) -> Result<u32, DispatchError> {
        // checks portfolio's custodian and if it is a counter party with a pending or rejected affirmation
        Self::ensure_portfolios_and_affirmation_status(
            instruction_id,
            &portfolios,
            did,
            secondary_key,
            &[AffirmationStatus::Pending, AffirmationStatus::Rejected],
        )?;

        let (total_leg_count, filtered_legs) =
            Self::filtered_legs(instruction_id, &portfolios, max_legs_count)?;
        with_transaction(|| {
            for (leg_id, leg_details) in filtered_legs {
                if let Err(_) = Self::lock_via_leg(&leg_details) {
                    // rustc fails to infer return type of `with_transaction` if you use ?/map_err here
                    return Err(DispatchError::from(Error::<T>::FailedToLockTokens));
                }
                <InstructionLegStatus<T>>::insert(
                    instruction_id,
                    leg_id,
                    LegStatus::ExecutionPending,
                );
            }
            Ok(())
        })?;

        let affirms_pending = Self::instruction_affirms_pending(instruction_id);

        // Updates storage
        for portfolio in &portfolios {
            UserAffirmations::insert(portfolio, instruction_id, AffirmationStatus::Affirmed);
            AffirmsReceived::insert(instruction_id, portfolio, AffirmationStatus::Affirmed);
            Self::deposit_event(RawEvent::InstructionAffirmed(
                did,
                *portfolio,
                instruction_id,
            ));
        }
        InstructionAffirmsPending::insert(
            instruction_id,
            affirms_pending.saturating_sub(u64::try_from(portfolios.len()).unwrap_or_default()),
        );

        Ok(total_leg_count)
    }

    fn unsafe_claim_receipt(
        did: IdentityId,
        instruction_id: u64,
        receipt_details: ReceiptDetails<T::AccountId, T::OffChainSignature>,
        secondary_key: Option<&SecondaryKey<T::AccountId>>,
    ) -> DispatchResult {
        Self::ensure_instruction_validity(instruction_id)?;

        ensure!(
            Self::instruction_leg_status(instruction_id, receipt_details.leg_id)
                == LegStatus::ExecutionPending,
            Error::<T>::LegNotPending
        );
        let venue_id = Self::instruction_details(instruction_id).venue_id;
        ensure!(
            Self::venue_signers(venue_id, &receipt_details.signer),
            Error::<T>::UnauthorizedSigner
        );
        ensure!(
            !Self::receipts_used(&receipt_details.signer, receipt_details.receipt_uid),
            Error::<T>::ReceiptAlreadyClaimed
        );

        let leg = Self::instruction_legs(instruction_id, receipt_details.leg_id);

        T::Portfolio::ensure_portfolio_custody_and_permission(leg.from, did, secondary_key)?;

        let msg = Receipt {
            receipt_uid: receipt_details.receipt_uid,
            from: leg.from,
            to: leg.to,
            asset: leg.asset,
            amount: leg.amount,
        };

        ensure!(
            receipt_details
                .signature
                .verify(&msg.encode()[..], &receipt_details.signer),
            Error::<T>::InvalidSignature
        );

        Self::unlock_via_leg(&leg)?;

        <ReceiptsUsed<T>>::insert(&receipt_details.signer, receipt_details.receipt_uid, true);

        <InstructionLegStatus<T>>::insert(
            instruction_id,
            receipt_details.leg_id,
            LegStatus::ExecutionToBeSkipped(
                receipt_details.signer.clone(),
                receipt_details.receipt_uid,
            ),
        );
        Self::deposit_event(RawEvent::ReceiptClaimed(
            did,
            instruction_id,
            receipt_details.leg_id,
            receipt_details.receipt_uid,
            receipt_details.signer,
            receipt_details.metadata,
        ));
        Ok(())
    }

    // Unclaims all receipts for an instruction
    // Should only be used if user is unclaiming, or instruction has failed
    fn unsafe_unclaim_receipts(instruction_id: u64, legs: &Vec<(u64, Leg<T::Balance>)>) {
        for (leg_id, _) in legs.iter() {
            match Self::instruction_leg_status(instruction_id, leg_id) {
                LegStatus::ExecutionToBeSkipped(signer, receipt_uid) => {
                    <ReceiptsUsed<T>>::insert(&signer, receipt_uid, false);
                    Self::deposit_event(RawEvent::ReceiptUnclaimed(
                        SettlementDID.as_id(),
                        instruction_id,
                        *leg_id,
                        receipt_uid,
                        signer,
                    ));
                }
                LegStatus::PendingTokenLock | LegStatus::ExecutionPending => {}
            }
        }
    }

    fn unchecked_release_locks(instruction_id: u64, legs: &Vec<(u64, Leg<T::Balance>)>) {
        for (leg_id, leg_details) in legs.iter() {
            match Self::instruction_leg_status(instruction_id, leg_id) {
                LegStatus::ExecutionPending => {
                    // This can never return an error since the settlement module
                    // must've locked these tokens when instruction was affirmed
                    let _ = Self::unlock_via_leg(&leg_details);
                }
                LegStatus::ExecutionToBeSkipped(_, _) | LegStatus::PendingTokenLock => {}
            }
        }
    }

    /// Schedule a given instruction to be executed on the next block only if the
    /// settlement type is `SettleOnAffirmation` and no. of affirms pending is 0.
    fn maybe_schedule_instruction(affirms_pending: u64, id: u64, legs_count: u32) {
        if affirms_pending == 0
            && Self::instruction_details(id).settlement_type == SettlementType::SettleOnAffirmation
        {
            // Schedule instruction to be executed in the next block.
            let execution_at = system::Module::<T>::block_number() + One::one();
            Self::schedule_instruction(id, execution_at, legs_count);
        }
    }

    /// Schedule execution of given instruction at given block number.
    ///
    /// NB - It is expected to execute the given instruction into the given block number but
    /// it is not a guaranteed behavior, Scheduler may have other high priority task scheduled
    /// for the given block so there are chances where the instruction execution block no. may drift.
    fn schedule_instruction(instruction_id: u64, execution_at: T::BlockNumber, legs_count: u32) {
        let call = Call::<T>::execute_scheduled_instruction(instruction_id, legs_count).into();
        if let Err(_) = T::Scheduler::schedule_named(
            (SETTLEMENT_INSTRUCTION_EXECUTION, instruction_id).encode(),
            DispatchTime::At(execution_at),
            None,
            SETTLEMENT_INSTRUCTION_EXECUTION_PRIORITY,
            RawOrigin::Root.into(),
            call,
        ) {
            Self::deposit_event(RawEvent::SchedulingFailed(
                Error::<T>::FailedToSchedule.into(),
            ));
        }
    }

    pub fn base_affirm_with_receipts(
        origin: <T as frame_system::Trait>::Origin,
        instruction_id: u64,
        receipt_details: Vec<ReceiptDetails<T::AccountId, T::OffChainSignature>>,
        portfolios: Vec<PortfolioId>,
        max_legs_count: u32,
    ) -> Result<u32, DispatchError> {
        let (did, secondary_key) =
            Self::ensure_origin_perm_and_instruction_validity(origin, instruction_id)?;
        let portfolios_set = portfolios.into_iter().collect::<BTreeSet<_>>();

        // Verify that the receipts provided are unique
        let receipt_ids = receipt_details
            .iter()
            .map(|receipt| (receipt.signer.clone(), receipt.receipt_uid))
            .collect::<BTreeSet<_>>();

        ensure!(
            receipt_ids.len() == receipt_details.len(),
            Error::<T>::ReceiptAlreadyClaimed
        );

        let instruction_details = Self::instruction_details(instruction_id);

        // verify portfolio custodianship and check if it is a counter party with a pending or rejected affirmation
        Self::ensure_portfolios_and_affirmation_status(
            instruction_id,
            &portfolios_set,
            did,
            secondary_key.as_ref(),
            &[AffirmationStatus::Pending, AffirmationStatus::Rejected],
        )?;

        // Verify that the receipts are valid
        for receipt in &receipt_details {
            ensure!(
                Self::venue_signers(&instruction_details.venue_id, &receipt.signer),
                Error::<T>::UnauthorizedSigner
            );
            ensure!(
                !Self::receipts_used(&receipt.signer, &receipt.receipt_uid),
                Error::<T>::ReceiptAlreadyClaimed
            );

            let leg = Self::instruction_legs(&instruction_id, &receipt.leg_id);
            ensure!(
                portfolios_set.contains(&leg.from),
                Error::<T>::PortfolioMismatch
            );

            let msg = Receipt {
                receipt_uid: receipt.receipt_uid,
                from: leg.from,
                to: leg.to,
                asset: leg.asset,
                amount: leg.amount,
            };
            ensure!(
                receipt.signature.verify(&msg.encode()[..], &receipt.signer),
                Error::<T>::InvalidSignature
            );
        }

        let (total_leg_count, filtered_legs) =
            Self::filtered_legs(instruction_id, &portfolios_set, max_legs_count)?;
        // Lock tokens that do not have a receipt attached to their leg.
        with_transaction(|| {
            for (leg_id, leg_details) in filtered_legs {
                // Receipt for the leg was provided
                if let Some(receipt) = receipt_details
                    .iter()
                    .find(|receipt| receipt.leg_id == leg_id)
                {
                    <InstructionLegStatus<T>>::insert(
                        instruction_id,
                        leg_id,
                        LegStatus::ExecutionToBeSkipped(
                            receipt.signer.clone(),
                            receipt.receipt_uid,
                        ),
                    );
                } else if let Err(_) = Self::lock_via_leg(&leg_details) {
                    // rustc fails to infer return type of `with_transaction` if you use ?/map_err here
                    return Err(DispatchError::from(Error::<T>::FailedToLockTokens));
                } else {
                    <InstructionLegStatus<T>>::insert(
                        instruction_id,
                        leg_id,
                        LegStatus::ExecutionPending,
                    );
                }
            }
            Ok(())
        })?;

        // Update storage
        let affirms_pending = Self::instruction_affirms_pending(instruction_id)
            .saturating_sub(u64::try_from(portfolios_set.len()).unwrap_or_default());

        // Mark receipts used in affirmation as claimed
        for receipt in &receipt_details {
            <ReceiptsUsed<T>>::insert(&receipt.signer, receipt.receipt_uid, true);
            Self::deposit_event(RawEvent::ReceiptClaimed(
                did,
                instruction_id,
                receipt.leg_id,
                receipt.receipt_uid,
                receipt.signer.clone(),
                receipt.metadata.clone(),
            ));
        }

        for portfolio in portfolios_set {
            UserAffirmations::insert(portfolio, instruction_id, AffirmationStatus::Affirmed);
            AffirmsReceived::insert(instruction_id, portfolio, AffirmationStatus::Affirmed);
            Self::deposit_event(RawEvent::InstructionAffirmed(
                did,
                portfolio,
                instruction_id,
            ));
        }

        InstructionAffirmsPending::insert(instruction_id, affirms_pending);
        Ok(total_leg_count)
    }

    pub fn base_affirm_instruction(
        origin: <T as frame_system::Trait>::Origin,
        instruction_id: u64,
        portfolios: impl Iterator<Item = PortfolioId>,
        max_legs_count: u32,
    ) -> Result<u32, DispatchError> {
        let (did, secondary_key) =
            Self::ensure_origin_perm_and_instruction_validity(origin, instruction_id)?;
        let portfolios_set = portfolios.collect::<BTreeSet<_>>();

        // Provide affirmation to the instruction
        Self::unsafe_affirm_instruction(
            did,
            instruction_id,
            portfolios_set,
            max_legs_count,
            secondary_key.as_ref(),
        )
    }

    // It affirms the instruction and may schedule the instruction
    // depends on the settlement type.
    pub fn affirm_with_receipts_and_maybe_schedule_instruction(
        origin: <T as frame_system::Trait>::Origin,
        instruction_id: u64,
        receipt_details: Vec<ReceiptDetails<T::AccountId, T::OffChainSignature>>,
        portfolios: Vec<PortfolioId>,
        max_legs_count: u32,
    ) -> DispatchResult {
        let legs_count = Self::base_affirm_with_receipts(
            origin,
            instruction_id,
            receipt_details,
            portfolios,
            max_legs_count,
        )?;
        // Schedule instruction to be execute in the next block (expected) if conditions are met.
        Self::maybe_schedule_instruction(
            Self::instruction_affirms_pending(instruction_id),
            instruction_id,
            legs_count,
        );
        Ok(())
    }

    /// Schedule settlement instruction execution in the next block, unless already scheduled.
    /// Used for general purpose settlement.
    pub fn affirm_and_maybe_schedule_instruction(
        origin: <T as frame_system::Trait>::Origin,
        instruction_id: u64,
        portfolios: impl Iterator<Item = PortfolioId>,
        max_legs_count: u32,
    ) -> DispatchResult {
        let legs_count =
            Self::base_affirm_instruction(origin, instruction_id, portfolios, max_legs_count)?;
        // Schedule the instruction if conditions are met
        Self::maybe_schedule_instruction(
            Self::instruction_affirms_pending(instruction_id),
            instruction_id,
            legs_count,
        );
        Ok(())
    }

    /// Affirm with receipts, executing the instruction when all affirmations have been received.
    ///
    /// NB - Use this function only in the STO pallet to support DVP settlements.
    pub fn affirm_and_execute_instruction(
        origin: <T as frame_system::Trait>::Origin,
        instruction_id: u64,
        portfolios: Vec<PortfolioId>,
        max_legs_count: u32,
    ) -> DispatchResult {
        with_transaction(|| {
            Self::base_affirm_instruction(
                origin,
                instruction_id,
                portfolios.into_iter(),
                max_legs_count,
            )?;
            Self::execute_settle_on_affirmation_instruction(
                instruction_id,
                Self::instruction_affirms_pending(instruction_id),
                Self::instruction_details(instruction_id).settlement_type,
            )
        })
    }

    /// Affirm with receipts, executing the instruction when all affirmations have been received.
    ///
    /// NB - Use this function only in the STO pallet to support DVP settlements.
    pub fn affirm_with_receipts_and_execute_instruction(
        origin: <T as frame_system::Trait>::Origin,
        instruction_id: u64,
        receipt_details: Vec<ReceiptDetails<T::AccountId, T::OffChainSignature>>,
        portfolios: Vec<PortfolioId>,
        max_legs_count: u32,
    ) -> DispatchResult {
        with_transaction(|| {
            Self::base_affirm_with_receipts(
                origin,
                instruction_id,
                receipt_details,
                portfolios,
                max_legs_count,
            )?;
            Self::execute_settle_on_affirmation_instruction(
                instruction_id,
                Self::instruction_affirms_pending(instruction_id),
                Self::instruction_details(instruction_id).settlement_type,
            )
        })
    }

    fn execute_settle_on_affirmation_instruction(
        instruction_id: u64,
        affirms_pending: u64,
        settlement_type: SettlementType<T::BlockNumber>,
    ) -> DispatchResult {
        // We assume `settlement_type == SettleOnAffirmation`,
        // to be defensive, however, this is checked before instruction execution.
        if settlement_type == SettlementType::SettleOnAffirmation && affirms_pending == 0 {
            Self::execute_instruction(instruction_id)?;
        }
        Ok(())
    }

    fn ensure_portfolios_and_affirmation_status(
        instruction_id: u64,
        portfolios: &BTreeSet<PortfolioId>,
        custodian: IdentityId,
        secondary_key: Option<&SecondaryKey<T::AccountId>>,
        expected_statuses: &[AffirmationStatus],
    ) -> DispatchResult {
        for portfolio in portfolios {
            T::Portfolio::ensure_portfolio_custody_and_permission(
                *portfolio,
                custodian,
                secondary_key,
            )?;
            let user_affirmation = Self::user_affirmations(portfolio, instruction_id);
            ensure!(
                expected_statuses.contains(&user_affirmation),
                Error::<T>::UnexpectedAffirmationStatus
            );
        }
        Ok(())
    }

    /// Returns total number of legs of an `instruction_id` and vector of legs where sender is in the `portfolios` set.
    /// Also, ensures that the number of filtered legs is under the limit.
    fn filtered_legs(
        instruction_id: u64,
        portfolios: &BTreeSet<PortfolioId>,
        max_filtered_legs: u32,
    ) -> Result<(u32, Vec<(u64, Leg<T::Balance>)>), DispatchError> {
        let mut legs_count = 0;
        let filtered_legs = <InstructionLegs<T>>::iter_prefix(instruction_id)
            .into_iter()
            .inspect(|_| legs_count += 1)
            .filter(|(_, leg_details)| portfolios.contains(&leg_details.from))
            .collect::<Vec<_>>();
        // Ensure leg count is under the limit
        ensure!(
            filtered_legs.len() as u32 <= max_filtered_legs,
            Error::<T>::LegCountTooSmall
        );
        Ok((legs_count, filtered_legs))
    }
}
