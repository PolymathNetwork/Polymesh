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

use super::*;
use crate::benchmarking::{set_ca_targets, setup_ca};
use core::iter;
use frame_benchmarking::benchmarks;
use polymesh_common_utilities::benchs::User;

const MAX_CHOICES: u32 = 1000;
const MAX_TARGETS: u32 = 1000;

const RANGE: BallotTimeRange = BallotTimeRange {
    start: 3000,
    end: 4000,
};

fn meta(n_motions: u32, n_choices: u32) -> BallotMeta {
    let motion = Motion {
        title: "".into(),
        info_link: "".into(),
        choices: iter::repeat("".into()).take(n_choices as usize).collect(),
    };
    let motions = iter::repeat(motion).take(n_motions as usize).collect();
    BallotMeta {
        title: "".into(),
        motions,
    }
}

fn attach<T: Trait>(n_motions: u32, n_choices: u32) -> (User<T>, CAId) {
    let meta = meta(n_motions, n_choices);
    let (owner, ca_id) = setup_ca::<T>(CAKind::IssuerNotice);
    <Module<T>>::attach_ballot(owner.origin().into(), ca_id, RANGE, meta, true).unwrap();
    (owner, ca_id)
}

benchmarks! {
    _ {}

    attach_ballot {
        let c in 0..MAX_CHOICES;

        let meta = meta(1, c);
        let (owner, ca_id) = setup_ca::<T>(CAKind::IssuerNotice);
    }: _(owner.origin(), ca_id, RANGE, meta, true)
    verify {
        ensure!(TimeRanges::get(ca_id) == Some(RANGE), "ballot not created");
    }

    vote {
        let c in 0..MAX_CHOICES;
        let t in 0..MAX_TARGETS;

        // Attach and prepare to vote.
        let (owner, ca_id) = attach::<T>(1, c);
        <pallet_timestamp::Now<T>>::set(3000u32.into());

        // Change targets, as they are read in voting.
        set_ca_targets::<T>(ca_id, t);

        // Construct the voting list.
        let votes = (0..c)
            .map(|c| BallotVote {
                power: 0u32.into(),
                fallback: (c as u16).checked_sub(1),
            })
            .collect::<Vec<_>>();

        // Vote already to force a longer code path.
        <Module<T>>::vote(owner.origin().into(), ca_id, votes.clone()).unwrap();
        let results = votes.iter().map(|v| v.power).collect::<Vec<_>>();
    }: _(owner.origin(), ca_id, votes)
    verify {
        ensure!(<Results<T>>::get(ca_id) == results, "voting results are wrong")
    }

    change_end {
        let (owner, ca_id) = attach::<T>(0, 0);
    }: _(owner.origin(), ca_id, 5000)
    verify {
        ensure!(TimeRanges::get(ca_id).unwrap().end == 5000, "range not changed");
    }

    change_meta {
        let c in 0..MAX_CHOICES;

        let (owner, ca_id) = attach::<T>(0, 0);
        let meta = meta(1, c);
        let meta2 = meta.clone();
    }: _(owner.origin(), ca_id, meta)
    verify {
        ensure!(Metas::get(ca_id).unwrap() == meta2, "meta not changed");
    }

    change_rcv {
        let (owner, ca_id) = attach::<T>(0, 0);
    }: _(owner.origin(), ca_id, false)
    verify {
        ensure!(!RCV::get(ca_id), "RCV not changed");
    }

    remove_ballot {
        let (owner, ca_id) = attach::<T>(0, 0);
    }: _(owner.origin(), ca_id)
    verify {
        ensure!(TimeRanges::get(ca_id) == None, "ballot not removed");
    }
}
