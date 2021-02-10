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

//! # Corporate Ballots module.
//!
//! The corporate ballots module provides functionality for conducting corporate ballots,
//! e.g., for the annual general meeting.
//! Ballots consist of a set of motions, each with a set of choices like "Yay" or "Nay".
//!
//! The process works by first initiating the corporate action (CA) through `initiate_corporate_action`,
//! and then attaching a ballot to it via `attach_ballot`.
//! When attaching a ballot, the motions are provided, along with when the duration of the ballot.
//!
//! Once the start time is due, token holders in the CA's ticker/asset may cast their ballot.
//! To do so, they call the `vote` dispatchable,
//! dividing their available votes to each choice within a motion.
//!
//! The available votes are computed based on the record date provided when the CA was created.
//! The record date is then translated into a checkpoint,
//! and the holder's balance at that point is used as the available power.
//!
//! Eventually, the voting duration will be over.
//! The interpretation of the vote results can then be interpreted off-chain,
//! depending on the exact by-laws of the corporation.
//! For example, Ranked-Choice Voting (RCV), may be used, when fallbacks are provided in votes.
//!
//! ## Overview
//!
//! The Voting module provides functions for:
//!
//! - Creating ballots that can include multiple motions with multiple choices for each of those.
//! - Adjusting details of a ballot that hasn't yet started.
//! - Voting on motions.
//! - Removing/Cancelling ballots.
//!
//! ### Terminology
//!
//! - **Ballot:** A set of motions made, each with a set of choices on which a token holder can vote.
//!
//! - **Motion:** A motion can be e.g., "Elect Alice as CEO".
//!     That is, a motion is a suggested action or stance that the corporation should take.
//!     Each motion can then have a number of choices, e.g., "Yay", or "Nay".
//!     Token holders can then divide all of their power across the choices of one motion,
//!     and reuse the same amount of voting power on other motions.
//!     The motion is associated with some descriptive text, and a link for more information.
//!     Commonly, a motion will only have two choices, "Yay" or "Nay".
//!     Any voting power that is not used is considered as abstain.
//!
//! - **RCV:** Ranked-Choice Voting allows voters to select a fallback choice should their first
//!     preference fail to reach a certain threshold or e.g., be eliminated in the top-2 run-off.
//!     The chain supports this by admitting fallback choices, if the ballot is configured to support this.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `attach_ballot(origin, ca_id, range, meta)` attaches a ballot to CA with `ca_id`
//!   within the voting duration specified by `range`, and motions drawn from `meta`.
//! - `vote(origin, ca_id, votes)` casts `votes` in the ballot for CA with `ca_id`.
//! - `change_end(origin, ca_id, end)` changes the end date of the ballot for CA with `ca_id`.
//! - `change_meta(origin, ca_id, meta)` changes the motions of the ballot for CA with `ca_id`.
//! - `change_rcv(origin, ca_id, rcv)` changes the support for RCV to `rcv` in the ballot for CA with `ca_id`.
//! - `remove_ballot(origin, ca_id)` removes the ballot for CA with `ca_id`.

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

use crate as ca;
use ca::{CAId, CAKind, CorporateAction, Trait};
use codec::{Decode, Encode};
use core::convert::TryInto;
use core::mem;
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    traits::Get,
    weights::Weight,
};
use pallet_asset::checkpoint;
use pallet_identity as identity;
use polymesh_common_utilities::protocol_fee::{ChargeProtocolFee, ProtocolOp};
use polymesh_common_utilities::CommonTrait;
use polymesh_primitives::{EventDid, IdentityId, Moment};
use polymesh_primitives_derive::VecU8StrongTyped;
use sp_runtime::traits::{CheckedAdd, Zero};
#[cfg(feature = "std")]
use sp_runtime::{Deserialize, Serialize};
use sp_std::prelude::*;

type Identity<T> = identity::Module<T>;
type Checkpoint<T> = checkpoint::Module<T>;
type CA<T> = ca::Module<T>;

/// A wrapper for a motion title.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, PartialEq, Eq, Hash, Debug, Decode, Encode, VecU8StrongTyped)]
pub struct MotionTitle(pub Vec<u8>);

