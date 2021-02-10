//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_corporate_actions::distribution::WeightInfo for WeightInfo {
    fn distribute() -> Weight {
        (228_327_000 as Weight)
            .saturating_add(DbWeight::get().reads(17 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn claim(t: u32, w: u32) -> Weight {
        (730_892_000 as Weight)
            .saturating_add((427_000 as Weight).saturating_mul(t as Weight))
            .saturating_add((214_000 as Weight).saturating_mul(w as Weight))
            .saturating_add(DbWeight::get().reads(31 as Weight))
            .saturating_add(DbWeight::get().writes(11 as Weight))
    }
    fn push_benefit(t: u32, w: u32) -> Weight {
        (721_582_000 as Weight)
            .saturating_add((473_000 as Weight).saturating_mul(t as Weight))
            .saturating_add((217_000 as Weight).saturating_mul(w as Weight))
            .saturating_add(DbWeight::get().reads(32 as Weight))
            .saturating_add(DbWeight::get().writes(11 as Weight))
    }
    fn reclaim() -> Weight {
        (155_556_000 as Weight)
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn remove_distribution() -> Weight {
        (163_289_000 as Weight)
            .saturating_add(DbWeight::get().reads(11 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
}
