//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_committee::WeightInfo for WeightInfo {
    fn set_vote_threshold() -> Weight {
        (30_228_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_release_coordinator() -> Weight {
        (202_048_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_expires_after() -> Weight {
        (30_934_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn vote_or_propose_new_proposal() -> Weight {
        (612_282_000 as Weight)
            .saturating_add(DbWeight::get().reads(11 as Weight))
            .saturating_add(DbWeight::get().writes(4 as Weight))
    }
    fn vote_or_propose_existing_proposal() -> Weight {
        (576_980_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn vote_aye() -> Weight {
        (1_368_903_000 as Weight)
            .saturating_add(DbWeight::get().reads(10 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn vote_nay() -> Weight {
        (571_001_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
}