/// A wrapper for a motion info link.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, PartialEq, Eq, Hash, Debug, Decode, Encode, VecU8StrongTyped)]
pub struct MotionInfoLink(pub Vec<u8>);

/// A wrapper for a choice's title.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, PartialEq, Eq, Hash, Debug, Decode, Encode, VecU8StrongTyped)]
pub struct ChoiceTitle(pub Vec<u8>);

/// Details about motions
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
pub struct Motion {
    /// Title of the motion
    pub title: MotionTitle,

    /// Link from where more information about the motion can be obtained.
    pub info_link: MotionInfoLink,

    /// Choices for the motion excluding abstain.
    /// Voting power not used is considered abstained.
    pub choices: Vec<ChoiceTitle>,
}

/// A wrapper for a ballot's title.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default, Decode, Encode, VecU8StrongTyped)]
pub struct BallotTitle(pub Vec<u8>);

/// Metadata about a ballot.
///
/// Beyond the number of motions and the number of choices within a motion,
/// the actual metadata strings have no on-chain effect.
/// When the metadata has been committed to chain,
/// the needed numbers aforementioned are cached away,
/// and the metadata is not read on-chain again.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, Default)]
pub struct BallotMeta {
    /// The ballot's title.
    pub title: BallotTitle,

    /// All motions with their associated titles, choices, etc.
    pub motions: Vec<Motion>,
}

impl BallotMeta {
    fn saturating_num_choices(&self) -> u32 {
        self.motions
            .iter()
            .fold(0, |a, m| a.saturating_add(m.choices.len() as u32))
    }
}

/// Timestamp range details about vote start / end.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug, Encode, Decode)]
pub struct BallotTimeRange {
    /// Timestamp at which voting starts.
    pub start: Moment,

    /// Timestamp at which voting ends.
    pub end: Moment,
}

/// A vote cast on some choice in some motion in a ballot.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug, Encode, Decode)]
pub struct BallotVote<Balance> {
    /// The weight / voting power assigned to this vote.
    pub power: Balance,
    /// The fallback choice, if any, to assign `power` to,
    /// should the vote total fail to reach some threshold.
    ///
    /// This is only used when Ranked-Choice Voting (RCV) is active.
    ///
    /// # Representation
    ///
    /// The fallback is encoded as an index into one of the other choices within the same motion.
    /// Representationally, this admits an arbitrary directed graph, but we do exclude self-cycles.
    ///
    /// # An example
    ///
    /// To understand the semantics of `fallback`,
    /// let's consider a ballot, with a simplified representation:
    ///
    /// ```text
    /// ballot: [
    ///     motion A: { title: "Everyone must love chocolate.", choices: ["Yay", "Nay"] },
    ///     motion B: { title: "Elect 🦄 for president", choices: ["OK", "Make them Veep", "Nope"] },
    /// ]
    /// ```
    ///
    /// Votes are provided as a flat list, assigned to each choice.
    /// For example, imagine that only Alice votes, using a total of 100 power.
    /// In this case, she voted like:
    /// ```text
    /// votes: [
    ///     BallotVote { power: 100, fallback: None },
    ///     BallotVote { power: 0,   fallback: None },
    ///
    ///     BallotVote { power: 41,  fallback: None },
    ///     BallotVote { power: 49,  fallback: None },
    ///     BallotVote { power: 10,  fallback: Some(0) },
    /// ]
    /// ```
    ///
    /// Here, the first two `BallotVote`s belong to the two choices in motion A.
    /// The three remaining belong to motion B.
    ///
    /// Now suppose that we have a top-2 run-off voting process.
    /// Zooming in on motion B, the third choice would be eliminated,
    /// but because of `fallback: Some(0)`, now choice "OK" receives an additional 10 votes,
    /// putting the choice at a total of 51 votes. As 51 > 49, this is the choice that wins.
    ///
    /// Note that `Some(0)` does *not* point into motion A's first choice.
    pub fallback: Option<u16>,
}

