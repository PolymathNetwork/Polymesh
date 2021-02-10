//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_asset::checkpoint::WeightInfo for WeightInfo {
    fn set_schedules_max_complexity() -> Weight {
        (10_655_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn create_checkpoint() -> Weight {
        (45_191_000 as Weight)
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn create_schedule(s: u32) -> Weight {
        (71_700_000 as Weight)
            .saturating_add((73_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(DbWeight::get().reads(15 as Weight))
            .saturating_add(DbWeight::get().writes(7 as Weight))
    }
    fn remove_schedule(s: u32) -> Weight {
        (44_085_000 as Weight)
            .saturating_add((84_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
}
