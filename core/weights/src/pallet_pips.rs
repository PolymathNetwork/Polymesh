//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_pips::WeightInfo for WeightInfo {
    fn set_prune_historical_pips() -> Weight {
        (50_355_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_min_proposal_deposit() -> Weight {
        (51_155_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_default_enactment_period() -> Weight {
        (49_892_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_pending_pip_expiry() -> Weight {
        (58_089_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_max_pip_skip_count() -> Weight {
        (49_843_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_active_pip_limit() -> Weight {
        (50_705_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn propose_from_community() -> Weight {
        (381_663_000 as Weight)
            .saturating_add(DbWeight::get().reads(13 as Weight))
            .saturating_add(DbWeight::get().writes(10 as Weight))
    }
    fn propose_from_committee() -> Weight {
        (213_730_000 as Weight)
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(5 as Weight))
    }
    fn vote() -> Weight {
        (513_931_000 as Weight)
            .saturating_add(DbWeight::get().reads(6 as Weight))
            .saturating_add(DbWeight::get().writes(4 as Weight))
    }
    fn approve_committee_proposal() -> Weight {
        (247_313_000 as Weight)
            .saturating_add(DbWeight::get().reads(4 as Weight))
            .saturating_add(DbWeight::get().writes(4 as Weight))
    }
    fn reject_proposal() -> Weight {
        (344_515_000 as Weight)
            .saturating_add(DbWeight::get().reads(9 as Weight))
            .saturating_add(DbWeight::get().writes(10 as Weight))
    }
    fn prune_proposal() -> Weight {
        (185_066_000 as Weight)
            .saturating_add(DbWeight::get().reads(2 as Weight))
            .saturating_add(DbWeight::get().writes(5 as Weight))
    }
    fn reschedule_execution() -> Weight {
        (326_310_000 as Weight)
            .saturating_add(DbWeight::get().reads(11 as Weight))
            .saturating_add(DbWeight::get().writes(6 as Weight))
    }
    fn clear_snapshot() -> Weight {
        (111_949_000 as Weight)
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn snapshot() -> Weight {
        (1_537_755_000 as Weight)
            .saturating_add(DbWeight::get().reads(8 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn enact_snapshot_results(a: u32, r: u32, s: u32) -> Weight {
        (0 as Weight)
            .saturating_add((1_129_852_000 as Weight).saturating_mul(a as Weight))
            .saturating_add((32_682_393_000 as Weight).saturating_mul(r as Weight))
            .saturating_add(DbWeight::get().reads(688 as Weight))
            .saturating_add(DbWeight::get().reads((3 as Weight).saturating_mul(a as Weight)))
            .saturating_add(DbWeight::get().reads((405 as Weight).saturating_mul(r as Weight)))
            .saturating_add(DbWeight::get().reads((2 as Weight).saturating_mul(s as Weight)))
            .saturating_add(DbWeight::get().writes(684 as Weight))
            .saturating_add(DbWeight::get().writes((4 as Weight).saturating_mul(a as Weight)))
            .saturating_add(DbWeight::get().writes((404 as Weight).saturating_mul(r as Weight)))
            .saturating_add(DbWeight::get().writes((2 as Weight).saturating_mul(s as Weight)))
    }
    fn execute_scheduled_pip() -> Weight {
        (34_533_173_000 as Weight)
            .saturating_add(DbWeight::get().reads(1205 as Weight))
            .saturating_add(DbWeight::get().writes(1606 as Weight))
    }
    fn expire_scheduled_pip() -> Weight {
        (32_083_813_000 as Weight)
            .saturating_add(DbWeight::get().reads(1207 as Weight))
            .saturating_add(DbWeight::get().writes(1607 as Weight))
    }
}
