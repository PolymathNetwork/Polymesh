//! This file is based on auto-generated one using the substrate benchmark CLI version 2.0.0.
//! It has the following changes:
//!   - `fn *_transfer` functions has been removed because they are using to verify the benchmark.
//!   - Function parameter `c: u32` has been replaced by the relayed call. In this way, we
//!   can fetch the actual call's weight instead of make an estimation based on the number of
//!   relayed calls.

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{GetDispatchInfo, RocksDbWeight as DbWeight, Weight};

fn sum_weights(calls: &[impl GetDispatchInfo]) -> Weight {
    calls
        .iter()
        .map(|call| call.get_dispatch_info().weight)
        .fold(0 as Weight, |a: Weight, n| a.saturating_add(n))
}

pub struct WeightInfo;
impl pallet_utility::WeightInfo for WeightInfo {
    fn batch(calls: &[impl GetDispatchInfo]) -> Weight {
        (32_338_000 as Weight)
            .saturating_add(sum_weights(calls))
            .saturating_add(DbWeight::get().reads(2 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }

    fn batch_atomic(calls: &[impl GetDispatchInfo]) -> Weight {
        (53_986_000 as Weight)
            .saturating_add(sum_weights(calls))
            .saturating_add(DbWeight::get().reads(2 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }

    fn batch_optimistic(calls: &[impl GetDispatchInfo]) -> Weight {
        (44_239_000 as Weight)
            .saturating_add(sum_weights(calls))
            .saturating_add(DbWeight::get().reads(2 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }

    fn relay_tx(call: &impl GetDispatchInfo) -> Weight {
        (245_921_000 as Weight)
            .saturating_add(call.get_dispatch_info().weight)
            .saturating_add(DbWeight::get().reads(14 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
}
