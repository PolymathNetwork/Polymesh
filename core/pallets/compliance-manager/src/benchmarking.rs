// This file is part of the Polymesh distribution (https://github.com/PolymathNetwork/Polymesh).
// Copyright (c) 2020 Polymath

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

use crate::*;

use frame_benchmarking::benchmarks;
use pallet_asset::SecurityToken;
use polymesh_common_utilities::benchs::{User, UserBuilder};
use polymesh_primitives::{asset::AssetType, TrustedFor, TrustedIssuer};

const MAX_DEFAULT_TRUSTED_CLAIM_ISSUERS: u32 = 3;
const MAX_TRUSTED_ISSUER_PER_CONDITION: u32 = 3;
const MAX_SENDER_CONDITIONS_PER_COMPLIANCE: u32 = 5;
const MAX_RECEIVER_CONDITIONS_PER_COMPLIANCE: u32 = 5;
const MAX_COMPLIANCE_REQUIREMENTS: u32 = 2;

/// Create a token issuer trusted for `Any`.
pub fn make_issuer<T: IdentityTrait + BalancesTrait>(id: u32) -> TrustedIssuer {
    let u = UserBuilder::<T>::default()
        .generate_did()
        .seed(id)
        .build("ISSUER");
    TrustedIssuer {
        issuer: IdentityId::from(u.did.unwrap()),
        trusted_for: TrustedFor::Any,
    }
}

/// Helper function to create `s` token issuers with `fn make_issuer`.
/// # TODO
///   - It could have more complexity if `TrustedIssuer::trusted_for` is a vector but not on
///   benchmarking of add/remove. That could be useful for benchmarking executions/evaluation of
///   complience requiriments.
pub fn make_issuers<T: IdentityTrait + BalancesTrait>(s: u32) -> Vec<TrustedIssuer> {
    (0..s).map(|i| make_issuer::<T>(i)).collect::<Vec<_>>()
}

/// Create simple conditions with a variable number of `issuers`.
pub fn make_conditions(s: u32, issuers: &Vec<TrustedIssuer>) -> Vec<Condition> {
    (0..s)
        .map(|_| Condition {
            condition_type: ConditionType::IsPresent(Claim::NoData),
            issuers: issuers.clone(),
        })
        .collect()
}
/// Create a new token with name `name` on behalf of `owner`.
/// The new token is a _divisible_ one with 1_000_000 units.
pub fn make_token<T: Trait>(owner: &User<T>, name: Vec<u8>) -> Ticker {
    let token = SecurityToken {
        name: name.into(),
        owner_did: owner.did.clone().unwrap(),
        total_supply: 1_000_000,
        divisible: true,
        asset_type: AssetType::default(),
        ..Default::default()
    };
    let ticker = Ticker::try_from(token.name.0.as_slice()).unwrap();

    T::Asset::create_asset(
        owner.origin.clone().into(),
        token.name.clone(),
        ticker.clone(),
        u128::try_from(token.total_supply).unwrap().into(),
        true,
        token.asset_type.clone(),
        vec![],
        None,
    )
    .expect("Cannot create an asset");

    ticker
}

/// This struct helps to simplify the parameter copy/pass during the benchmarks.
struct ComplianceRequirementInfo<T: Trait> {
    pub owner: User<T>,
    pub buyer: User<T>,
    pub ticker: Ticker,
    pub issuers: Vec<TrustedIssuer>,
    pub sender_conditions: Vec<Condition>,
    pub receiver_conditions: Vec<Condition>,
}

impl<T: Trait> ComplianceRequirementInfo<T> {
    pub fn add_default_trusted_claim_issuer(self: &Self, i: u32) {
        make_issuers::<T>(i).into_iter().for_each(|issuer| {
            Module::<T>::add_default_trusted_claim_issuer(
                self.owner.origin.clone().into(),
                self.ticker.clone(),
                issuer,
            )
            .expect("Default trusted claim issuer cannot be added");
        });
    }
}

struct ComplianceRequirementBuilder<T: Trait> {
    info: ComplianceRequirementInfo<T>,
    has_been_added: bool,
}