/// Weight abstraction for the corporate actions module.
pub trait WeightInfo {
    fn attach_ballot(num_choices: u32) -> Weight;
    fn vote(votes: u32, target_ids: u32) -> Weight;
    fn change_end() -> Weight;
    fn change_meta(num_choices: u32) -> Weight;
    fn change_rcv() -> Weight;
    fn remove_ballot() -> Weight;
}

decl_storage! {
    trait Store for Module<T: Trait> as CorporateBallot {
        /// Metadata of a corporate ballot.
        ///
        /// (CAId) => BallotMeta
        pub Metas get(fn metas): map hasher(blake2_128_concat) CAId => Option<BallotMeta>;

        /// Time details of a corporate ballot associated with a CA.
        /// The timestamps denote when voting starts and stops.
        ///
        /// (CAId) => BallotTimeRange
        pub TimeRanges get(fn time_ranges): map hasher(blake2_128_concat) CAId => Option<BallotTimeRange>;

        /// Stores how many choices there are in each motion.
        ///
        /// At all times, the invariant holds that `motion_choices[idx]` is equal to
        /// `metas.unwrap().motions[idx].choices.len()`. That is, this is just a cache,
        /// used to avoid fetching all the motions with their associated texts.
        ///
        /// `u16` choices should be more than enough to fit real use cases.
        ///
        /// (CAId) => Number of choices in each motion.
        pub MotionNumChoices get(fn motion_choices): map hasher(blake2_128_concat) CAId => Vec<u16>;

        /// Is ranked choice voting (RCV) enabled for this ballot?
        /// For an understanding of how RCV is handled, see note on `BallotVote`'s `fallback` field.
        ///
        /// (CAId) => bool
        pub RCV get(fn rcv): map hasher(blake2_128_concat) CAId => bool;

        /// Stores the total vote tally on each choice.
        ///
        /// RCV is not accounted for,
        /// as there are too many wants to interpret the graph,
        /// and because it would not be efficient.
        ///
        /// (CAId) => [current vote weights]
        pub Results get(fn results): map hasher(blake2_128_concat) CAId => Vec<T::Balance>;

        /// Stores each DID's votes in a given ballot.
        /// See the documentation of `BallotVote` for notes on semantics.
        ///
        /// (CAId) => (DID) => [vote weight]
        ///
        /// User must enter 0 vote weight if they don't want to vote for a choice.
        pub Votes get(fn votes):
            double_map hasher(blake2_128_concat) CAId, hasher(identity) IdentityId =>
                Vec<BallotVote<T::Balance>>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Attach a corporate ballot to the CA identified by `ca_id`.
        ///
        /// The ballot will admit votes within `range`.
        /// The ballot's metadata is provided by `meta`,
        /// which includes the ballot title, the motions, their choices, etc.
        /// See the `BallotMeta` for more.
        ///
        /// ## Arguments
        /// - `origin` which must be a signer for the CAA of `ca_id`.
        /// - `ca_id` identifies the CA to attach the ballot to.
        /// - `range` specifies when voting starts and ends.
        /// - `meta` specifies the ballot's metadata as aforementioned.
        /// - `rcv` specifies whether RCV is enabled for this ballot.
        ///
        /// # Errors
        /// - `UnauthorizedAsAgent` if `origin` is not `ticker`'s sole CAA (owner is not necessarily the CAA).
        /// - `NoSuchCA` if `ca_id` does not identify an existing CA.
        /// - `CANotNotice` if the CA is not of the `IssuerNotice` kind.
        /// - `StartAfterEnd` if `range.start > range.end`.
        /// - `NowAfterEnd` if `now > range.end` where `now` is the current timestamp.
        /// - `NoRecordDate` if CA has no record date.
        /// - `RecordDateAfterStart` if `date > range.start` where `date` is the CA's record date.
        /// - `AlreadyExists` if there's a ballot already.
        /// - `NumberOfChoicesOverflow` if the total choice in `meta` overflows `usize`.
        /// - `InsufficientBalance` if the protocol fee couldn't be charged.
        #[weight = <T as Trait>::BallotWeightInfo::attach_ballot(meta.saturating_num_choices())]
        pub fn attach_ballot(origin, ca_id: CAId, range: BallotTimeRange, meta: BallotMeta, rcv: bool) {
            // Ensure origin is CAA, that `ca_id` exists, that its a notice, and the date invariant.
            let caa = <CA<T>>::ensure_ca_agent(origin, ca_id.ticker)?;
            let ca = <CA<T>>::ensure_ca_exists(ca_id)?;
            ensure!(matches!(ca.kind, CAKind::IssuerNotice), Error::<T>::CANotNotice);
            Self::ensure_range_invariant(&ca, range)?;

            // Ensure CA doesn't have a ballot yet.
            ensure!(!TimeRanges::contains_key(ca_id), Error::<T>::AlreadyExists);

            // Compute number-of-choices-in-motion cache.
            let choices = Self::derive_motion_num_choices(&meta.motions)?;

            // Charge protocol fee.
            T::ProtocolFee::charge_fee(ProtocolOp::BallotAttachBallot)?;

            // Commit to storage.
            MotionNumChoices::insert(ca_id, choices);
            TimeRanges::insert(ca_id, range);
            Metas::insert(ca_id, meta.clone());
            RCV::insert(ca_id, rcv);

            // Emit event.
            Self::deposit_event(Event::<T>::Created(caa, ca_id, range, meta, rcv));
        }

        /// Cast `votes` in the ballot attached to the CA identified by `ca_id`.
        ///
        /// ## Arguments
        /// - `origin` which must be a permissioned signer targeted by the CA.
        /// - `ca_id` identifies the attached ballot's CA.
        /// - `votes` specifies the balances to assign to each choice in the ballot.
        ///    The full voting power of `origin`'s DID may be used for each motion in the ballot.
        ///
        /// # Errors
        /// - `NoSuchBallot` if `ca_id` does not identify a ballot.
        /// - `VotingNotStarted` if the voting period hasn't commenced yet.
        /// - `VotingAlreadyEnded` if the voting period has ended.
        /// - `WrongVoteCount` if the number of choices in the ballot does not match `votes.len()`.
        /// - `NoSuchCA` if `ca_id` does not identify an existing CA.
        /// - `NotTargetedByCA` if the CA does not target `origin`'s DID.
        /// - `InsufficientVotes` if the voting power used for any motion in `votes`
        ///    exceeds `origin`'s DID's voting power.
        #[weight = <T as Trait>::BallotWeightInfo::vote(votes.len() as u32, T::MaxTargetIds::get())]
        pub fn vote(origin, ca_id: CAId, votes: Vec<BallotVote<T::Balance>>) {
            let did = <Identity<T>>::ensure_perms(origin)?;

            // Ensure ballot has started but not ended, i.e. `start <= now <= end`.
            let range = Self::ensure_ballot_exists(ca_id)?;
            let now = <Checkpoint<T>>::now_unix();
            ensure!(range.start <= now, Error::<T>::VotingNotStarted);
            ensure!(now <= range.end, Error::<T>::VotingAlreadyEnded);

            // Ensure that `did` is targeted by this ballot.
            let ca = <CA<T>>::ensure_ca_exists(ca_id)?;
            <CA<T>>::ensure_ca_targets(&ca, &did)?;

            // Ensure we have balances provided for each choice.
            let choices_count = MotionNumChoices::get(ca_id);
            let total_choices = choices_count.iter().copied().map(|c| c as usize).sum::<usize>();
            ensure!(votes.len() == total_choices, Error::<T>::WrongVoteCount);

            // Divide `votes` into motions.
            let motions = choices_count
                .iter()
                .map(|c| *c as usize)
                .scan(0, |start, count| {
                    let end = *start + count;
                    Some(&votes[mem::replace(start, end)..end])
                });

            if RCV::get(ca_id) {
                // RCV is enabled.
                // Ensure that all fallback choices point to some choice in the same motion.
                // For in-depth discussion on `fallback`, consult `BallotVote`'s definition.
                motions
                    .clone()
                    .try_for_each(|votes| -> DispatchResult {
                        let count = votes.len();
                        votes
                            .iter()
                            .enumerate()
                            // Only check when a fallback is actually provided.
                            .filter_map(|(idx, vote)| Some((idx, vote.fallback? as usize)))
                            .try_for_each(|(idx, fallback)| {
                                // Exclude self-cycles.
                                ensure!(idx != fallback, Error::<T>::RCVSelfCycle);
                                // Ensure the index does not point outside, i.e. beyond, the motion.
                                ensure!(fallback < count, Error::<T>::NoSuchRCVFallback);
                                Ok(())
                            })
                    })?;
            } else {
                // It's not. Make sure its also not used.
                votes
                    .iter()
                    .all(|vote| vote.fallback.is_none())
                    .then_some(())
                    .ok_or(Error::<T>::RCVNotAllowed)?;
            }

            // Extract `did`'s balance at the record date.
            // Record date has passed by definition.
            let cp_id = <CA<T>>::record_date_cp(&ca, ca_id);
            let available_power = <CA<T>>::balance_at_cp(did, ca_id, cp_id);

            // Ensure the total balance used in each motion doesn't exceed caller's voting power.
            motions
                .map(|vs| vs.iter().try_fold(T::Balance::zero(), |acc, vote| acc.checked_add(&vote.power)))
                .all(|power| power.filter(|&p| p <= available_power).is_some())
                .then_some(())
                .ok_or(Error::<T>::InsufficientVotes)?;

            // Update vote and total results.
            <Votes<T>>::mutate(ca_id, did, |vslot| {
                <Results<T>>::mutate_exists(ca_id, |rslot| match rslot {
                    Some(rslot) => {
                        for (result, old) in rslot.iter_mut().zip(vslot.iter()) {
                            *result -= old.power;
                        }
                        for (result, new) in rslot.iter_mut().zip(votes.iter()) {
                            *result += new.power;
                        }
                    }
                    None => *rslot = Some(votes.iter().map(|v| v.power).collect()),
                });
                *vslot = votes.clone();
            });

            // Emit event.
            Self::deposit_event(Event::<T>::VoteCast(did, ca_id, votes));
        }

        /// Amend the end date of the ballot of the CA identified by `ca_id`.
        ///
        /// ## Arguments
        /// - `origin` which must be a signer for the CAA of `ca_id`.
        /// - `ca_id` identifies the attached ballot's CA.
        /// - `end` specifies the new end date of the ballot.
        ///
        /// # Errors
        /// - `UnauthorizedAsAgent` if `origin` is not `ticker`'s sole CAA (owner is not necessarily the CAA).
        /// - `NoSuchBallot` if `ca_id` does not identify a ballot.
        /// - `VotingAlreadyStarted` if `start >= now`, where `now` is the current time.
        /// - `StartAfterEnd` if `start > end`.
        #[weight = <T as Trait>::BallotWeightInfo::change_end()]
        pub fn change_end(origin, ca_id: CAId, end: Moment) {
            // Ensure origin is CAA, ballot exists, and start is in the future.
            let caa = <CA<T>>::ensure_ca_agent(origin, ca_id.ticker)?;
            let mut range = Self::ensure_ballot_exists(ca_id)?;
            Self::ensure_ballot_not_started(range)?;

            // Ensure we preserve `start <= end`.
            range.end = end;
            Self::ensure_range_consistent(range)?;

            // Commit new range to storage + emit event.
            TimeRanges::insert(ca_id, range);
            Self::deposit_event(Event::<T>::RangeChanged(caa, ca_id, range));
        }

        /// Amend the metadata (title, motions, etc.) of the ballot of the CA identified by `ca_id`.
        ///
        /// ## Arguments
        /// - `origin` which must be a signer for the CAA of `ca_id`.
        /// - `ca_id` identifies the attached ballot's CA.
        /// - `meta` specifies the new metadata.
        ///
        /// # Errors
        /// - `UnauthorizedAsAgent` if `origin` is not `ticker`'s sole CAA (owner is not necessarily the CAA).
        /// - `NoSuchBallot` if `ca_id` does not identify a ballot.
        /// - `VotingAlreadyStarted` if `start >= now`, where `now` is the current time.
        /// - `NumberOfChoicesOverflow` if the total choice in `meta` overflows `usize`.
        #[weight = <T as Trait>::BallotWeightInfo::change_meta(meta.saturating_num_choices())]
        pub fn change_meta(origin, ca_id: CAId, meta: BallotMeta) {
            // Ensure origin is CAA, a ballot exists, start is in the future.
            let caa = <CA<T>>::ensure_ca_agent(origin, ca_id.ticker)?;
            Self::ensure_ballot_not_started(Self::ensure_ballot_exists(ca_id)?)?;

            // Compute number-of-choices-in-motion cache.
            let choices = Self::derive_motion_num_choices(&meta.motions)?;

            // Commit metadata to storage + emit event.
            MotionNumChoices::insert(ca_id, choices);
            Metas::insert(ca_id, meta.clone());
            Self::deposit_event(Event::<T>::MetaChanged(caa, ca_id, meta));
        }

        /// Amend RCV support for the ballot of the CA identified by `ca_id`.
        ///
        /// ## Arguments
        /// - `origin` which must be a signer for the CAA of `ca_id`.
        /// - `ca_id` identifies the attached ballot's CA.
        /// - `rcv` specifies if RCV is to be supported or not.
        ///
        /// # Errors
        /// - `UnauthorizedAsAgent` if `origin` is not `ticker`'s sole CAA (owner is not necessarily the CAA).
        /// - `NoSuchBallot` if `ca_id` does not identify a ballot.
        /// - `VotingAlreadyStarted` if `start >= now`, where `now` is the current time.
        #[weight = <T as Trait>::BallotWeightInfo::change_rcv()]
        pub fn change_rcv(origin, ca_id: CAId, rcv: bool) {
            // Ensure origin is CAA, a ballot exists, start is in the future.
            let caa = <CA<T>>::ensure_ca_agent(origin, ca_id.ticker)?;
            Self::ensure_ballot_not_started(Self::ensure_ballot_exists(ca_id)?)?;

            // Commit to storage + emit event.
            RCV::insert(ca_id, rcv);
            Self::deposit_event(Event::<T>::RCVChanged(caa, ca_id, rcv));
        }

        /// Remove the ballot of the CA identified by `ca_id`.
        ///
        /// ## Arguments
        /// - `origin` which must be a signer for the CAA of `ca_id`.
        /// - `ca_id` identifies the attached ballot's CA.
        ///
        /// # Errors
        /// - `UnauthorizedAsAgent` if `origin` is not `ticker`'s sole CAA (owner is not necessarily the CAA).
        /// - `NoSuchBallot` if `ca_id` does not identify a ballot.
        /// - `VotingAlreadyStarted` if `start >= now`, where `now` is the current time.
        #[weight = <T as Trait>::BallotWeightInfo::remove_ballot()]
        pub fn remove_ballot(origin, ca_id: CAId) {
            let caa = <CA<T>>::ensure_ca_agent(origin, ca_id.ticker)?.for_event();
            let range = Self::ensure_ballot_exists(ca_id)?;
            Self::remove_ballot_base(caa, ca_id, range)?;
        }
    }
}

