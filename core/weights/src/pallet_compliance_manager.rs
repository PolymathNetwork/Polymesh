//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_compliance_manager::WeightInfo for WeightInfo {
    fn add_compliance_requirement(s: u32, r: u32) -> Weight {
        (179_972_000 as Weight)
            .saturating_add((4_937_000 as Weight).saturating_mul(s as Weight))
            .saturating_add((4_321_000 as Weight).saturating_mul(r as Weight))
            .saturating_add(DbWeight::get().reads(11 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn remove_compliance_requirement() -> Weight {
        (193_357_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn pause_asset_compliance() -> Weight {
        (181_875_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn resume_asset_compliance() -> Weight {
        (142_372_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn add_default_trusted_claim_issuer() -> Weight {
        (145_855_000 as Weight)
            .saturating_add(DbWeight::get().reads(10 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn remove_default_trusted_claim_issuer() -> Weight {
        (137_782_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    // WARNING! Some components were not used: ["s"]
    fn change_compliance_requirement(r: u32, _s: u32) -> Weight {
        (199_636_000 as Weight)
            .saturating_add((10_080_000 as Weight).saturating_mul(r as Weight))
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn replace_asset_compliance(c: u32) -> Weight {
        (192_722_000 as Weight)
            .saturating_add((42_981_000 as Weight).saturating_mul(c as Weight))
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn reset_asset_compliance() -> Weight {
        (143_884_000 as Weight)
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
}