impl<T: Trait> ComplianceRequirementBuilder<T> {
    pub fn new(
        trusted_issuer_count: u32,
        sender_conditions_count: u32,
        receiver_conditions_count: u32,
    ) -> Self {
        // Create accounts and token.
        let owner = UserBuilder::<T>::default().generate_did().build("OWNER");
        let buyer = UserBuilder::<T>::default().generate_did().build("BUYER");
        let ticker = make_token::<T>(&owner, b"1".to_vec());

        // Create issuers (i) and conditions(s & r).
        let issuers = make_issuers::<T>(trusted_issuer_count);
        let sender_conditions = make_conditions(sender_conditions_count, &issuers);
        let receiver_conditions = make_conditions(receiver_conditions_count, &issuers);

        let info = ComplianceRequirementInfo {
            owner,
            buyer,
            ticker,
            issuers,
            sender_conditions,
            receiver_conditions,
        };

        Self {
            info,
            has_been_added: false,
        }
    }

    /// Register the compliance requirement in the module.
    pub fn add_compliance_requirement(mut self: Self) -> Self {
        assert!(
            self.has_been_added == false,
            "Compliance has been added before"
        );
        Module::<T>::add_compliance_requirement(
            self.info.owner.origin.clone().into(),
            self.info.ticker.clone(),
            self.info.sender_conditions.clone(),
            self.info.receiver_conditions.clone(),
        )
        .expect("Compliance requirement cannot be added");
        self.has_been_added = true;
        self
    }

    pub fn build(self: Self) -> ComplianceRequirementInfo<T> {
        self.info
    }
}