decl_event! {
    pub enum Event<T>
    where
        Balance = <T as CommonTrait>::Balance,
    {
        /// A corporate ballot was created.
        ///
        /// (Ticker's CAA, CA's ID, Voting start/end, Ballot metadata, RCV enabled?)
        Created(IdentityId, CAId, BallotTimeRange, BallotMeta, bool),

        /// A vote was cast in a corporate ballot.
        ///
        /// (voter DID, CAId, Votes)
        VoteCast(IdentityId, CAId, Vec<BallotVote<Balance>>),

        /// A corporate ballot changed its start/end date range.
        ///
        /// (Ticker's CAA, CA's ID, Voting start/end)
        RangeChanged(IdentityId, CAId, BallotTimeRange),

        /// A corporate ballot changed its metadata.
        ///
        /// (Ticker's CAA, CA's ID, New metadata)
        MetaChanged(IdentityId, CAId, BallotMeta),

        /// A corporate ballot changed its RCV support.
        ///
        /// (Ticker's CAA, CA's ID, New support)
        RCVChanged(IdentityId, CAId, bool),

        /// A corporate ballot was removed.
        ///
        /// (Ticker's CAA, CA's ID)
        Removed(EventDid, CAId),
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// A corporate ballot was made for a non `IssuerNotice` CA.
        CANotNotice,
        /// A corporate ballot already exists for this CA.
        AlreadyExists,
        /// A corporate ballot doesn't exist for this CA.
        NoSuchBallot,
        /// A corporate ballot's start time was strictly after the ballot's end.
        StartAfterEnd,
        /// A corporate ballot's end time was strictly before the current time.
        NowAfterEnd,
        /// If some motion in a corporate ballot has more choices than would fit in `u16`.
        NumberOfChoicesOverflow,
        /// Voting started already. Amending a ballot is no longer possible.
        VotingAlreadyStarted,
        /// Voting hasn't started yet.
        VotingNotStarted,
        /// Voting ended already.
        VotingAlreadyEnded,
        /// Provided list of balances does not match the total number of choices.
        WrongVoteCount,
        /// Voting power used by a DID on a motion exceeds that which is available to them.
        InsufficientVotes,
        /// The RCV fallback of some choice does not exist.
        NoSuchRCVFallback,
        /// The RCV fallback points to the origin choice.
        RCVSelfCycle,
        /// RCV is not allowed for this ballot.
        RCVNotAllowed
    }
}

