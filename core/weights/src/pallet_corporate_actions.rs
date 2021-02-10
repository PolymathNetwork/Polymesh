//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_corporate_actions::WeightInfo for WeightInfo {
    fn set_max_details_length() -> Weight {
        (27_328_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn reset_caa() -> Weight {
        (153_578_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_default_targets(t: u32) -> Weight {
        (195_590_000 as Weight)
            .saturating_add((695_000 as Weight).saturating_mul(t as Weight))
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_default_withholding_tax() -> Weight {
        (135_218_000 as Weight)
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_did_withholding_tax(w: u32) -> Weight {
        (186_392_000 as Weight)
            .saturating_add((396_000 as Weight).saturating_mul(w as Weight))
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn initiate_corporate_action_use_defaults(w: u32, t: u32) -> Weight {
        (424_252_000 as Weight)
            .saturating_add((550_000 as Weight).saturating_mul(w as Weight))
            .saturating_add((370_000 as Weight).saturating_mul(t as Weight))
            .saturating_add(DbWeight::get().reads(19 as Weight))
            .saturating_add(DbWeight::get().writes(5 as Weight))
    }
    fn initiate_corporate_action_provided(w: u32, t: u32) -> Weight {
        (388_825_000 as Weight)
            .saturating_add((547_000 as Weight).saturating_mul(w as Weight))
            .saturating_add((541_000 as Weight).saturating_mul(t as Weight))
            .saturating_add(DbWeight::get().reads(16 as Weight))
            .saturating_add(DbWeight::get().writes(5 as Weight))
    }
    fn link_ca_doc(d: u32) -> Weight {
        (0 as Weight)
            .saturating_add((13_381_000 as Weight).saturating_mul(d as Weight))
            .saturating_add(DbWeight::get().reads(10 as Weight))
            .saturating_add(DbWeight::get().reads((1 as Weight).saturating_mul(d as Weight)))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn remove_ca_with_ballot() -> Weight {
        (225_479_000 as Weight)
            .saturating_add(DbWeight::get().reads(11 as Weight))
            .saturating_add(DbWeight::get().writes(6 as Weight))
    }
    fn remove_ca_with_dist() -> Weight {
        (211_714_000 as Weight)
            .saturating_add(DbWeight::get().reads(12 as Weight))
            .saturating_add(DbWeight::get().writes(4 as Weight))
    }
    fn change_record_date_with_ballot() -> Weight {
        (241_643_000 as Weight)
            .saturating_add(DbWeight::get().reads(16 as Weight))
            .saturating_add(DbWeight::get().writes(4 as Weight))
    }
    fn change_record_date_with_dist() -> Weight {
        (250_851_000 as Weight)
            .saturating_add(DbWeight::get().reads(16 as Weight))
            .saturating_add(DbWeight::get().writes(4 as Weight))
    }
}