benchmarks! {
    _ {}

    add_compliance_requirement {
        // INTERNAL: This benchmark only evaluate the adding operation. Its execution should be measured in another module.
        let s in 1..MAX_SENDER_CONDITIONS_PER_COMPLIANCE;
        let r in 1..MAX_RECEIVER_CONDITIONS_PER_COMPLIANCE;

        let d = ComplianceRequirementBuilder::<T>::new(MAX_TRUSTED_ISSUER_PER_CONDITION, s, r).build();

    }: _(d.owner.origin, d.ticker, d.sender_conditions.clone(), d.receiver_conditions.clone())
    verify {
        let req = Module::<T>::asset_compliance(d.ticker).requirements.pop().unwrap();
        ensure!( req.sender_conditions == d.sender_conditions, "Sender conditions not expected");
        ensure!( req.receiver_conditions == d.receiver_conditions, "Sender conditions not expected");
    }

    remove_compliance_requirement {
        // Add the compliance requirement.
        let d = ComplianceRequirementBuilder::<T>::new(
            MAX_TRUSTED_ISSUER_PER_CONDITION,
            MAX_SENDER_CONDITIONS_PER_COMPLIANCE,
            MAX_RECEIVER_CONDITIONS_PER_COMPLIANCE)
            .add_compliance_requirement().build();

        // Remove the latest one.
        let id = Module::<T>::get_latest_requirement_id(d.ticker);
    }: _(d.owner.origin, d.ticker, id)
    verify {
        let is_removed = Module::<T>::asset_compliance(d.ticker)
            .requirements
            .into_iter()
            .find(|r| r.id == id)
            .is_none();

        ensure!( is_removed, "Compliance requirement was not removed");
    }

    pause_asset_compliance {
        let d = ComplianceRequirementBuilder::<T>::new(
            MAX_TRUSTED_ISSUER_PER_CONDITION,
            MAX_SENDER_CONDITIONS_PER_COMPLIANCE,
            MAX_RECEIVER_CONDITIONS_PER_COMPLIANCE)
            .add_compliance_requirement().build();
    }: _(d.owner.origin, d.ticker)
    verify {
        ensure!( Module::<T>::asset_compliance(d.ticker).paused == true, "Asset compliance is not paused");
    }

    resume_asset_compliance {
        let d = ComplianceRequirementBuilder::<T>::new(2, 1, 1)
            .add_compliance_requirement().build();

        Module::<T>::pause_asset_compliance(
            d.owner.origin.clone().into(),
            d.ticker.clone()).unwrap();
    }: _(d.owner.origin, d.ticker)
    verify {
        ensure!( Module::<T>::asset_compliance(d.ticker).paused == false, "Asset compliance is paused");
    }

    add_default_trusted_claim_issuer {
        // Create and add the compliance requirement.
        let d = ComplianceRequirementBuilder::<T>::new(1, 1, 0)
            .add_compliance_requirement()
            .build();
        d.add_default_trusted_claim_issuer(MAX_DEFAULT_TRUSTED_CLAIM_ISSUERS -1);

        // Add one more for benchmarking.
        let new_issuer = make_issuer::<T>(MAX_DEFAULT_TRUSTED_CLAIM_ISSUERS);
    }: _(d.owner.origin, d.ticker, new_issuer.clone())
    verify {
        let trusted_issuers = Module::<T>::trusted_claim_issuer(d.ticker);
        ensure!(
            trusted_issuers.contains(&new_issuer),
            "Default trusted claim issuer was not added");
    }

    remove_default_trusted_claim_issuer {
        // Create and add the compliance requirement.
        let d = ComplianceRequirementBuilder::<T>::new(2, 1, 0)
            .add_compliance_requirement().build();

        // Generate some trusted issuer.
        d.add_default_trusted_claim_issuer(MAX_DEFAULT_TRUSTED_CLAIM_ISSUERS);

        // Delete the latest trusted issuer.
        let issuer = Module::<T>::trusted_claim_issuer(d.ticker).pop().unwrap();
    }: _(d.owner.origin, d.ticker, issuer.issuer.clone())
    verify {
        let trusted_issuers = Module::<T>::trusted_claim_issuer(d.ticker);
        ensure!(
            !trusted_issuers.contains(&issuer),
            "Default trusted claim issuer was not removed"
        );
    }

    change_compliance_requirement {
        let s in 1..MAX_SENDER_CONDITIONS_PER_COMPLIANCE;
        let r in 1..MAX_RECEIVER_CONDITIONS_PER_COMPLIANCE;

        // Add the compliance requirement.
        let d = ComplianceRequirementBuilder::<T>::new(MAX_TRUSTED_ISSUER_PER_CONDITION, s, r)
            .add_compliance_requirement().build();

        // Remove the latest one.
        let id = Module::<T>::get_latest_requirement_id(d.ticker);
        let new_req = ComplianceRequirement {
            id,
            ..Default::default()
        };
    }: _(d.owner.origin, d.ticker, new_req.clone())
    verify {
        let req = Module::<T>::asset_compliance( d.ticker)
            .requirements
            .into_iter()
            .find(|req| req.id == new_req.id)
            .unwrap();
        ensure!( req == new_req,
            "Compliance requirement was not updated");
    }

    replace_asset_compliance {
        let c in 0..(MAX_COMPLIANCE_REQUIREMENTS-1);

        // Add the compliance requirement.
        let d = ComplianceRequirementBuilder::<T>::new(
            MAX_TRUSTED_ISSUER_PER_CONDITION,
            MAX_SENDER_CONDITIONS_PER_COMPLIANCE,
            MAX_RECEIVER_CONDITIONS_PER_COMPLIANCE)
            .add_compliance_requirement().build();

        let issuers = make_issuers::<T>(MAX_TRUSTED_ISSUER_PER_CONDITION);
        let sender_conditions = make_conditions(MAX_SENDER_CONDITIONS_PER_COMPLIANCE, &issuers);
        let receiver_conditions = make_conditions(MAX_RECEIVER_CONDITIONS_PER_COMPLIANCE, &issuers);

        // Add new requirements to the asset.
        (0..c).for_each( |_i| {
            let _ = Module::<T>::add_compliance_requirement(
                d.owner.origin.clone().into(),
                d.ticker.clone(),
                sender_conditions.clone(),
                receiver_conditions.clone()).unwrap();
        });

        // Create the replacement asset compiance.
        let asset_compliance = (0..c).map(|id| {
            ComplianceRequirement {
                sender_conditions: sender_conditions.clone(),
                receiver_conditions: receiver_conditions.clone(),
                id,
            }}).collect::<Vec<_>>();
    }: _(d.owner.origin, d.ticker, asset_compliance.clone())
    verify {
        let reqs = Module::<T>::asset_compliance(d.ticker).requirements;
        ensure!( reqs == asset_compliance, "Asset compliance was not replaced");
    }

    reset_asset_compliance {
        // Add the compliance requirement.
        let d = ComplianceRequirementBuilder::<T>::new(
            MAX_TRUSTED_ISSUER_PER_CONDITION,
            MAX_SENDER_CONDITIONS_PER_COMPLIANCE,
            MAX_RECEIVER_CONDITIONS_PER_COMPLIANCE)
            .add_compliance_requirement().build();
    }: _(d.owner.origin, d.ticker)
    verify {
        ensure!(
            Module::<T>::asset_compliance(d.ticker).requirements.is_empty(),
            "Compliance Requeriment was not reset");
    }
}
