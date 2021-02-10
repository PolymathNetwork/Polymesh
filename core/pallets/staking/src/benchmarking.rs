// This file is part of Substrate.

// Copyright (C) 2020 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Staking pallet benchmarking.

use super::*;
use crate::Module as Staking;
use testing_utils::*;

pub use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use polymesh_common_utilities::benchs::UserBuilder;
use sp_runtime::traits::One;
const SEED: u32 = 0;
const MAX_SPANS: u32 = 100;
const MAX_VALIDATORS: u32 = 1000;
const MAX_SLASHES: u32 = 1000;

// Add slashing spans to a user account. Not relevant for actual use, only to benchmark
// read and write operations.
fn add_slashing_spans<T: Trait>(who: &T::AccountId, spans: u32) {
    if spans == 0 {
        return;
    }

    // For the first slashing span, we initialize
    let mut slashing_spans = crate::slashing::SlashingSpans::new(0);
    SpanSlash::<T>::insert((who, 0), crate::slashing::SpanRecord::default());

    for i in 1..spans {
        assert!(slashing_spans.end_span(i));
        SpanSlash::<T>::insert((who, i), crate::slashing::SpanRecord::default());
    }
    SlashingSpans::<T>::insert(who, slashing_spans);
}

fn add_perm_validator<T: Trait>(id: IdentityId, intended_count: Option<u32>) {
    Staking::<T>::set_validator_count(RawOrigin::Root.into(), 10)
        .expect("Failed to set the validator count");
    Staking::<T>::add_permissioned_validator(RawOrigin::Root.into(), id, intended_count)
        .expect("Failed to add permissioned validator");
}

// This function generates one validator being nominated by n nominators, and returns the validator
// stash account. It also starts an era and creates pending payouts.
pub fn create_validator_with_nominators<T: Trait>(
    n: u32,
    upper_bound: u32,
    dead: bool,
) -> Result<T::AccountId, DispatchError> {
    create_validator_with_nominators_with_balance::<T>(n, upper_bound, 10000u32.into(), dead)
}

// This function generates one validator being nominated by n nominators, and returns the validator
// stash account. It also starts an era and creates pending payouts.
// The balance is added to controller and stash accounts.
pub fn create_validator_with_nominators_with_balance<T: Trait>(
    n: u32,
    upper_bound: u32,
    balance: u32,
    dead: bool,
) -> Result<T::AccountId, DispatchError> {
    let mut points_total = 0;
    let mut points_individual = Vec::new();

    let (v_stash, v_controller) = create_stash_controller_with_balance::<T>(0, balance)?;
    let v_controller_origin = v_controller.origin();

    let validator_prefs = ValidatorPrefs {
        commission: Perbill::from_percent(50),
    };
    emulate_validator_setup::<T>(1, 10, Perbill::from_percent(60));
    Staking::<T>::add_permissioned_validator(RawOrigin::Root.into(), v_stash.did(), Some(2))
        .expect("Failed to add permissioned validator");
    Staking::<T>::validate(v_controller_origin.into(), validator_prefs.clone())
        .expect("Failed to validate");
    assert_eq!(
        Staking::<T>::validators(v_stash.account()),
        validator_prefs,
        "Failed to set the validator"
    );
    let stash_lookup = v_stash.lookup();

    points_total += 10;
    points_individual.push((v_stash.account(), 10));

    // Give the validator n nominators, but keep total users in the system the same.
    for i in 0..upper_bound {
        let (_n_stash, n_controller) = if !dead {
            create_stash_controller_with_balance::<T>(u32::max_value() - i, 100)?
        } else {
            create_stash_with_dead_controller::<T>(u32::max_value() - i, 100)?
        };
        if i < n {
            Staking::<T>::nominate(n_controller.origin().into(), vec![stash_lookup.clone()])?;
        }
    }

    ValidatorCount::put(1);

    // Start a new Era
    let new_validators = Staking::<T>::new_era(SessionIndex::one()).unwrap();

    assert!(new_validators.len() == 1);

    // Give Era Points
    let reward = EraRewardPoints::<T::AccountId> {
        total: points_total,
        individual: points_individual.into_iter().collect(),
    };

    let current_era = CurrentEra::get().unwrap();
    ErasRewardPoints::<T>::insert(current_era, reward);

    // Create reward pool
    let total_payout: BalanceOf<T> = 10000u32.into();
    <ErasValidatorReward<T>>::insert(current_era, total_payout);

    Ok(v_stash.account())
}

