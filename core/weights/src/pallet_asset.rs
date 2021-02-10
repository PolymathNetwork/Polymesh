//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_asset::WeightInfo for WeightInfo {
    fn register_ticker() -> Weight {
        (160_588_000 as Weight)
            .saturating_add(DbWeight::get().reads(13 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn accept_ticker_transfer() -> Weight {
        (203_172_000 as Weight)
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(6 as Weight))
    }
    fn accept_asset_ownership_transfer() -> Weight {
        (217_044_000 as Weight)
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(6 as Weight))
    }
    fn create_asset(n: u32, i: u32, f: u32) -> Weight {
        (458_026_000 as Weight)
            .saturating_add((20_000 as Weight).saturating_mul(n as Weight))
            .saturating_add((150_000 as Weight).saturating_mul(i as Weight))
            .saturating_add((33_000 as Weight).saturating_mul(f as Weight))
            .saturating_add(DbWeight::get().reads(22 as Weight))
            .saturating_add(DbWeight::get().writes(11 as Weight))
    }
    fn freeze() -> Weight {
        (136_638_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn unfreeze() -> Weight {
        (136_133_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn rename_asset(n: u32) -> Weight {
        (139_234_000 as Weight)
            .saturating_add((4_000 as Weight).saturating_mul(n as Weight))
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn issue() -> Weight {
        (278_225_000 as Weight)
            .saturating_add(DbWeight::get().reads(18 as Weight))
            .saturating_add(DbWeight::get().writes(5 as Weight))
    }
    fn redeem() -> Weight {
        (289_971_000 as Weight)
            .saturating_add(DbWeight::get().reads(16 as Weight))
            .saturating_add(DbWeight::get().writes(6 as Weight))
    }
    fn make_divisible() -> Weight {
        (132_758_000 as Weight)
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn add_documents(d: u32) -> Weight {
        (117_746_000 as Weight)
            .saturating_add((95_770_000 as Weight).saturating_mul(d as Weight))
            .saturating_add(DbWeight::get().reads(10 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
            .saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(d as Weight)))
    }
    fn remove_documents(d: u32) -> Weight {
        (77_447_000 as Weight)
            .saturating_add((21_861_000 as Weight).saturating_mul(d as Weight))
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(d as Weight)))
    }
    fn set_funding_round(f: u32) -> Weight {
        (122_651_000 as Weight)
            .saturating_add((9_000 as Weight).saturating_mul(f as Weight))
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn update_identifiers(i: u32) -> Weight {
        (124_618_000 as Weight)
            .saturating_add((135_000 as Weight).saturating_mul(i as Weight))
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn add_extension() -> Weight {
        (157_444_000 as Weight)
            .saturating_add(DbWeight::get().reads(11 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn archive_extension() -> Weight {
        (134_570_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn unarchive_extension() -> Weight {
        (133_931_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn remove_smart_extension() -> Weight {
        (143_172_000 as Weight)
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn remove_primary_issuance_agent() -> Weight {
        (126_381_000 as Weight)
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn claim_classic_ticker() -> Weight {
        (424_504_000 as Weight)
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn reserve_classic_ticker() -> Weight {
        (91_914_000 as Weight)
            .saturating_add(DbWeight::get().reads(3 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn accept_primary_issuance_agent_transfer() -> Weight {
        (198_593_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn controller_transfer() -> Weight {
        (309_999_000 as Weight)
            .saturating_add(DbWeight::get().reads(18 as Weight))
            .saturating_add(DbWeight::get().writes(8 as Weight))
    }
}
