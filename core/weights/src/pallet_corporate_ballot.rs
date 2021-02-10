//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_corporate_actions::ballot::WeightInfo for WeightInfo {
    fn attach_ballot(c: u32) -> Weight {
        (195_816_000 as Weight)
            .saturating_add((69_000 as Weight).saturating_mul(c as Weight))
            .saturating_add(DbWeight::get().reads(13 as Weight))
            .saturating_add(DbWeight::get().writes(4 as Weight))
    }
    fn vote(c: u32, t: u32) -> Weight {
        (233_490_000 as Weight)
            .saturating_add((425_000 as Weight).saturating_mul(c as Weight))
            .saturating_add((482_000 as Weight).saturating_mul(t as Weight))
            .saturating_add(DbWeight::get().reads(15 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn change_end() -> Weight {
        (138_579_000 as Weight)
            .saturating_add(DbWeight::get().reads(10 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn change_meta(c: u32) -> Weight {
        (164_691_000 as Weight)
            .saturating_add((53_000 as Weight).saturating_mul(c as Weight))
            .saturating_add(DbWeight::get().reads(10 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn change_rcv() -> Weight {
        (136_151_000 as Weight)
            .saturating_add(DbWeight::get().reads(10 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn remove_ballot() -> Weight {
        (152_330_000 as Weight)
            .saturating_add(DbWeight::get().reads(10 as Weight))
            .saturating_add(DbWeight::get().writes(4 as Weight))
    }
}
