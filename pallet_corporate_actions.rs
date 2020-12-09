//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{Weight, constants::RocksDbWeight as DbWeight};

pub struct WeightInfo;
impl pallet_corporate_actions::WeightInfo for WeightInfo {
	fn set_max_details_length() -> Weight {
		(10_099_000 as Weight)
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn reset_caa() -> Weight {
		(36_154_000 as Weight)
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn set_default_targets(i: u32, ) -> Weight {
		(46_657_000 as Weight)
			.saturating_add((227_000 as Weight).saturating_mul(i as Weight))
			.saturating_add(DbWeight::get().reads(7 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn set_default_withholding_tax() -> Weight {
		(35_018_000 as Weight)
			.saturating_add(DbWeight::get().reads(7 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn set_did_withholding_tax(i: u32, ) -> Weight {
		(51_258_000 as Weight)
			.saturating_add((217_000 as Weight).saturating_mul(i as Weight))
			.saturating_add(DbWeight::get().reads(7 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn initiate_corporate_action_use_defaults(i: u32, j: u32, k: u32, ) -> Weight {
		(76_497_000 as Weight)
			.saturating_add((61_000 as Weight).saturating_mul(i as Weight))
			.saturating_add((192_000 as Weight).saturating_mul(j as Weight))
			.saturating_add((200_000 as Weight).saturating_mul(k as Weight))
			.saturating_add(DbWeight::get().reads(17 as Weight))
			.saturating_add(DbWeight::get().writes(5 as Weight))
	}
	// WARNING! Some components were not used: ["i"]
	fn initiate_corporate_action_provided(j: u32, k: u32, ) -> Weight {
		(78_654_000 as Weight)
			.saturating_add((105_000 as Weight).saturating_mul(j as Weight))
			.saturating_add((143_000 as Weight).saturating_mul(k as Weight))
			.saturating_add(DbWeight::get().reads(14 as Weight))
			.saturating_add(DbWeight::get().writes(5 as Weight))
	}
	fn link_ca_doc(i: u32, ) -> Weight {
		(23_092_000 as Weight)
			.saturating_add((4_670_000 as Weight).saturating_mul(i as Weight))
			.saturating_add(DbWeight::get().reads(8 as Weight))
			.saturating_add(DbWeight::get().reads((1 as Weight).saturating_mul(i as Weight)))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn remove_ca_with_ballot() -> Weight {
		(55_368_000 as Weight)
			.saturating_add(DbWeight::get().reads(9 as Weight))
			.saturating_add(DbWeight::get().writes(7 as Weight))
	}
	fn remove_ca_with_dist() -> Weight {
		(58_944_000 as Weight)
			.saturating_add(DbWeight::get().reads(10 as Weight))
			.saturating_add(DbWeight::get().writes(4 as Weight))
	}
	fn change_record_date_with_ballot() -> Weight {
		(65_077_000 as Weight)
			.saturating_add(DbWeight::get().reads(14 as Weight))
			.saturating_add(DbWeight::get().writes(4 as Weight))
	}
	fn change_record_date_with_dist() -> Weight {
		(134_500_000 as Weight)
			.saturating_add(DbWeight::get().reads(14 as Weight))
			.saturating_add(DbWeight::get().writes(4 as Weight))
	}
}
