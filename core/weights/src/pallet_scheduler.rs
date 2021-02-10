//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_scheduler::WeightInfo for WeightInfo {
    fn schedule(s: u32) -> Weight {
        (48_941_000 as Weight)
            .saturating_add((175_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn cancel(s: u32) -> Weight {
        (44_143_000 as Weight)
            .saturating_add((6_825_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn schedule_named(s: u32) -> Weight {
        (62_871_000 as Weight)
            .saturating_add((198_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(DbWeight::get().reads(2 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn cancel_named(s: u32) -> Weight {
        (52_205_000 as Weight)
            .saturating_add((6_616_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(DbWeight::get().reads(2 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
}
