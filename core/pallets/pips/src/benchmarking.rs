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

use crate::*;

use frame_benchmarking::benchmarks;
use frame_support::{
    dispatch::{DispatchError, DispatchResult},
    ensure,
    traits::UnfilteredDispatchable,
};
use frame_system::RawOrigin;
use polymesh_common_utilities::{
    benchs::{user, User},
    MaybeBlock, SystematicIssuers, GC_DID,
};
use rand::{seq::SliceRandom, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sp_std::{
    convert::{TryFrom, TryInto},
    iter,
    prelude::*,
};

#[cfg(feature = "running-ci")]
mod limits {
    pub const DESCRIPTION_LEN: usize = 10;
    pub const URL_LEN: usize = 50;
    pub const PROPOSAL_PADDING_LEN: usize = 100;
    pub const VOTERS_A_NUM: usize = 10;
    pub const VOTERS_B_NUM: usize = 10;
    pub const PROPOSALS_NUM: usize = 5;
}

#[cfg(not(feature = "running-ci"))]
mod limits {
    pub const DESCRIPTION_LEN: usize = 1_000;
    pub const URL_LEN: usize = 500;
    pub const PROPOSAL_PADDING_LEN: usize = 10_000;
    pub const VOTERS_A_NUM: usize = 200;
    pub const VOTERS_B_NUM: usize = 200;
    pub const PROPOSALS_NUM: usize = 100;
}

use limits::*;

pub const MAX_SKIPPED_COUNT: u8 = 255;

/// Makes a proposal.
fn make_proposal<T: Trait>() -> (Box<T::Proposal>, Url, PipDescription) {
    let content = vec![b'X'; PROPOSAL_PADDING_LEN];
    let proposal = Box::new(frame_system::Call::<T>::remark(content).into());
    let url = Url::try_from(vec![b'X'; URL_LEN].as_slice()).unwrap();
    let description = PipDescription::try_from(vec![b'X'; DESCRIPTION_LEN].as_slice()).unwrap();
    (proposal, url, description)
}

/// Creates voters with seeds from 1 to `num_voters` inclusive.
fn make_voters<T: Trait>(
    num_voters: usize,
    prefix: &'static str,
) -> Vec<(T::AccountId, RawOrigin<T::AccountId>, IdentityId)> {
    (1..=num_voters)
        .map(|i| {
            let User {
                account,
                origin,
                did,
                ..
            } = user::<T>(prefix, i as u32);
            (account, origin, did.unwrap())
        })
        .collect()
}

/// Casts an `aye_or_nay` vote from each of the given voters.
fn cast_votes<T: Trait>(
    id: PipId,
    voters: &[(T::AccountId, RawOrigin<T::AccountId>, IdentityId)],
    aye_or_nay: bool,
) -> DispatchResult {
    for (_, origin, did) in voters {
        identity::CurrentDid::put(did);
        Module::<T>::vote(origin.clone().into(), id, aye_or_nay, 1u32.into())?;
    }
    Ok(())
}

/// Sets up PIPs and votes.
fn pips_and_votes_setup<T: Trait>(
    approve_only: bool,
) -> Result<(RawOrigin<T::AccountId>, IdentityId), DispatchError> {
    Module::<T>::set_active_pip_limit(RawOrigin::Root.into(), PROPOSALS_NUM as u32)?;
    Module::<T>::set_min_proposal_deposit(RawOrigin::Root.into(), 0u32.into())?;
    let (voters_a_num, voters_b_num) = if approve_only {
        (VOTERS_A_NUM + VOTERS_B_NUM, 0)
    } else {
        (VOTERS_A_NUM, VOTERS_B_NUM)
    };
    let hi_voters = make_voters::<T>(voters_a_num, "hi");
    let bye_voters = make_voters::<T>(voters_b_num, "bye");
    let User { origin, did, .. } = user::<T>("initial", 0);
    let did = did.ok_or("no did in pips_and_votes_setup")?;
    for i in 0..PROPOSALS_NUM {
        let (proposal, url, description) = make_proposal::<T>();
        // Pick a proposer, diversifying like a poor man.
        let (proposer_origin, proposer_did) = if hi_voters.len() >= i + 1 {
            (hi_voters[i].1.clone(), hi_voters[i].2.clone())
        } else {
            (origin.clone(), did)
        };
        identity::CurrentDid::put(proposer_did);
        Module::<T>::propose(
            proposer_origin.into(),
            proposal,
            42u32.into(),
            Some(url.clone()),
            Some(description.clone()),
        )?;
        // Alternate aye and nay voters with every iteration unless only approve votes are cast.
        cast_votes::<T>(i as u32, hi_voters.as_slice(), approve_only || i % 2 == 0)?;
        cast_votes::<T>(i as u32, bye_voters.as_slice(), i % 2 != 0)?;
    }
    identity::CurrentDid::kill();
    Ok((origin, did))
}

fn enact_call<T: Trait>(num_approves: usize, num_rejects: usize, num_skips: usize) -> Call<T> {
    let seed = [42; 32];
    let mut rng = ChaCha20Rng::from_seed(seed);
    let mut snapshot_results: Vec<_> = iter::repeat(SnapshotResult::Approve)
        .take(num_approves)
        .chain(iter::repeat(SnapshotResult::Reject).take(num_rejects))
        .chain(iter::repeat(SnapshotResult::Skip).take(num_skips))
        .collect();
    snapshot_results.shuffle(&mut rng);
    Call::<T>::enact_snapshot_results(
        Module::<T>::snapshot_queue()
            .iter()
            .rev()
            .map(|s| s.id)
            .zip(snapshot_results.into_iter())
            .collect(),
    )
}

fn propose_verify<T: Trait>(url: Url, description: PipDescription) -> DispatchResult {
    let meta = Module::<T>::proposal_metadata(0).unwrap();
    ensure!(0 == meta.id, "incorrect meta.id");
    ensure!(Some(url) == meta.url, "incorrect meta.url");
    ensure!(
        Some(description) == meta.description,
        "incorrect meta.description"
    );
    Ok(())
}

fn execute_verify<T: Trait>(state: ProposalState, err: &'static str) -> DispatchResult {
    if Proposals::<T>::contains_key(&0) {
        ensure!(state == Module::<T>::proposals(&0).unwrap().state, err);
    }
    Ok(())
}

benchmarks! {
    _ {}

    set_prune_historical_pips {
        let origin = RawOrigin::Root;
    }: _(origin, true)
    verify {
        ensure!(PruneHistoricalPips::get(), "set_prune_historical_pips didn't work");
    }

    set_min_proposal_deposit {
        let origin = RawOrigin::Root;
        let deposit = 42u32.into();
    }: _(origin, deposit)
    verify {
        ensure!(deposit == MinimumProposalDeposit::<T>::get(), "incorrect MinimumProposalDeposit");
    }

    set_default_enactment_period {
        let origin = RawOrigin::Root;
        let period = 42u32.into();
    }: _(origin, period)
    verify {
        ensure!(period == DefaultEnactmentPeriod::<T>::get(), "incorrect DefaultEnactmentPeriod");
    }

    set_pending_pip_expiry {
        let origin = RawOrigin::Root;
        let maybe_block = MaybeBlock::Some(42u32.into());
    }: _(origin, maybe_block)
    verify {
        ensure!(maybe_block == PendingPipExpiry::<T>::get(), "incorrect PendingPipExpiry");
    }

    set_max_pip_skip_count {
        let origin = RawOrigin::Root;
        let count = 42.try_into().unwrap();
    }: _(origin, count)
    verify {
        ensure!(count == MaxPipSkipCount::get(), "incorrect MaxPipSkipCount");
    }

    set_active_pip_limit {
        let origin = RawOrigin::Root;
        let pip_limit = 42;
    }: _(origin, pip_limit)
    verify {
        ensure!(pip_limit == ActivePipLimit::get(), "incorrect ActivePipLimit");
    }

    propose_from_community {
        let user = user::<T>("proposer", 0);
        identity::CurrentDid::put(user.did());
        let (proposal, url, description) = make_proposal::<T>();
        let some_url = Some(url.clone());
        let some_desc = Some(description.clone());
        let origin = user.origin();
    }: propose(origin, proposal, 42u32.into(), some_url, some_desc)
    verify {
        propose_verify::<T>(url, description)?;
    }

    // `propose` from a committee origin.
    propose_from_committee {
        let user = user::<T>("proposer", 0);
        identity::CurrentDid::put(user.did());
        let (proposal, url, description) = make_proposal::<T>();
        let origin = T::UpgradeCommitteeVMO::successful_origin();
        Module::<T>::set_min_proposal_deposit(RawOrigin::Root.into(), 0u32.into())?;
        let some_url = Some(url.clone());
        let some_desc = Some(description.clone());
        let call = Call::<T>::propose(proposal, 0u32.into(), some_url, some_desc);
    }: {
        call.dispatch_bypass_filter(origin)?;
    }
    verify {
        propose_verify::<T>(url, description)?;
    }

    vote {
        let proposer = user::<T>("proposer", 0);
        identity::CurrentDid::put(proposer.did());
        let (proposal, url, description) = make_proposal::<T>();
        Module::<T>::propose(
            proposer.origin().into(),
            proposal,
            42u32.into(),
            Some(url),
            Some(description)
        )?;
        // Populate vote history.
        let aye_voters = make_voters::<T>(VOTERS_A_NUM, "aye");
        let nay_voters = make_voters::<T>(VOTERS_B_NUM, "nay");
        cast_votes::<T>(0, aye_voters.as_slice(), true)?;
        cast_votes::<T>(0, nay_voters.as_slice(), false)?;
        // Cast an opposite vote.
        let voter = user::<T>("voter", 0);
        identity::CurrentDid::put(voter.did());
        let voter_deposit = 43u32.into();
        // Cast an opposite vote.
        Module::<T>::vote(voter.origin().into(), 0, false, voter_deposit)?;
        let origin = voter.origin();
    }: _(origin, 0, true, voter_deposit)
    verify {
        ensure!(voter_deposit == Deposits::<T>::get(0, &voter.account()).amount, "incorrect voter deposit");
    }

    approve_committee_proposal {
        let (proposal, url, description) = make_proposal::<T>();
        let proposer_origin = T::UpgradeCommitteeVMO::successful_origin();
        let proposer_did = SystematicIssuers::Committee.as_id();
        identity::CurrentDid::put(proposer_did);
        Module::<T>::set_min_proposal_deposit(RawOrigin::Root.into(), 0u32.into())?;
        let propose_call = Call::<T>::propose(proposal, 0u32.into(), Some(url.clone()), Some(description.clone()));
        propose_call.dispatch_bypass_filter(proposer_origin)?;
        let origin = T::VotingMajorityOrigin::successful_origin();
        let call = Call::<T>::approve_committee_proposal(0);
    }: {
        call.dispatch_bypass_filter(origin)?;
    }
    verify {
        ensure!(PipToSchedule::<T>::contains_key(&0), "approved committee proposal is not in the schedule");
    }

    reject_proposal {
        Module::<T>::set_prune_historical_pips(RawOrigin::Root.into(), true)?;
        let user = user::<T>("proposer", 0);
        identity::CurrentDid::put(user.did());
        let (proposal, url, description) = make_proposal::<T>();
        let deposit = 42u32.into();
        Module::<T>::propose(
            user.origin().into(),
            proposal,
            deposit,
            Some(url),
            Some(description)
        )?;
        ensure!(deposit == Deposits::<T>::get(&0, &user.account()).amount, "incorrect deposit in reject_proposal");
        let vmo_origin = T::VotingMajorityOrigin::successful_origin();
        let call = Call::<T>::reject_proposal(0);
    }: {
        call.dispatch_bypass_filter(vmo_origin)?;
    }
    verify {
        ensure!(!Deposits::<T>::contains_key(&0, &user.account()), "deposit of the rejected proposal is present");
    }

    prune_proposal {
        Module::<T>::set_prune_historical_pips(RawOrigin::Root.into(), false)?;
        let user = user::<T>("proposer", 0);
        identity::CurrentDid::put(user.did());
        let (proposal, url, description) = make_proposal::<T>();
        Module::<T>::propose(
            user.origin().into(),
            proposal,
            42u32.into(),
            Some(url),
            Some(description)
        )?;
        let vmo_origin = T::VotingMajorityOrigin::successful_origin();
        let reject_call = Call::<T>::reject_proposal(0);
        reject_call.dispatch_bypass_filter(vmo_origin.clone())?;
        let call = Call::<T>::prune_proposal(0);
    }: {
        call.dispatch_bypass_filter(vmo_origin)?;
    }
    verify {
        ensure!(!Proposals::<T>::contains_key(&0), "pruned proposal is present");
        ensure!(!ProposalMetadata::<T>::contains_key(&0), "pruned proposal metadata is present");
    }

    reschedule_execution {
        let user = user::<T>("proposer", 0);
        identity::CurrentDid::put(user.did());
        let (proposal, url, description) = make_proposal::<T>();
        Module::<T>::propose(
            user.origin().into(),
            proposal,
            42u32.into(),
            Some(url.clone()),
            Some(description.clone())
        )?;
        T::GovernanceCommittee::bench_set_release_coordinator(user.did());
        Module::<T>::snapshot(user.origin().into())?;
        let vmo_origin = T::VotingMajorityOrigin::successful_origin();
        let enact_call = Call::<T>::enact_snapshot_results(vec![(0, SnapshotResult::Approve)]);
        enact_call.dispatch_bypass_filter(vmo_origin)?;
        let future_block = frame_system::Module::<T>::block_number() + 100u32.into();
        let origin = user.origin();
    }: _(origin, 0, Some(future_block))
    verify {
        ensure!(PipToSchedule::<T>::contains_key(&0), "rescheduled proposal is missing in the schedule");
    }

    clear_snapshot {
        let user = user::<T>("proposer", 0);
        identity::CurrentDid::put(user.did());
        let (proposal, url, description) = make_proposal::<T>();
        Module::<T>::propose(
            user.origin().into(),
            proposal,
            42u32.into(),
            Some(url.clone()),
            Some(description.clone())
        )?;
        T::GovernanceCommittee::bench_set_release_coordinator(user.did());
        Module::<T>::snapshot(user.origin().into())?;
        ensure!(SnapshotMeta::<T>::get().is_some(), "missing a snapshot before clear_snapshot");
        let origin = user.origin();
    }: _(origin)
    verify {
        ensure!(SnapshotMeta::<T>::get().is_none(), "snapshot was not cleared by clear_snapshot");
    }

    snapshot {
        let (origin0, did0) = pips_and_votes_setup::<T>(true)?;
        identity::CurrentDid::put(did0);
        T::GovernanceCommittee::bench_set_release_coordinator(did0);
    }: _(origin0)
    verify {
        ensure!(SnapshotMeta::<T>::get().is_some(), "snapshot finished incorrectly");
    }

    enact_snapshot_results {
        // The number of Approve results.
        let a in 0..PROPOSALS_NUM as u32 / 3;
        // The number of Reject results.
        let r in 0..PROPOSALS_NUM as u32 / 3;
        // The number of Skip results.
        let s in 0..PROPOSALS_NUM as u32 / 3;

        Module::<T>::set_max_pip_skip_count(RawOrigin::Root.into(), MAX_SKIPPED_COUNT)?;
        let (origin0, did0) = pips_and_votes_setup::<T>(true)?;

        // snapshot
        identity::CurrentDid::put(did0);
        T::GovernanceCommittee::bench_set_release_coordinator(did0);
        Module::<T>::snapshot(origin0.into())?;

        // enact
        let enact_origin = T::VotingMajorityOrigin::successful_origin();
        let enact_call = enact_call::<T>(a as usize, r as usize, s as usize);
    }: {
        enact_call.dispatch_bypass_filter(enact_origin)?;
    }
    verify {
        ensure!(
            Module::<T>::snapshot_queue().len() == PROPOSALS_NUM - (a + r + s) as usize,
            "incorrect snapshot queue after enact_snapshot_results"
        );
    }

    execute_scheduled_pip {
        // set up
        Module::<T>::set_prune_historical_pips(RawOrigin::Root.into(), true)?;
        let (origin0, did0) = pips_and_votes_setup::<T>(true)?;

        // snapshot
        identity::CurrentDid::put(did0);
        T::GovernanceCommittee::bench_set_release_coordinator(did0);
        Module::<T>::snapshot(origin0.into())?;
        ensure!(
            Module::<T>::snapshot_queue().len() == PROPOSALS_NUM as usize,
            "wrong snapshot queue length"
        );

        // enact
        let enact_origin = T::VotingMajorityOrigin::successful_origin();
        let enact_call = enact_call::<T>(PROPOSALS_NUM, 0, 0);
        enact_call.dispatch_bypass_filter(enact_origin)?;

        // execute
        identity::CurrentDid::kill();
        let origin = RawOrigin::Root;
    }: _(origin, 0)
    verify {
        execute_verify::<T>(ProposalState::Failed, "incorrect proposal state after execution")?;
    }

    expire_scheduled_pip {
        // set up
        Module::<T>::set_prune_historical_pips(RawOrigin::Root.into(), true)?;
        let (origin0, did0) = pips_and_votes_setup::<T>(true)?;

        // snapshot
        identity::CurrentDid::put(did0);
        T::GovernanceCommittee::bench_set_release_coordinator(did0);
        Module::<T>::snapshot(origin0.into())?;

        ensure!(
            ProposalState::Pending == Module::<T>::proposals(&0).unwrap().state,
            "incorrect proposal state before expiration"
        );
        let origin = RawOrigin::Root;
    }: _(origin, GC_DID, 0)
    verify {
        execute_verify::<T>(ProposalState::Expired, "incorrect proposal state after expiration")?;
    }
}
