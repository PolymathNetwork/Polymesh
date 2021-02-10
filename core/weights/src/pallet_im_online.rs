//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_im_online::WeightInfo for WeightInfo {
    fn heartbeat(k: u32, e: u32) -> Weight {
        (88_105_000 as Weight)
            .saturating_add((189_000 as Weight).saturating_mul(k as Weight))
            .saturating_add((1_038_000 as Weight).saturating_mul(e as Weight))
            .saturating_add(DbWeight::get().reads(3 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_slashing_params() -> Weight {
        (32_278_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
    }
}
