//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl frame_system::WeightInfo for WeightInfo {
    // WARNING! Some components were not used: ["b"]
    fn remark() -> Weight {
        (2_828_000 as Weight)
    }
    fn set_heap_pages() -> Weight {
        (4_253_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_changes_trie_config() -> Weight {
        (17_616_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn set_storage(i: u32) -> Weight {
        (0 as Weight)
            .saturating_add((1_289_000 as Weight).saturating_mul(i as Weight))
            .saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(i as Weight)))
    }
    fn kill_storage(i: u32) -> Weight {
        (3_425_000 as Weight)
            .saturating_add((883_000 as Weight).saturating_mul(i as Weight))
            .saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(i as Weight)))
    }
    fn kill_prefix(p: u32) -> Weight {
        (4_859_000 as Weight)
            .saturating_add((1_309_000 as Weight).saturating_mul(p as Weight))
            .saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(p as Weight)))
    }
    fn suicide() -> Weight {
        (53_564_000 as Weight)
    }
}
