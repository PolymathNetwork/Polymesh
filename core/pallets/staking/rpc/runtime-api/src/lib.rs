//! Runtime API definition for staking module.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::dispatch::Vec;
use sp_runtime::Perbill;

sp_api::decl_runtime_apis! {
    pub trait StakingApi {
        fn get_curve() -> Vec<(Perbill, Perbill)>;
    }
}
