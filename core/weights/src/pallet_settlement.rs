//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_settlement::WeightInfo for WeightInfo {
    fn create_venue(d: u32, s: u32) -> Weight {
        (118_442_000 as Weight)
            .saturating_add((10_000 as Weight).saturating_mul(d as Weight))
            .saturating_add((7_107_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
            .saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(s as Weight)))
    }
    fn update_venue(d: u32) -> Weight {
        (105_721_000 as Weight)
            .saturating_add((11_000 as Weight).saturating_mul(d as Weight))
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn add_instruction(l: u32) -> Weight {
        (134_326_000 as Weight)
            .saturating_add((32_767_000 as Weight).saturating_mul(l as Weight))
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().reads((1 as Weight).saturating_mul(l as Weight)))
            .saturating_add(DbWeight::get().writes(1 as Weight))
            .saturating_add(DbWeight::get().writes((3 as Weight).saturating_mul(l as Weight)))
    }
    fn add_instruction_with_settle_on_block_type(l: u32) -> Weight {
        (205_421_000 as Weight)
            .saturating_add((31_716_000 as Weight).saturating_mul(l as Weight))
            .saturating_add(DbWeight::get().reads(10 as Weight))
            .saturating_add(DbWeight::get().reads((1 as Weight).saturating_mul(l as Weight)))
            .saturating_add(DbWeight::get().writes(3 as Weight))
            .saturating_add(DbWeight::get().writes((3 as Weight).saturating_mul(l as Weight)))
    }
    fn add_and_affirm_instruction(l: u32) -> Weight {
        (239_157_000 as Weight)
            .saturating_add((135_911_000 as Weight).saturating_mul(l as Weight))
            .saturating_add(DbWeight::get().reads(10 as Weight))
            .saturating_add(DbWeight::get().reads((5 as Weight).saturating_mul(l as Weight)))
            .saturating_add(DbWeight::get().writes(4 as Weight))
            .saturating_add(DbWeight::get().writes((6 as Weight).saturating_mul(l as Weight)))
    }
    fn add_and_affirm_instruction_with_settle_on_block_type(l: u32) -> Weight {
        (326_808_000 as Weight)
            .saturating_add((138_816_000 as Weight).saturating_mul(l as Weight))
            .saturating_add(DbWeight::get().reads(12 as Weight))
            .saturating_add(DbWeight::get().reads((5 as Weight).saturating_mul(l as Weight)))
            .saturating_add(DbWeight::get().writes(6 as Weight))
            .saturating_add(DbWeight::get().writes((6 as Weight).saturating_mul(l as Weight)))
    }
    fn set_venue_filtering() -> Weight {
        (121_493_000 as Weight)
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_venue_filtering_disallow() -> Weight {
        (130_614_000 as Weight)
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn allow_venues(v: u32) -> Weight {
        (119_918_000 as Weight)
            .saturating_add((6_411_000 as Weight).saturating_mul(v as Weight))
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(v as Weight)))
    }
    fn disallow_venues(v: u32) -> Weight {
        (112_796_000 as Weight)
            .saturating_add((6_329_000 as Weight).saturating_mul(v as Weight))
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(v as Weight)))
    }
    fn withdraw_affirmation(l: u32) -> Weight {
        (119_739_000 as Weight)
            .saturating_add((117_028_000 as Weight).saturating_mul(l as Weight))
            .saturating_add(DbWeight::get().reads(10 as Weight))
            .saturating_add(DbWeight::get().reads((5 as Weight).saturating_mul(l as Weight)))
            .saturating_add(DbWeight::get().writes(1 as Weight))
            .saturating_add(DbWeight::get().writes((4 as Weight).saturating_mul(l as Weight)))
    }
    fn withdraw_affirmation_with_receipt(l: u32) -> Weight {
        (151_539_000 as Weight)
            .saturating_add((113_121_000 as Weight).saturating_mul(l as Weight))
            .saturating_add(DbWeight::get().reads(10 as Weight))
            .saturating_add(DbWeight::get().reads((4 as Weight).saturating_mul(l as Weight)))
            .saturating_add(DbWeight::get().writes(1 as Weight))
            .saturating_add(DbWeight::get().writes((4 as Weight).saturating_mul(l as Weight)))
    }
    fn unclaim_receipt() -> Weight {
        (193_393_000 as Weight)
            .saturating_add(DbWeight::get().reads(12 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn reject_instruction(l: u32) -> Weight {
        (219_654_000 as Weight)
            .saturating_add((130_999_000 as Weight).saturating_mul(l as Weight))
            .saturating_add(DbWeight::get().reads(11 as Weight))
            .saturating_add(DbWeight::get().reads((5 as Weight).saturating_mul(l as Weight)))
            .saturating_add(DbWeight::get().writes(3 as Weight))
            .saturating_add(DbWeight::get().writes((4 as Weight).saturating_mul(l as Weight)))
    }
    fn reject_instruction_with_no_pre_affirmations(l: u32) -> Weight {
        (237_721_000 as Weight)
            .saturating_add((46_777_000 as Weight).saturating_mul(l as Weight))
            .saturating_add(DbWeight::get().reads(11 as Weight))
            .saturating_add(DbWeight::get().reads((3 as Weight).saturating_mul(l as Weight)))
            .saturating_add(DbWeight::get().writes(3 as Weight))
            .saturating_add(DbWeight::get().writes((2 as Weight).saturating_mul(l as Weight)))
    }
    fn affirm_instruction(l: u32) -> Weight {
        (184_867_000 as Weight)
            .saturating_add((71_840_000 as Weight).saturating_mul(l as Weight))
            .saturating_add(DbWeight::get().reads(11 as Weight))
            .saturating_add(DbWeight::get().reads((3 as Weight).saturating_mul(l as Weight)))
            .saturating_add(DbWeight::get().writes(2 as Weight))
            .saturating_add(DbWeight::get().writes((2 as Weight).saturating_mul(l as Weight)))
    }
    fn claim_receipt() -> Weight {
        (290_864_000 as Weight)
            .saturating_add(DbWeight::get().reads(13 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn affirm_with_receipts(r: u32) -> Weight {
        (173_739_000 as Weight)
            .saturating_add((197_611_000 as Weight).saturating_mul(r as Weight))
            .saturating_add(DbWeight::get().reads(10 as Weight))
            .saturating_add(DbWeight::get().reads((4 as Weight).saturating_mul(r as Weight)))
            .saturating_add(DbWeight::get().writes(1 as Weight))
            .saturating_add(DbWeight::get().writes((4 as Weight).saturating_mul(r as Weight)))
    }
    fn execute_scheduled_instruction(l: u32) -> Weight {
        (0 as Weight)
            .saturating_add((2_698_678_000 as Weight).saturating_mul(l as Weight))
            .saturating_add(DbWeight::get().reads(4 as Weight))
            .saturating_add(DbWeight::get().reads((76 as Weight).saturating_mul(l as Weight)))
            .saturating_add(DbWeight::get().writes(3 as Weight))
            .saturating_add(DbWeight::get().writes((14 as Weight).saturating_mul(l as Weight)))
    }
    fn change_receipt_validity() -> Weight {
        (200_000_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
    }
}
