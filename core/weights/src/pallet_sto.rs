//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_sto::WeightInfo for WeightInfo {
    // WARNING! Some components were not used: ["i"]
    fn create_fundraiser(_i: u32) -> Weight {
        (233_299_000 as Weight)
            .saturating_add(DbWeight::get().reads(13 as Weight))
            .saturating_add(DbWeight::get().writes(4 as Weight))
    }
    fn invest() -> Weight {
        (5_744_445_000 as Weight)
            .saturating_add(DbWeight::get().reads(159 as Weight))
            .saturating_add(DbWeight::get().writes(30 as Weight))
    }
    fn freeze_fundraiser() -> Weight {
        (155_155_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn unfreeze_fundraiser() -> Weight {
        (166_632_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn modify_fundraiser_window() -> Weight {
        (135_027_000 as Weight)
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn stop() -> Weight {
        (160_263_000 as Weight)
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
}
