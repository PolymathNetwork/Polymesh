//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use polymesh_runtime_common::{RocksDbWeight as DbWeight, Weight};

pub struct WeightInfo;
impl pallet_staking::WeightInfo for WeightInfo {
    fn bond() -> Weight {
        (117_920_000 as Weight)
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(5 as Weight))
    }
    fn bond_extra() -> Weight {
        (103_834_000 as Weight)
            .saturating_add(DbWeight::get().reads(6 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn unbond() -> Weight {
        (102_511_000 as Weight)
            .saturating_add(DbWeight::get().reads(6 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn withdraw_unbonded_update(s: u32) -> Weight {
        (76_821_000 as Weight)
            .saturating_add((253_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(DbWeight::get().reads(5 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn withdraw_unbonded_kill(s: u32) -> Weight {
        (140_836_000 as Weight)
            .saturating_add((1_496_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(7 as Weight))
            .saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(s as Weight)))
    }
    fn set_min_bond_threshold() -> Weight {
        (24_065_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn add_permissioned_validator() -> Weight {
        (124_643_000 as Weight)
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn remove_permissioned_validator() -> Weight {
        (42_399_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_commission_cap(m: u32) -> Weight {
        (56_300_000 as Weight)
            .saturating_add((25_053_000 as Weight).saturating_mul(m as Weight))
            .saturating_add(DbWeight::get().reads(3 as Weight))
            .saturating_add(DbWeight::get().reads((1 as Weight).saturating_mul(m as Weight)))
            .saturating_add(DbWeight::get().writes(2 as Weight))
            .saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(m as Weight)))
    }
    fn validate() -> Weight {
        (80_160_000 as Weight)
            .saturating_add(DbWeight::get().reads(7 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn nominate(n: u32) -> Weight {
        (136_451_000 as Weight)
            .saturating_add((4_769_000 as Weight).saturating_mul(n as Weight))
            .saturating_add(DbWeight::get().reads(11 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn chill() -> Weight {
        (57_444_000 as Weight)
            .saturating_add(DbWeight::get().reads(5 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn set_payee() -> Weight {
        (29_842_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_controller() -> Weight {
        (70_035_000 as Weight)
            .saturating_add(DbWeight::get().reads(3 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn set_validator_count() -> Weight {
        (1_941_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn force_no_eras() -> Weight {
        (1_873_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn force_new_era() -> Weight {
        (1_854_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn force_new_era_always() -> Weight {
        (1_854_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn set_invulnerables(v: u32) -> Weight {
        (2_109_000 as Weight)
            .saturating_add((11_000 as Weight).saturating_mul(v as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn force_unstake(s: u32) -> Weight {
        (50_902_000 as Weight)
            .saturating_add((1_932_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(DbWeight::get().reads(4 as Weight))
            .saturating_add(DbWeight::get().writes(7 as Weight))
            .saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(s as Weight)))
    }
    fn cancel_deferred_slash(s: u32) -> Weight {
        (782_599_000 as Weight)
            .saturating_add((1_743_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn payout_stakers(n: u32) -> Weight {
        (267_975_000 as Weight)
            .saturating_add((123_000 as Weight).saturating_mul(n as Weight))
            .saturating_add(DbWeight::get().reads(13 as Weight))
            .saturating_add(DbWeight::get().writes(4 as Weight))
    }
    // WARNING! Some components were not used: ["n"]
    fn payout_stakers_alive_controller() -> Weight {
        (466_343_000 as Weight)
            .saturating_add(DbWeight::get().reads(13 as Weight))
            .saturating_add(DbWeight::get().writes(4 as Weight))
    }
    fn rebond(l: u32) -> Weight {
        (28_015_000 as Weight)
            .saturating_add((4_000 as Weight).saturating_mul(l as Weight))
            .saturating_add(DbWeight::get().reads(4 as Weight))
            .saturating_add(DbWeight::get().writes(3 as Weight))
    }
    fn set_history_depth(e: u32) -> Weight {
        (0 as Weight)
            .saturating_add((30_172_000 as Weight).saturating_mul(e as Weight))
            .saturating_add(DbWeight::get().reads(2 as Weight))
            .saturating_add(DbWeight::get().writes(4 as Weight))
            .saturating_add(DbWeight::get().writes((7 as Weight).saturating_mul(e as Weight)))
    }
    fn reap_stash(s: u32) -> Weight {
        (43_183_000 as Weight)
            .saturating_add((822_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(DbWeight::get().reads(4 as Weight))
            .saturating_add(DbWeight::get().writes(8 as Weight))
            .saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(s as Weight)))
    }
    fn new_era(v: u32, n: u32) -> Weight {
        (0 as Weight)
            .saturating_add((1_886_613_000 as Weight).saturating_mul(v as Weight))
            .saturating_add((212_212_000 as Weight).saturating_mul(n as Weight))
            .saturating_add(DbWeight::get().reads(19 as Weight))
            .saturating_add(DbWeight::get().reads((9 as Weight).saturating_mul(v as Weight)))
            .saturating_add(DbWeight::get().reads((7 as Weight).saturating_mul(n as Weight)))
            .saturating_add(DbWeight::get().writes(8 as Weight))
            .saturating_add(DbWeight::get().writes((3 as Weight).saturating_mul(v as Weight)))
    }
    fn do_slash(l: u32) -> Weight {
        (72_382_000 as Weight)
            .saturating_add((21_000 as Weight).saturating_mul(l as Weight))
            .saturating_add(DbWeight::get().reads(3 as Weight))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn payout_all(v: u32, n: u32) -> Weight {
        (0 as Weight)
            .saturating_add((3_222_276_000 as Weight).saturating_mul(v as Weight))
            .saturating_add((288_148_000 as Weight).saturating_mul(n as Weight))
            .saturating_add(DbWeight::get().reads((58 as Weight).saturating_mul(v as Weight)))
            .saturating_add(DbWeight::get().reads((6 as Weight).saturating_mul(n as Weight)))
            .saturating_add(DbWeight::get().writes((34 as Weight).saturating_mul(v as Weight)))
            .saturating_add(DbWeight::get().writes((3 as Weight).saturating_mul(n as Weight)))
    }
    // WARNING! Some components were not used: ["n"]
    fn submit_solution_initial(v: u32, a: u32, w: u32) -> Weight {
        (0 as Weight)
            .saturating_add((1_114_000 as Weight).saturating_mul(v as Weight))
            .saturating_add((64_658_000 as Weight).saturating_mul(a as Weight))
            .saturating_add((135_090_000 as Weight).saturating_mul(w as Weight))
            .saturating_add(DbWeight::get().reads(6 as Weight))
            .saturating_add(DbWeight::get().reads((4 as Weight).saturating_mul(a as Weight)))
            .saturating_add(DbWeight::get().reads((1 as Weight).saturating_mul(w as Weight)))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    fn submit_solution_better(v: u32, n: u32, a: u32, w: u32) -> Weight {
        (0 as Weight)
            .saturating_add((16_556_000 as Weight).saturating_mul(v as Weight))
            .saturating_add((16_923_000 as Weight).saturating_mul(n as Weight))
            .saturating_add((121_063_000 as Weight).saturating_mul(a as Weight))
            .saturating_add((69_411_000 as Weight).saturating_mul(w as Weight))
            .saturating_add(DbWeight::get().reads(6 as Weight))
            .saturating_add(DbWeight::get().reads((4 as Weight).saturating_mul(a as Weight)))
            .saturating_add(DbWeight::get().reads((1 as Weight).saturating_mul(w as Weight)))
            .saturating_add(DbWeight::get().writes(2 as Weight))
    }
    // WARNING! Some components were not used: ["v"]
    fn submit_solution_weaker(n: u32) -> Weight {
        (72_872_000 as Weight)
            .saturating_add((21_000 as Weight).saturating_mul(n as Weight))
            .saturating_add(DbWeight::get().reads(3 as Weight))
    }
    fn change_slashing_allowed_for() -> Weight {
        (85_940_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn update_permissioned_validator_intended_count() -> Weight {
        (44_183_000 as Weight)
            .saturating_add(DbWeight::get().reads(2 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn increase_validator_count() -> Weight {
        (12_443_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
    fn scale_validator_count() -> Weight {
        (22_453_000 as Weight)
            .saturating_add(DbWeight::get().reads(1 as Weight))
            .saturating_add(DbWeight::get().writes(1 as Weight))
    }
}