fn payout_stakers_<T: Trait>(
    alive: bool,
    n: u32,
) -> Result<(RawOrigin<T::AccountId>, T::AccountId, u32, BalanceOf<T>), DispatchError> {
    let validator = create_validator_with_nominators::<T>(
        n,
        T::MaxNominatorRewardedPerValidator::get() as u32,
        !alive,
    )?;
    let current_era = CurrentEra::get().unwrap();
    <ErasValidatorPrefs<T>>::insert(
        current_era,
        validator.clone(),
        <Staking<T>>::validators(&validator),
    );
    let caller = UserBuilder::<T>::default()
        .seed(n)
        .generate_did()
        .build("caller");
    let caller_key = frame_system::Account::<T>::hashed_key_for(&caller.account());
    frame_benchmarking::benchmarking::add_to_whitelist(caller_key.into());
    let balance_before = T::Currency::free_balance(&validator);
    Ok((caller.origin(), validator, current_era, balance_before))
}

benchmarks! {
    _{
        // User account seed
        let u in 0 .. 1000 => ();
    }

    bond {
        let stash = create_funded_user::<T>("stash", 2, 100);
        let controller = create_funded_user::<T>("controller", 5, 100);
        let controller_lookup = controller.lookup();
        let reward_destination = RewardDestination::Staked;
    }: _(stash.origin(), controller_lookup, 10u32.into(), reward_destination)
    verify {
        assert!(Bonded::<T>::contains_key(stash.account()));
        assert!(Ledger::<T>::contains_key(controller.account()));
    }

    bond_extra {
        let (stash, controller) = create_stash_controller::<T>(5, 1000)?;
        let max_additional = 200u32;
        let ledger = Ledger::<T>::get(&controller.account()).ok_or("ledger not created before")?;
        let original_bonded: BalanceOf<T> = ledger.active;
    }: _(stash.origin(), max_additional.into())
    verify {
        let ledger = Ledger::<T>::get(&controller.account()).ok_or("ledger not created after")?;
        let new_bonded: BalanceOf<T> = ledger.active;
        assert!(original_bonded < new_bonded);
    }

    unbond {
        let (_, controller) = create_stash_controller::<T>(500, 2000)?;
        let amount = 20u32;
        let ledger = Ledger::<T>::get(&controller.account()).ok_or("ledger not created before")?;
        let original_bonded: BalanceOf<T> = ledger.active;
    }: _(controller.origin(), amount.into())
    verify {
        let ledger = Ledger::<T>::get(&controller.account()).ok_or("ledger not created after")?;
        let new_bonded: BalanceOf<T> = ledger.active;
        assert!(original_bonded > new_bonded);
    }

    // Withdraw only updates the ledger
    withdraw_unbonded_update {
        // Slashing Spans
        let s in 0 .. MAX_SPANS;
        let (stash, controller) = create_stash_controller::<T>(0, 1000)?;
        add_slashing_spans::<T>(&stash.account(), s);
        let amount = 50u32; // Half of total
        Staking::<T>::unbond(controller.origin().into(), amount.into())?;
        CurrentEra::put(EraIndex::max_value());
        let ledger = Ledger::<T>::get(&controller.account()).ok_or("ledger not created before")?;
        let original_total: BalanceOf<T> = ledger.total;
    }: withdraw_unbonded(controller.origin(), s)
    verify {
        let ledger = Ledger::<T>::get(&controller.account()).ok_or("ledger not created after")?;
        let new_total: BalanceOf<T> = ledger.total;
        assert!(original_total > new_total);
    }

    // Worst case scenario, everything is removed after the bonding duration
    withdraw_unbonded_kill {
        // Slashing Spans
        let s in 0 .. MAX_SPANS;
        let (stash, controller) = create_stash_controller::<T>(0, 1000)?;
        add_slashing_spans::<T>(&stash.account(), s);
        let amount = 100u32;
        Staking::<T>::unbond(controller.origin().into(), amount.into())?;
        CurrentEra::put(EraIndex::max_value());
        let ledger = Ledger::<T>::get(&controller.account()).ok_or("ledger not created before")?;
        let original_total: BalanceOf<T> = ledger.total;
    }: withdraw_unbonded(controller.origin(), s)
    verify {
        assert!(!Ledger::<T>::contains_key(controller.account()));
    }

    set_min_bond_threshold {
        let origin = RawOrigin::Root;
    }: _(origin, 10000u32.into())
    verify {
        assert_eq!(Staking::<T>::min_bond_threshold(), 10000u32.into());
    }

    add_permissioned_validator {
        let (stash, controller) = create_stash_controller::<T>(5, 100)?;
        Staking::<T>::set_validator_count(RawOrigin::Root.into(), 10)?;
    }: _(RawOrigin::Root, stash.did(), Some(1))
    verify {
        let pref = Staking::<T>::permissioned_identity(stash.did());
        ensure!(pref.is_some(), "fail to add a permissioned identity");
        ensure!(pref.unwrap().intended_count == 1, "fail to set a incorrect intended count");
    }

    remove_permissioned_validator {
        let (stash, controller) = create_stash_controller::<T>(5, 100)?;
        add_perm_validator::<T>(stash.did(), Some(1));
    }: _(RawOrigin::Root, stash.did())
    verify {
        let pref = Staking::<T>::permissioned_identity(stash.did());
        ensure!(pref.is_none(), "fail to remove a permissioned identity");
    }

    set_commission_cap {
        let m in 0 .. MAX_ALLOWED_VALIDATORS;
        let mut stashes = Vec::with_capacity(m as usize);
        // Add validators
        for i in 0 .. m {
            let stash = create_funded_user::<T>("stash", i, 1000);
            stashes.push(stash.account());
            Validators::<T>::insert(stash.account(), ValidatorPrefs { commission: Perbill::from_percent(70)});
        }
    }: _(RawOrigin::Root, Perbill::from_percent(50))
    verify {
        stashes.iter().for_each(|s| {
            assert_eq!(Staking::<T>::validators(s), ValidatorPrefs { commission: Perbill::from_percent(50) });
        });
    }

    validate {
        Staking::<T>::set_min_bond_threshold(RawOrigin::Root.into(), 100u32.into())?;
        let (stash, controller) = create_stash_controller::<T>(70, 10000)?;
        add_perm_validator::<T>(stash.did(), Some(2));
        let prefs = ValidatorPrefs::default();
    }: _(controller.origin(), prefs)
    verify {
        assert!(Validators::<T>::contains_key(stash.account()));
    }

    // Worst case scenario, MAX_NOMINATIONS
    nominate {
        let n in 1 .. MAX_NOMINATIONS as u32;
        let (stash, controller) = create_stash_controller::<T>(n + 1, 10000)?;
        let validators = create_validators::<T>(n, 1000000)?;
    }: _(controller.origin(), validators)
    verify {
        assert!(Nominators::<T>::contains_key(stash.account()));
    }

    chill {
        let (_, controller) = create_stash_controller::<T>(10, 100)?;
    }: _(controller.origin())

    set_payee {
        let (stash, controller) = create_stash_controller::<T>(10, 100)?;
        assert_eq!(Payee::<T>::get(&stash.account()), RewardDestination::Staked);
    }: _(controller.origin(), RewardDestination::Controller)
    verify {
        assert_eq!(Payee::<T>::get(&stash.account()), RewardDestination::Controller);
    }

    set_controller {
        let (stash, _) = create_stash_controller::<T>(10, 100)?;
        let new_controller = create_funded_user::<T>("new_controller", 10, 100);
        let new_controller_lookup = new_controller.lookup();
    }: _(stash.origin(), new_controller_lookup)
    verify {
        assert!(Ledger::<T>::contains_key(&new_controller.account()));
    }

    set_validator_count {
        let c in 0 .. MAX_VALIDATORS;
    }: _(RawOrigin::Root, c)
    verify {
        assert_eq!(ValidatorCount::get(), c);
    }

    force_no_eras { }: _(RawOrigin::Root)
    verify { assert_eq!(ForceEra::get(), Forcing::ForceNone); }

    force_new_era { }: _(RawOrigin::Root)
    verify { assert_eq!(ForceEra::get(), Forcing::ForceNew); }

    force_new_era_always { }: _(RawOrigin::Root)
    verify { assert_eq!(ForceEra::get(), Forcing::ForceAlways); }

    // Worst case scenario, the list of invulnerables is very long.
    set_invulnerables {
        let v in 0 .. MAX_VALIDATORS;
        let mut invulnerables = Vec::new();
        for i in 0 .. v {
            invulnerables.push(account("invulnerable", i, SEED));
        }
    }: _(RawOrigin::Root, invulnerables)
    verify {
        assert_eq!(Invulnerables::<T>::get().len(), v as usize);
    }

    force_unstake {
        // Slashing Spans
        let s in 0 .. MAX_SPANS;
        let (stash, controller) = create_stash_controller::<T>(0, 1000)?;
        add_slashing_spans::<T>(&stash.account(), s);
    }: _(RawOrigin::Root, stash.account(), s)
    verify {
        assert!(!Ledger::<T>::contains_key(&controller.account()));
    }

    cancel_deferred_slash {
        let s in 1 .. MAX_SLASHES;
        let mut unapplied_slashes = Vec::new();
        let era = EraIndex::one();
        for _ in 0 .. MAX_SLASHES {
            unapplied_slashes.push(UnappliedSlash::<T::AccountId, BalanceOf<T>>::default());
        }
        UnappliedSlashes::<T>::insert(era, &unapplied_slashes);

        let slash_indices: Vec<u32> = (0 .. s).collect();
    }: _(RawOrigin::Root, era, slash_indices)
    verify {
        assert_eq!(UnappliedSlashes::<T>::get(&era).len(), (MAX_SLASHES - s) as usize);
    }

    payout_stakers {
        let n in 1 .. T::MaxNominatorRewardedPerValidator::get() as u32;
        let (origin, validator, current_era, balance_before) = payout_stakers_::<T>(false, n)?;
    }: _(origin, validator.clone(), current_era)
    verify {
        // Validator has been paid!
        let balance_after = T::Currency::free_balance(&validator);
        assert!(balance_before < balance_after);
    }

    payout_stakers_alive_controller {
        let n in 1 .. T::MaxNominatorRewardedPerValidator::get() as u32;
        let (origin, validator, current_era, balance_before) = payout_stakers_::<T>(true, n)?;
    }: payout_stakers(origin, validator.clone(), current_era)
    verify {
        // Validator has been paid!
        let balance_after = T::Currency::free_balance(&validator);
        assert!(balance_before < balance_after);
    }

    rebond {
        let l in 1 .. MAX_UNLOCKING_CHUNKS as u32;
        let (_, controller) = create_stash_controller::<T>(u, 1000)?;
        let mut staking_ledger = Ledger::<T>::get(controller.account()).unwrap();
        let unlock_chunk = UnlockChunk::<BalanceOf<T>> {
            value: 1u32.into(),
            era: EraIndex::zero(),
        };
        for _ in 0 .. l {
            staking_ledger.unlocking.push(unlock_chunk.clone())
        }
        Ledger::<T>::insert(controller.account(), staking_ledger.clone());
        let original_bonded: BalanceOf<T> = staking_ledger.active;
    }: _(controller.origin(), (l + 100).into())
    verify {
        let ledger = Ledger::<T>::get(&controller.account()).ok_or("ledger not created after")?;
        let new_bonded: BalanceOf<T> = ledger.active;
        assert!(original_bonded < new_bonded);
    }

    set_history_depth {
        let e in 1 .. 100;
        HistoryDepth::put(e);
        CurrentEra::put(e);
        for i in 0 .. e {
            <ErasStakers<T>>::insert(i, T::AccountId::default(), Exposure::<T::AccountId, BalanceOf<T>>::default());
            <ErasStakersClipped<T>>::insert(i, T::AccountId::default(), Exposure::<T::AccountId, BalanceOf<T>>::default());
            <ErasValidatorPrefs<T>>::insert(i, T::AccountId::default(), ValidatorPrefs::default());
            <ErasValidatorReward<T>>::insert(i, BalanceOf::<T>::one());
            <ErasRewardPoints<T>>::insert(i, EraRewardPoints::<T::AccountId>::default());
            <ErasTotalStake<T>>::insert(i, BalanceOf::<T>::one());
            ErasStartSessionIndex::insert(i, i);
        }
    }: _(RawOrigin::Root, EraIndex::zero(), u32::max_value())
    verify {
        assert_eq!(HistoryDepth::get(), 0);
    }

    reap_stash {
        let s in 1 .. MAX_SPANS;
        let (stash, controller) = create_stash_controller::<T>(0, 100)?;
        add_slashing_spans::<T>(&stash.account(), s);
        T::Currency::make_free_balance_be(&stash.account(), 0u32.into());
    }: _(controller.origin(), stash.account(), s)
    verify {
        assert!(!Bonded::<T>::contains_key(&stash.account()));
    }

    new_era {
        let v in 1 .. 10;
        let n in 1 .. 100;

        create_validators_with_nominators_for_era::<T>(v, n, MAX_NOMINATIONS, false, None)?;
        let session_index = SessionIndex::one();
    }: {
        let validators = Staking::<T>::new_era(session_index).ok_or("`new_era` failed")?;
        assert!(validators.len() == v as usize);
    }

    do_slash {
        let l in 1 .. MAX_UNLOCKING_CHUNKS as u32;
        let (stash, controller) = create_stash_controller::<T>(0, 100)?;
        let mut staking_ledger = Ledger::<T>::get(controller.account()).unwrap();
        let unlock_chunk = UnlockChunk::<BalanceOf<T>> {
            value: 1u32.into(),
            era: EraIndex::zero(),
        };
        for _ in 0 .. l {
            staking_ledger.unlocking.push(unlock_chunk.clone())
        }
        Ledger::<T>::insert(controller.account(), staking_ledger);
        let balance_before = T::Currency::free_balance(&stash.account());
    }: {
        crate::slashing::do_slash::<T>(
            &stash.account(),
            10u32.into(),
            &mut BalanceOf::<T>::zero(),
            &mut NegativeImbalanceOf::<T>::zero()
        );
    } verify {
        let balance_after = T::Currency::free_balance(&stash.account());
        assert!(balance_before > balance_after);
    }

    payout_all {
        let v in 1 .. 10;
        let n in 1 .. 100;
        create_validators_with_nominators_for_era::<T>(v, n, MAX_NOMINATIONS, false, None)?;
        // Start a new Era
        let new_validators = Staking::<T>::new_era(SessionIndex::one()).unwrap();
        assert!(new_validators.len() == v as usize);

        let current_era = CurrentEra::get().unwrap();
        let mut points_total = 0;
        let mut points_individual = Vec::new();
        let mut payout_calls_arg = Vec::new();

        for validator in new_validators.iter() {
            points_total += 10;
            points_individual.push((validator.clone(), 10));
            payout_calls_arg.push((validator.clone(), current_era));
        }

        // Give Era Points
        let reward = EraRewardPoints::<T::AccountId> {
            total: points_total,
            individual: points_individual.into_iter().collect(),
        };

        ErasRewardPoints::<T>::insert(current_era, reward);

        // Create reward pool
        let total_payout: BalanceOf<T> = 1000u32.into();
        <ErasValidatorReward<T>>::insert(current_era, total_payout);

        let caller: T::AccountId = whitelisted_caller();
    }: {
        for arg in payout_calls_arg {
            <Staking<T>>::payout_stakers(RawOrigin::Signed(caller.clone()).into(), arg.0, arg.1)?;
        }
    }

    // This benchmark create `v` validators intent, `n` nominators intent, each nominator nominate
    // MAX_NOMINATIONS in the set of the first `w` validators.
    // It builds a solution with `w` winners composed of nominated validators randomly nominated,
    // `a` assignment with MAX_NOMINATIONS.
    submit_solution_initial {
        // number of validator intent
        let v in 1000 .. 2000;
        // number of nominator intent
        let n in 1000 .. 2000;
        // number of assignments. Basically, number of active nominators.
        let a in 200 .. 500;
        // number of winners, also ValidatorCount
        let w in 16 .. 100;

        ensure!(w as usize >= MAX_NOMINATIONS, "doesn't support lower value");

        let winners = create_validators_with_nominators_for_era::<T>(
            v,
            n,
            MAX_NOMINATIONS,
            false,
            Some(w),
        )?;

        // needed for the solution to be generates.
        assert!(<Staking<T>>::create_stakers_snapshot().0);

        // set number of winners
        ValidatorCount::put(w);

        // create a assignments in total for the w winners.
        let (winners, assignments) = create_assignments_for_offchain::<T>(a, winners)?;

        let (
            winners,
            compact,
            score,
            size
        ) = offchain_election::prepare_submission::<T>(assignments, winners, false).unwrap();

        // needed for the solution to be accepted
        <EraElectionStatus<T>>::put(ElectionStatus::Open(T::BlockNumber::from(1u32)));

        let era = <Staking<T>>::current_era().unwrap_or(0);
        let caller = create_funded_user::<T>("caller", n, 10000);

        // Whitelist caller account from further DB operations.
        let caller_key = frame_system::Account::<T>::hashed_key_for(&caller.account());
        frame_benchmarking::benchmarking::add_to_whitelist(caller_key.into());
    }: {
        let result = <Staking<T>>::submit_election_solution(
            caller.origin().into(),
            winners,
            compact,
            score.clone(),
            era,
            size,
        );
        assert!(result.is_ok());
    }
    verify {
        // new solution has been accepted.
        assert_eq!(<Staking<T>>::queued_score().unwrap(), score);
    }

    // same as submit_solution_initial but we place a very weak solution on chian first.
    submit_solution_better {
        // number of validator intent
        let v in 1000 .. 2000;
        // number of nominator intent
        let n in 1000 .. 2000;
        // number of assignments. Basically, number of active nominators.
        let a in 200 .. 500;
        // number of winners, also ValidatorCount
        let w in 16 .. 100;

        ensure!(w as usize >= MAX_NOMINATIONS, "doesn't support lower value");

        let winners = create_validators_with_nominators_for_era::<T>(
            v,
            n,
            MAX_NOMINATIONS,
            false,
            Some(w),
        )?;

        // needed for the solution to be generates.
        assert!(<Staking<T>>::create_stakers_snapshot().0);

        // set number of winners
        ValidatorCount::put(w);

        // create a assignments in total for the w winners.
        let (winners, assignments) = create_assignments_for_offchain::<T>(a, winners)?;

        let single_winner = winners[0].0.clone();

        let (
            winners,
            compact,
            score,
            size
        ) = offchain_election::prepare_submission::<T>(assignments, winners, false).unwrap();

        // needed for the solution to be accepted
        <EraElectionStatus<T>>::put(ElectionStatus::Open(T::BlockNumber::from(1u32)));

        let era = <Staking<T>>::current_era().unwrap_or(0);
        let caller = create_funded_user::<T>("caller", n, 10000);

        // Whitelist caller account from further DB operations.
        let caller_key = frame_system::Account::<T>::hashed_key_for(&caller.account());
        frame_benchmarking::benchmarking::add_to_whitelist(caller_key.into());

        // submit a very bad solution on-chain
        {
            // this is needed to fool the chain to accept this solution.
            ValidatorCount::put(1);
            let (winners, compact, score, size) = get_single_winner_solution::<T>(single_winner)?;
            assert!(
                <Staking<T>>::submit_election_solution(
                    caller.origin().into(),
                    winners,
                    compact,
                    score.clone(),
                    era,
                    size,
            ).is_ok());

            // new solution has been accepted.
            assert_eq!(<Staking<T>>::queued_score().unwrap(), score);
            ValidatorCount::put(w);
        }
    }: {
        let result = <Staking<T>>::submit_election_solution(
            caller.origin().into(),
            winners,
            compact,
            score.clone(),
            era,
            size,
        );
        assert!(result.is_ok());
    }
    verify {
        // new solution has been accepted.
        assert_eq!(<Staking<T>>::queued_score().unwrap(), score);
    }

    // This will be early rejected based on the score.
    submit_solution_weaker {
        // number of validator intent
        let v in 1000 .. 2000;
        // number of nominator intent
        let n in 1000 .. 2000;

        create_validators_with_nominators_for_era::<T>(v, n, MAX_NOMINATIONS, false, None)?;

        // needed for the solution to be generates.
        assert!(<Staking<T>>::create_stakers_snapshot().0);

        // needed for the solution to be accepted
        <EraElectionStatus<T>>::put(ElectionStatus::Open(T::BlockNumber::from(1u32)));
        let caller = create_funded_user::<T>("caller", n, 10000);
        let era = <Staking<T>>::current_era().unwrap_or(0);

        // Whitelist caller account from further DB operations.
        let caller_key = frame_system::Account::<T>::hashed_key_for(&caller.account());
        frame_benchmarking::benchmarking::add_to_whitelist(caller_key.into());

        // submit a seq-phragmen with all the good stuff on chain.
        {
            let (winners, compact, score, size) = get_seq_phragmen_solution::<T>(true);
            assert!(
                <Staking<T>>::submit_election_solution(
                    caller.origin().into(),
                    winners,
                    compact,
                    score.clone(),
                    era,
                    size,
                ).is_ok()
            );

            // new solution has been accepted.
            assert_eq!(<Staking<T>>::queued_score().unwrap(), score);
        }

        // prepare a bad solution. This will be very early rejected.
        let (winners, compact, score, size) = get_weak_solution::<T>(true);
    }: {
        assert!(
            <Staking<T>>::submit_election_solution(
                caller.origin().into(),
                winners,
                compact,
                score.clone(),
                era,
                size,
            ).is_err()
        );
    }

    change_slashing_allowed_for {

    }: _(RawOrigin::Root, SlashingSwitch::ValidatorAndNominator)
    verify {
        assert!(Staking::<T>::slashing_allowed_for() == SlashingSwitch::ValidatorAndNominator, "Incorrect value set");
    }

    update_permissioned_validator_intended_count {
        let (stash, controller) = create_stash_controller::<T>(5, 100)?;
        let stash_id = stash.did();
        add_perm_validator::<T>(stash_id, Some(1));
    }: _(RawOrigin::Root, stash_id, 2)
    verify {
        assert!(Staking::<T>::permissioned_identity(stash_id).unwrap().intended_count == 2, "Unable to update intended validator count");
    }

    increase_validator_count {
        Staking::<T>::set_validator_count(RawOrigin::Root.into(), 10)
        .expect("Failed to set the validator count");
    }: _(RawOrigin::Root, 15)
    verify {
        assert_eq!(Staking::<T>::validator_count(), 25);
    }

    scale_validator_count {
        Staking::<T>::set_validator_count(RawOrigin::Root.into(), 10)
        .expect("Failed to set the validator count");
    }: _(RawOrigin::Root, Percent::from_percent(25))
    verify {
        assert_eq!(Staking::<T>::validator_count(), 12);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{Balances, ExtBuilder, Origin, Staking, Test};
    use frame_support::assert_ok;

    #[test]
    fn create_validators_with_nominators_for_era_works() {
        ExtBuilder::default()
            .has_stakers(false)
            .build()
            .execute_with(|| {
                let v = 10;
                let n = 100;

                create_validators_with_nominators_for_era::<Test>(
                    v,
                    n,
                    MAX_NOMINATIONS,
                    false,
                    None,
                )
                .unwrap();

                let count_validators = Validators::<Test>::iter().count();
                let count_nominators = Nominators::<Test>::iter().count();

                assert_eq!(count_validators, v as usize);
                assert_eq!(count_nominators, n as usize);
            });
    }

    #[test]
    fn create_validator_with_nominators_works() {
        ExtBuilder::default()
            .has_stakers(false)
            .build()
            .execute_with(|| {
                let n = 10;

                let validator_stash = create_validator_with_nominators::<Test>(
                    n,
                    <Test as Trait>::MaxNominatorRewardedPerValidator::get() as u32,
                    false,
                )
                .unwrap();

                let current_era = CurrentEra::get().unwrap();

                let original_free_balance = Balances::free_balance(&validator_stash);
                assert_ok!(Staking::payout_stakers(
                    Origin::signed(1337),
                    validator_stash,
                    current_era
                ));
                let new_free_balance = Balances::free_balance(&validator_stash);

                assert!(original_free_balance < new_free_balance);
            });
    }

    #[test]
    fn add_slashing_spans_works() {
        ExtBuilder::default()
            .has_stakers(false)
            .build()
            .execute_with(|| {
                let n = 10;

                let validator_stash = create_validator_with_nominators::<Test>(
                    n,
                    <Test as Trait>::MaxNominatorRewardedPerValidator::get() as u32,
                    false,
                )
                .unwrap();

                // Add 20 slashing spans
                let num_of_slashing_spans = 20;
                add_slashing_spans::<Test>(&validator_stash, num_of_slashing_spans);

                let slashing_spans = SlashingSpans::<Test>::get(&validator_stash).unwrap();
                assert_eq!(
                    slashing_spans.iter().count(),
                    num_of_slashing_spans as usize
                );
                for i in 0..num_of_slashing_spans {
                    assert!(SpanSlash::<Test>::contains_key((&validator_stash, i)));
                }

                // Test everything is cleaned up
                assert_ok!(Staking::kill_stash(&validator_stash, num_of_slashing_spans));
                assert!(SlashingSpans::<Test>::get(&validator_stash).is_none());
                for i in 0..num_of_slashing_spans {
                    assert!(!SpanSlash::<Test>::contains_key((&validator_stash, i)));
                }
            });
    }

    #[test]
    fn test_payout_all() {
        ExtBuilder::default()
            .has_stakers(false)
            .build()
            .execute_with(|| {
                let v = 10;
                let n = 100;

                let selected_benchmark = SelectedBenchmark::payout_all;
                let c = vec![
                    (frame_benchmarking::BenchmarkParameter::v, v),
                    (frame_benchmarking::BenchmarkParameter::n, n),
                ];
                let closure_to_benchmark =
                    <SelectedBenchmark as frame_benchmarking::BenchmarkingSetup<Test>>::instance(
                        &selected_benchmark,
                        &c,
                    )
                    .unwrap();

                assert_ok!(closure_to_benchmark());
            });
    }

    #[test]
    fn test_benchmarks() {
        ExtBuilder::default()
            .has_stakers(false)
            .build()
            .execute_with(|| {
                assert_ok!(test_benchmark_bond::<Test>());
                assert_ok!(test_benchmark_bond_extra::<Test>());
                assert_ok!(test_benchmark_unbond::<Test>());
                assert_ok!(test_benchmark_withdraw_unbonded_update::<Test>());
                assert_ok!(test_benchmark_withdraw_unbonded_kill::<Test>());
                assert_ok!(test_benchmark_validate::<Test>());
                assert_ok!(test_benchmark_nominate::<Test>());
                assert_ok!(test_benchmark_chill::<Test>());
                assert_ok!(test_benchmark_set_payee::<Test>());
                assert_ok!(test_benchmark_set_controller::<Test>());
                assert_ok!(test_benchmark_set_validator_count::<Test>());
                assert_ok!(test_benchmark_force_no_eras::<Test>());
                assert_ok!(test_benchmark_force_new_era::<Test>());
                assert_ok!(test_benchmark_force_new_era_always::<Test>());
                assert_ok!(test_benchmark_set_invulnerables::<Test>());
                assert_ok!(test_benchmark_force_unstake::<Test>());
                assert_ok!(test_benchmark_cancel_deferred_slash::<Test>());
                assert_ok!(test_benchmark_payout_stakers::<Test>());
                assert_ok!(test_benchmark_payout_stakers_alive_controller::<Test>());
                assert_ok!(test_benchmark_rebond::<Test>());
                assert_ok!(test_benchmark_set_history_depth::<Test>());
                assert_ok!(test_benchmark_reap_stash::<Test>());
                assert_ok!(test_benchmark_new_era::<Test>());
                assert_ok!(test_benchmark_do_slash::<Test>());
                assert_ok!(test_benchmark_payout_all::<Test>());
                // only run one of them to same time on the CI. ignore the other two.
                assert_ok!(test_benchmark_submit_solution_initial::<Test>());
            });
    }

    #[test]
    #[ignore]
    fn test_benchmarks_offchain() {
        ExtBuilder::default()
            .has_stakers(false)
            .build()
            .execute_with(|| {
                assert_ok!(test_benchmark_submit_solution_better::<Test>());
                assert_ok!(test_benchmark_submit_solution_weaker::<Test>());
            });
    }
}
