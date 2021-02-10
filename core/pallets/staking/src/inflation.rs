// This file is part of Substrate.

// Copyright (C) 2019-2020 Parity Technologies (UK) Ltd.
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

//! This module expose one function `P_NPoS` (Payout NPoS) or `compute_total_payout` which returns
//! the total payout for the era given the era duration and the staking rate in NPoS.
//! The staking rate in NPoS is the total amount of tokens staked by nominators and validators,
//! divided by the total token supply.

use sp_runtime::{curve::PiecewiseLinear, traits::AtLeast32BitUnsigned, Perbill};

/// The total payout to all validators (and their nominators) per era and maximum payout.
///
/// Defined as such:
/// `staker-payout = yearly_inflation(npos_token_staked / total_tokens) * total_tokens / era_per_year`
/// `maximum-payout = max_yearly_inflation * total_tokens / era_per_year`
///
/// `era_duration` is expressed in millisecond.
pub fn compute_total_payout<N>(
    yearly_inflation: &PiecewiseLinear<'static>,
    npos_token_staked: N,
    total_tokens: N,
    era_duration: u64,
    max_inflated_issuance: N,
    non_inflated_yearly_reward: N,
) -> (N, N)
where
    N: AtLeast32BitUnsigned + Clone,
{
    // Milliseconds per year for the Julian year (365.25 days).
    const MILLISECONDS_PER_YEAR: u64 = 1000 * 3600 * 24 * 36525 / 100;
    let portion = Perbill::from_rational_approximation(era_duration as u64, MILLISECONDS_PER_YEAR);
    // Have fixed rewards kicked in?
    if total_tokens >= max_inflated_issuance {
        let payout = portion * non_inflated_yearly_reward;
        // payout is always maximum.
        (payout.clone(), payout)
    } else {
        let payout = portion
            * yearly_inflation
                .calculate_for_fraction_times_denominator(npos_token_staked, total_tokens.clone());
        let maximum = portion * (yearly_inflation.maximum * total_tokens);
        (payout, maximum)
    }
}

#[cfg(test)]
mod test {
    use sp_runtime::{curve::PiecewiseLinear, Perbill};

    pallet_staking_reward_curve::build! {
        const I_NPOS: PiecewiseLinear<'_> = curve!(
            min_inflation: 0_025_000,
            max_inflation: 0_100_000,
            ideal_stake: 0_500_000,
            falloff: 0_050_000,
            max_piece_count: 40,
            test_precision: 0_005_000,
        );
    }

    #[test]
    fn npos_curve_is_sensible() {
        const YEAR: u64 = 365 * 24 * 60 * 60 * 1000;
        const DAY: u64 = 24 * 60 * 60 * 1000;
        const SIX_HOURS: u64 = 6 * 60 * 60 * 1000;
        const HOUR: u64 = 60 * 60 * 1000;
        const MAX_VARIABLE_INFLATION_TOTAL_ISSUANCE: u64 = 1_000_000_000;
        const FIXED_YEARLY_REWARD: u64 = 1_000_000;

        let assert_payout = |t_staked, era_duration, expected_payout| {
            assert_eq!(
                super::compute_total_payout(
                    &I_NPOS,
                    t_staked,
                    100_000u64,
                    era_duration,
                    MAX_VARIABLE_INFLATION_TOTAL_ISSUANCE,
                    FIXED_YEARLY_REWARD
                )
                .0,
                expected_payout
            );
        };

        // check maximum inflation.
        // not 10_000 due to rounding error.
        assert_eq!(
            super::compute_total_payout(
                &I_NPOS,
                0,
                100_000u64,
                YEAR,
                MAX_VARIABLE_INFLATION_TOTAL_ISSUANCE,
                FIXED_YEARLY_REWARD
            )
            .1,
            9_993
        );

        assert_payout(0, YEAR, 2_498);
        assert_payout(5_000, YEAR, 3_248);
        assert_payout(25_000, YEAR, 6_246);
        assert_payout(40_000, YEAR, 8_494);
        assert_payout(50_000, YEAR, 9_993);
        assert_payout(60_000, YEAR, 4_379);
        assert_payout(75_000, YEAR, 2_733);
        assert_payout(95_000, YEAR, 2_513);
        assert_payout(100_000, YEAR, 2_505);
        assert_payout(25_000, DAY, 17);
        assert_payout(50_000, DAY, 27);
        assert_payout(75_000, DAY, 7);
        assert_payout(25_000, SIX_HOURS, 4);
        assert_payout(50_000, SIX_HOURS, 7);
        assert_payout(75_000, SIX_HOURS, 2);

        assert_eq!(
            super::compute_total_payout(
                &I_NPOS,
                2_500_000_000_000_000_000_000_000_000u128,
                5_000_000_000_000_000_000_000_000_000u128,
                HOUR,
                7_000_000_000_000_000_000_000_000_000u128,
                FIXED_YEARLY_REWARD.into()
            )
            .0,
            57_038_500_000_000_000_000_000
        );

        assert_eq!(
            super::compute_total_payout(
                &I_NPOS,
                1_000_000,
                1_074_582_300_000_000u128,
                SIX_HOURS,
                1_000_000_000_000_000u128,
                Perbill::from_percent(5) * 1_000_000_000_000_000u128
            ),
            (34223100000, 34223100000)
        );
    }
}
