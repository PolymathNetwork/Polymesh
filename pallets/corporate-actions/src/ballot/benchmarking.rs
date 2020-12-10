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

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use core::iter;
use crate::benchmarking::setup_ca;
use pallet_identity::benchmarking::User;
use frame_benchmarking::benchmarks;
use frame_support::assert_ok;

const MAX_MOTIONS: u32 = 10;
const MAX_CHOICES: u32 = 10;

const RANGE: BallotTimeRange = BallotTimeRange { start: 3000, end: 4000 };

fn meta(i: u32, j: u32) -> BallotMeta {
    let motion = Motion {
        title: "".into(),
        info_link: "".into(),
        choices: iter::repeat("".into()).take(j as usize).collect(),
    };
    let motions = iter::repeat(motion).take(i as usize).collect();
    BallotMeta { title: "".into(), motions }
}

fn attach<T: Trait>(i: u32, j: u32) -> (User<T>, CAId) {
    let meta = meta(i, j);
    let (owner, ca_id) = setup_ca::<T>(CAKind::IssuerNotice);
    assert_ok!(<Module<T>>::attach_ballot(owner.origin().into(), ca_id, RANGE, meta, true));
    (owner, ca_id)
}

benchmarks! {
    _ {}

    attach_ballot {
        let i in 0..MAX_MOTIONS;
        let j in 0..MAX_CHOICES;

        let meta = meta(i, j);
        let (owner, ca_id) = setup_ca::<T>(CAKind::IssuerNotice);
    }: _(owner.origin(), ca_id, RANGE, meta, true)
    verify {
        ensure!(TimeRanges::get(ca_id) == Some(RANGE), "ballot not created");
    }

    change_end {
        let (owner, ca_id) = attach::<T>(0, 0);
    }: _(owner.origin(), ca_id, 5000)
    verify {
        ensure!(TimeRanges::get(ca_id).unwrap().end == 5000, "range not changed");
    }

    change_rcv {
        let (owner, ca_id) = attach::<T>(0, 0);
    }: _(owner.origin(), ca_id, false)
    verify {
        ensure!(!RCV::get(ca_id), "RCV not changed");
    }
}