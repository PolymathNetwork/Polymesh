//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_group::WeightInfo for WeightInfo {
    fn set_active_members_limit() -> Weight {
        (41_819_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn add_member() -> Weight {
        (1_568_976_000 as Weight)
            .saturating_add(DbWeight::get().reads(5 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn remove_member() -> Weight {
        (533_296_000 as Weight)
            .saturating_add(DbWeight::get().reads(3 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn disable_member() -> Weight {
        (503_416_000 as Weight)
            .saturating_add(DbWeight::get().reads(5 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn swap_member() -> Weight {
        (1_490_737_000 as Weight)
            .saturating_add(DbWeight::get().reads(5 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn reset_members(m: u32) -> Weight {
        (0 as Weight)
            .saturating_add((1_116_540_000 as Weight).saturating_mul(m as Weight))
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().reads((1 as Weight).saturating_mul(m as Weight)))
            .saturating_add(DbWeight::get().writes(5 as Weight))
            .saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(m as Weight)))
    }
    fn abdicate_membership() -> Weight {
        (474_919_000 as Weight)
            .saturating_add(DbWeight::get().reads(6 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
}