impl<T: Trait> Module<T> {
    /// Ensure the ballot hasn't started and remove it.
    crate fn remove_ballot_base(
        caa: EventDid,
        ca_id: CAId,
        range: BallotTimeRange,
    ) -> DispatchResult {
        Self::ensure_ballot_not_started(range)?;

        // Remove all ballot data.
        TimeRanges::remove(ca_id);
        Metas::remove(ca_id);
        MotionNumChoices::remove(ca_id);
        RCV::remove(ca_id);

        // Emit event.
        Self::deposit_event(Event::<T>::Removed(caa, ca_id));
        Ok(())
    }

    // Compute number-of-choices-in-motion cache for `motions`.
    fn derive_motion_num_choices(motions: &[Motion]) -> Result<Vec<u16>, DispatchError> {
        let mut total: usize = 0;
        motions
            .iter()
            .map(|motion| {
                let len = motion.choices.len();
                // Overflowing usize here will never happen in practice,
                // but can happen in theory.
                // We do this now to avoid the potential overflow in `vote`.
                total = total.checked_add(len)?;
                len.try_into().ok()
            })
            .collect::<Option<_>>()
            .ok_or_else(|| Error::<T>::NumberOfChoicesOverflow.into())
    }

    /// Ensure that `now < range.start`.
    crate fn ensure_ballot_not_started(range: BallotTimeRange) -> DispatchResult {
        ensure!(
            <Checkpoint<T>>::now_unix() < range.start,
            Error::<T>::VotingAlreadyStarted
        );
        Ok(())
    }

    /// Ensure that `ca_id` has an active ballot and return its date-time range.
    fn ensure_ballot_exists(ca_id: CAId) -> Result<BallotTimeRange, DispatchError> {
        TimeRanges::get(ca_id).ok_or_else(|| Error::<T>::NoSuchBallot.into())
    }

    /// Ensure that `range.start <= range.end`.
    fn ensure_range_consistent(range: BallotTimeRange) -> DispatchResult {
        ensure!(range.start <= range.end, Error::<T>::StartAfterEnd);
        Ok(())
    }

    // Ensure that `start <= end`, `now <= end`, and `record_date <= voting start`.
    fn ensure_range_invariant(ca: &CorporateAction, range: BallotTimeRange) -> DispatchResult {
        Self::ensure_range_consistent(range)?;
        ensure!(
            <Checkpoint<T>>::now_unix() <= range.end,
            Error::<T>::NowAfterEnd
        );
        <CA<T>>::ensure_record_date_before_start(ca, range.start)
    }
}
