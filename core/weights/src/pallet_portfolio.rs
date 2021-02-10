//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_portfolio::WeightInfo for WeightInfo {
    // WARNING! Some components were not used: ["i"]
    fn create_portfolio(_i: u32) -> Weight {
        (135_558_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn delete_portfolio() -> Weight {
        (125_649_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn move_portfolio_funds(a: u32) -> Weight {
        (0 as Weight)
            .saturating_add((66_353_000 as Weight).saturating_mul(a as Weight))
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().reads((3 as Weight).saturating_mul(a as Weight)))
            .saturating_add(DbWeight::get().writes((2 as Weight).saturating_mul(a as Weight)))
    }
    fn rename_portfolio(i: u32) -> Weight {
        (149_813_000 as Weight)
            .saturating_add((33_000 as Weight).saturating_mul(i as Weight))
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
}
