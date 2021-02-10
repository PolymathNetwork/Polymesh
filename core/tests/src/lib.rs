#![allow(dead_code)]
#![feature(crate_visibility_modifier)]

pub mod storage;
pub use storage::{
    account_from, add_secondary_key, get_identity_id, make_account, make_account_with_balance,
    make_account_without_cdd, register_keyring_account_with_balance,
    register_keyring_account_without_cdd, TestStorage,
};

pub mod ext_builder;
pub use ext_builder::ExtBuilder;

#[cfg(test)]
mod asset_test;
#[cfg(test)]
mod balances_test;
#[cfg(test)]
mod bridge;
#[cfg(test)]
mod committee_test;
#[cfg(test)]
mod compliance_manager_test;
#[cfg(test)]
mod confidential_test;
#[cfg(test)]
mod contract_test;
#[cfg(test)]
mod corporate_actions_test;
#[cfg(test)]
mod fee_details;
#[cfg(test)]
mod group_test;
#[cfg(test)]
mod identity_test;
#[cfg(test)]
mod multisig;
#[cfg(test)]
mod pips_test;
#[cfg(test)]
mod portfolio;
#[cfg(test)]
mod protocol_fee;
#[cfg(test)]
mod settlement_test;
#[cfg(test)]
mod signed_extra;
#[cfg(test)]
mod staking;
#[cfg(test)]
mod statistics_test;
#[cfg(test)]
mod sto_test;
#[cfg(test)]
mod transaction_payment_test;
#[cfg(test)]
mod treasury_test;
#[cfg(test)]
mod utility_test;
