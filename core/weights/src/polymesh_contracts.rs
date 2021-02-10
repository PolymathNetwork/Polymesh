//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl polymesh_contracts::WeightInfo for WeightInfo {
    fn put_code(l: u32, u: u32, d: u32) -> Weight {
        (0 as Weight)
            .saturating_add((254_000 as Weight).saturating_mul(l as Weight))
            .saturating_add((16_000 as Weight).saturating_mul(u as Weight))
            .saturating_add((18_000 as Weight).saturating_mul(d as Weight))
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(4 as Weight))
    }
    fn instantiate() -> Weight {
        (1_661_897_000 as Weight)
            .saturating_add(DbWeight::get().reads(19 as Weight))
            .saturating_add(DbWeight::get().writes(6 as Weight))
    }
    fn freeze_instantiation() -> Weight {
        (120_473_000 as Weight)
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn unfreeze_instantiation() -> Weight {
        (129_928_000 as Weight)
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn transfer_template_ownership() -> Weight {
        (199_388_000 as Weight)
            .saturating_add(DbWeight::get().reads(12 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn change_template_fees() -> Weight {
        (2_470_203_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn change_template_meta_url(u: u32) -> Weight {
        (2_278_567_000 as Weight)
            .saturating_add((10_000 as Weight).saturating_mul(u as Weight))
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn update_schedule() -> Weight {
        (36_380_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
}
