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

//! # Compliance Manager Module
//!
//! The Compliance Manager module provides functionality to set and evaluate a list of conditions.
//! Those conditions define transfer restrictions for the sender and receiver. For instance, you can limit your asset to investors
//! of specific jurisdictions.
//!
//!
//! ## Overview
//!
//! The Compliance Manager module provides functions for:
//!
//! - Adding conditions for allowing transfers.
//! - Removing conditions that allow transfers.
//! - Resetting all conditions.
//!
//! ### Use case
//!
//! This module is very versatile and offers infinite possibilities.
//! The conditions can dictate various requirements like:
//!
//! - Only accredited investors should be able to trade.
//! - Only valid CDD holders should be able to trade.
//! - Only those with credit score of greater than 800 should be able to purchase this token.
//! - People from "Wakanda" should only be able to trade with people from "Wakanda".
//! - People from "Gryffindor" should not be able to trade with people from "Slytherin" (But allowed to trade with anyone else).
//! - Only "Marvel" supporters should be allowed to buy "Avengers" token.
//!
//! ### Terminology
//!
//! - **AssetCompliance:** It is an array of compliance requirements that are currently enforced for a ticker.
//! - **ComplianceRequirement:** Every compliance requirement contains an array for sender conditions and an array for receiver conditions
//! - **sender conditions:** These are conditions that the sender of security tokens must follow
//! - **receiver conditions:** These are conditions that the receiver of security tokens must follow
//! - **Valid transfer:** For a transfer to be valid,
//!     All receiver and sender conditions of any of the asset compliance must be followed.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - [add_compliance_requirement](Module::add_compliance_requirement) - Adds a new compliance requirement to an asset's compliance.
//! - [remove_compliance_requirement](Module::remove_compliance_requirement) - Removes a compliance requirement from an asset's compliance.
//! - [reset_asset_compliance](Module::reset_asset_compliance) - Resets(remove) an asset's compliance.
//! - [pause_asset_compliance](Module::pause_asset_compliance) - Pauses the evaluation of asset compliance for a ticker before executing a
//! transaction.
//! - [add_default_trusted_claim_issuer](Module::add_default_trusted_claim_issuer) - Adds a default
//!  trusted claim issuer for a given asset.
//! - [remove_default_trusted_claim_issuer](Module::remove_default_trusted_claim_issuer) - Removes
//!  the default claim issuer.
//! - [change_compliance_requirement](Module::change_compliance_requirement) - Updates a compliance requirement, based on its id.
//! based on its id for a given asset.
//!
//! ### Public Functions
//!
//! - [verify_restriction](Module::verify_restriction) - Checks if a transfer is a valid transfer and returns the result

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]
#![feature(const_option)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

use codec::{Decode, Encode};
use core::result::Result;
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    traits::Get,
    weights::Weight,
};
use pallet_identity as identity;
pub use polymesh_common_utilities::traits::compliance_manager::WeightInfo;
use polymesh_common_utilities::{
    asset::AssetFnTrait,
    balances::Trait as BalancesTrait,
    compliance_manager::Trait as ComplianceManagerTrait,
    constants::*,
    identity::Trait as IdentityTrait,
    protocol_fee::{ChargeProtocolFee, ProtocolOp},
};
use polymesh_primitives::{
    proposition, storage_migrate_on, storage_migration_ver, Claim, Condition, ConditionType,
    IdentityId, Ticker, TrustedIssuer,
};

#[cfg(feature = "std")]
use sp_runtime::{Deserialize, Serialize};
use sp_std::{
    convert::{From, TryFrom},
    prelude::*,
};

/// The module's configuration trait.
pub trait Trait:
    pallet_timestamp::Trait + frame_system::Trait + BalancesTrait + IdentityTrait
{
    /// The overarching event type.
    type Event: From<Event> + Into<<Self as frame_system::Trait>::Event>;

    /// Asset module
    type Asset: AssetFnTrait<Self::Balance, Self::AccountId, Self::Origin>;

    /// Weight details of all extrinsic
    type WeightInfo: WeightInfo;

    /// The maximum claim reads that are allowed to happen in worst case of a condition resolution
    type MaxConditionComplexity: Get<u32>;
}

/// A compliance requirement.
/// All sender and receiver conditions of the same compliance requirement must be true in order to execute the transfer.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Encode, Decode, Default, Clone, PartialEq, Eq, Debug)]
pub struct ComplianceRequirement {
    pub sender_conditions: Vec<Condition>,
    pub receiver_conditions: Vec<Condition>,
    /// Unique identifier of the compliance requirement
    pub id: u32,
}

impl ComplianceRequirement {
    /// Dedup `ClaimType`s in `TrustedFor::Specific`.
    fn dedup(&mut self) {
        let dedup_condition = |conds: &mut [Condition]| {
            conds
                .iter_mut()
                .flat_map(|c| &mut c.issuers)
                .for_each(|issuer| issuer.dedup())
        };
        dedup_condition(&mut self.sender_conditions);
        dedup_condition(&mut self.receiver_conditions);
    }
}

/// A compliance requirement along with its evaluation result
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
#[derive(Encode, Decode, Clone, PartialEq, Eq)]
pub struct ComplianceRequirementResult {
    pub sender_conditions: Vec<ConditionResult>,
    pub receiver_conditions: Vec<ConditionResult>,
    /// Unique identifier of the compliance requirement.
    pub id: u32,
    /// Result of this transfer condition's evaluation.
    pub result: bool,
}

impl From<ComplianceRequirement> for ComplianceRequirementResult {
    fn from(requirement: ComplianceRequirement) -> Self {
        let from_conds = |conds: Vec<_>| conds.into_iter().map(ConditionResult::from).collect();
        Self {
            sender_conditions: from_conds(requirement.sender_conditions),
            receiver_conditions: from_conds(requirement.receiver_conditions),
            id: requirement.id,
            result: true,
        }
    }
}

/// An individual condition along with its evaluation result
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
#[derive(Encode, Decode, Clone, PartialEq, Eq)]
pub struct ConditionResult {
    // Condition being evaluated
    pub condition: Condition,
    // Result of evaluation
    pub result: bool,
}

impl From<Condition> for ConditionResult {
    fn from(condition: Condition) -> Self {
        Self {
            condition,
            result: true,
        }
    }
}

/// List of compliance requirements associated to an asset.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
#[derive(Encode, Decode, Default, Clone, PartialEq, Eq)]
pub struct AssetCompliance {
    /// This flag indicates if asset compliance should be enforced
    pub paused: bool,
    /// List of compliance requirements.
    pub requirements: Vec<ComplianceRequirement>,
}

type Identity<T> = identity::Module<T>;

/// Asset compliance and it's evaluation result
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
#[derive(Encode, Decode, Clone, PartialEq, Eq)]
pub struct AssetComplianceResult {
    /// This flag indicates if asset compliance should be enforced
    pub paused: bool,
    /// List of compliance requirements.
    pub requirements: Vec<ComplianceRequirementResult>,
    // Final evaluation result of the asset compliance
    pub result: bool,
}

impl From<AssetCompliance> for AssetComplianceResult {
    fn from(asset_compliance: AssetCompliance) -> Self {
        Self {
            paused: asset_compliance.paused,
            requirements: asset_compliance
                .requirements
                .into_iter()
                .map(ComplianceRequirementResult::from)
                .collect(),
            result: asset_compliance.paused,
        }
    }
}

pub mod weight_for {
    use super::*;

    pub fn weight_for_verify_restriction<T: Trait>(no_of_compliance_requirements: u64) -> Weight {
        no_of_compliance_requirements * 100_000_000
    }

    pub fn weight_for_reading_asset_compliance<T: Trait>() -> Weight {
        T::DbWeight::get().reads(1) + 1_000_000
    }
}

// A value placed in storage that represents the current version of the this storage. This value
// is used by the `on_runtime_upgrade` logic to determine whether we run storage migration logic.
storage_migration_ver!(1);

decl_storage! {
    trait Store for Module<T: Trait> as ComplianceManager {
        /// Asset compliance for a ticker (Ticker -> AssetCompliance)
        pub AssetCompliances get(fn asset_compliance): map hasher(blake2_128_concat) Ticker => AssetCompliance;
        /// List of trusted claim issuer Ticker -> Issuer Identity
        pub TrustedClaimIssuer get(fn trusted_claim_issuer): map hasher(blake2_128_concat) Ticker => Vec<TrustedIssuer>;
        /// Storage version.
        StorageVersion get(fn storage_version) build(|_| Version::new(1).unwrap()): Version;
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// User is not authorized.
        Unauthorized,
        /// Did not exist
        DidNotExist,
        /// Compliance requirement id doesn't exist
        InvalidComplianceRequirementId,
        /// Issuer exist but trying to add it again
        IncorrectOperationOnTrustedIssuer,
        /// There are duplicate compliance requirements.
        DuplicateComplianceRequirements,
        /// The worst case scenario of the compliance requirement is too complex
        ComplianceRequirementTooComplex,
    }
}

decl_module! {
    /// The module declaration.
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            use polymesh_primitives::{migrate::{Empty, migrate_map}, condition::TrustedIssuerOld};

            let storage_ver = StorageVersion::get();

            storage_migrate_on!(storage_ver, 1, {
                migrate_map::<Vec<TrustedIssuerOld>, _>(b"ComplianceManager", b"TrustedClaimIssuer", |_| Empty);
            });

            1_000
        }

        /// Adds a compliance requirement to an asset's compliance by ticker.
        /// If the compliance requirement is a duplicate, it does nothing.
        ///
        /// # Arguments
        /// * origin - Signer of the dispatchable. It should be the owner of the ticker
        /// * ticker - Symbol of the asset
        /// * sender_conditions - Sender transfer conditions.
        /// * receiver_conditions - Receiver transfer conditions.
        #[weight = <T as Trait>::WeightInfo::add_compliance_requirement( sender_conditions.len() as u32, receiver_conditions.len() as u32)]
        pub fn add_compliance_requirement(origin, ticker: Ticker, sender_conditions: Vec<Condition>, receiver_conditions: Vec<Condition>) {
            let did = T::Asset::ensure_perms_owner_asset(origin, &ticker)?;

            <<T as IdentityTrait>::ProtocolFee>::charge_fee(
                ProtocolOp::ComplianceManagerAddComplianceRequirement
            )?;
            let id = Self::get_latest_requirement_id(ticker) + 1u32;
            let mut new_requirement = ComplianceRequirement { sender_conditions, receiver_conditions, id };
            new_requirement.dedup();

            let mut asset_compliance = AssetCompliances::get(ticker);
            let reqs = &mut asset_compliance.requirements;

            if !reqs
                .iter()
                .any(|requirement| requirement.sender_conditions == new_requirement.sender_conditions && requirement.receiver_conditions == new_requirement.receiver_conditions)
            {
                reqs.push(new_requirement.clone());
                Self::verify_compliance_complexity(&reqs, ticker, 0)?;
                AssetCompliances::insert(&ticker, asset_compliance);
                Self::deposit_event(Event::ComplianceRequirementCreated(did, ticker, new_requirement));
            }
        }

        /// Removes a compliance requirement from an asset's compliance.
        ///
        /// # Arguments
        /// * origin - Signer of the dispatchable. It should be the owner of the ticker
        /// * ticker - Symbol of the asset
        /// * id - Compliance requirement id which is need to be removed
        #[weight = <T as Trait>::WeightInfo::remove_compliance_requirement()]
        pub fn remove_compliance_requirement(origin, ticker: Ticker, id: u32) {
            let did = T::Asset::ensure_perms_owner_asset(origin, &ticker)?;

            AssetCompliances::try_mutate(ticker, |AssetCompliance { requirements, .. }| {
                let before = requirements.len();
                requirements.retain(|requirement| requirement.id != id);
                ensure!(before != requirements.len(), Error::<T>::InvalidComplianceRequirementId);
                Ok(()) as DispatchResult
            })?;

            Self::deposit_event(Event::ComplianceRequirementRemoved(did, ticker, id));
        }

        /// Replaces an asset's compliance by ticker with a new compliance.
        ///
        /// # Arguments
        /// * `ticker` - the asset ticker,
        /// * `asset_compliance - the new asset compliance.
        ///
        /// # Errors
        /// * `Unauthorized` if `origin` is not the owner of the ticker.
        /// * `DuplicateAssetCompliance` if `asset_compliance` contains multiple entries with the same `requirement_id`.
        ///
        #[weight = <T as Trait>::WeightInfo::replace_asset_compliance( asset_compliance.len() as u32)]
        pub fn replace_asset_compliance(origin, ticker: Ticker, asset_compliance: Vec<ComplianceRequirement>) {
            let did = T::Asset::ensure_perms_owner_asset(origin, &ticker)?;

            // Ensure there are no duplicate requirement ids.
            let mut asset_compliance = asset_compliance;
            let start_len = asset_compliance.len();
            asset_compliance.dedup_by_key(|r| r.id);
            ensure!(start_len == asset_compliance.len(), Error::<T>::DuplicateComplianceRequirements);

            // Dedup `ClaimType`s in `TrustedFor::Specific`.
            asset_compliance.iter_mut().for_each(|r| r.dedup());

            Self::verify_compliance_complexity(&asset_compliance, ticker, 0)?;
            AssetCompliances::mutate(&ticker, |old| old.requirements = asset_compliance.clone());
            Self::deposit_event(Event::AssetComplianceReplaced(did, ticker, asset_compliance));
        }

        /// Removes an asset's compliance
        ///
        /// # Arguments
        /// * origin - Signer of the dispatchable. It should be the owner of the ticker
        /// * ticker - Symbol of the asset
        #[weight = <T as Trait>::WeightInfo::reset_asset_compliance()]
        pub fn reset_asset_compliance(origin, ticker: Ticker) {
            let did = T::Asset::ensure_perms_owner_asset(origin, &ticker)?;
            AssetCompliances::remove(ticker);
            Self::deposit_event(Event::AssetComplianceReset(did, ticker));
        }

        /// Pauses the verification of conditions for `ticker` during transfers.
        ///
        /// # Arguments
        /// * origin - Signer of the dispatchable. It should be the owner of the ticker
        /// * ticker - Symbol of the asset
        #[weight = <T as Trait>::WeightInfo::pause_asset_compliance()]
        pub fn pause_asset_compliance(origin, ticker: Ticker) {
            let did = Self::pause_resume_asset_compliance(origin, ticker, true)?;
            Self::deposit_event(Event::AssetCompliancePaused(did, ticker));
        }

        /// Resumes the verification of conditions for `ticker` during transfers.
        ///
        /// # Arguments
        /// * origin - Signer of the dispatchable. It should be the owner of the ticker
        /// * ticker - Symbol of the asset
        #[weight = <T as Trait>::WeightInfo::resume_asset_compliance()]
        pub fn resume_asset_compliance(origin, ticker: Ticker) {
            let did = Self::pause_resume_asset_compliance(origin, ticker, false)?;
            Self::deposit_event(Event::AssetComplianceResumed(did, ticker));
        }

        /// Adds another default trusted claim issuer at the ticker level.
        ///
        /// # Arguments
        /// * origin - Signer of the dispatchable. It should be the owner of the ticker.
        /// * ticker - Symbol of the asset.
        /// * issuer - IdentityId of the trusted claim issuer.
        #[weight = <T as Trait>::WeightInfo::add_default_trusted_claim_issuer()]
        pub fn add_default_trusted_claim_issuer(origin, ticker: Ticker, issuer: TrustedIssuer) {
            let did = T::Asset::ensure_perms_owner_asset(origin, &ticker)?;
            ensure!(<Identity<T>>::is_identity_exists(&issuer.issuer), Error::<T>::DidNotExist);
            TrustedClaimIssuer::try_mutate(ticker, |issuers| {
                ensure!(!issuers.contains(&issuer), Error::<T>::IncorrectOperationOnTrustedIssuer);
                Self::base_verify_compliance_complexity(&AssetCompliances::get(ticker).requirements, issuers.len() + 1)?;
                issuers.push(issuer.clone());
                Ok(()) as DispatchResult
            })?;
            Self::deposit_event(Event::TrustedDefaultClaimIssuerAdded(did, ticker, issuer));
        }

        /// Removes the given `issuer` from the set of default trusted claim issuers at the ticker level.
        ///
        /// # Arguments
        /// * origin - Signer of the dispatchable. It should be the owner of the ticker.
        /// * ticker - Symbol of the asset.
        /// * issuer - IdentityId of the trusted claim issuer.
        #[weight = <T as Trait>::WeightInfo::remove_default_trusted_claim_issuer()]
        pub fn remove_default_trusted_claim_issuer(origin, ticker: Ticker, issuer: IdentityId) {
            let did = T::Asset::ensure_perms_owner_asset(origin, &ticker)?;
            TrustedClaimIssuer::try_mutate(ticker, |issuers| {
                let len = issuers.len();
                issuers.retain(|ti| ti.issuer != issuer);
                ensure!(len != issuers.len(), Error::<T>::IncorrectOperationOnTrustedIssuer);
                Ok(()) as DispatchResult
            })?;
            Self::deposit_event(Event::TrustedDefaultClaimIssuerRemoved(did, ticker, issuer));
        }

        /// Modify an existing compliance requirement of a given ticker.
        ///
        /// # Arguments
        /// * origin - Signer of the dispatchable. It should be the owner of the ticker.
        /// * ticker - Symbol of the asset.
        /// * new_req - Compliance requirement.
        #[weight = <T as Trait>::WeightInfo::change_compliance_requirement(
            new_req.sender_conditions.len() as u32,
            new_req.receiver_conditions.len() as u32)]
        pub fn change_compliance_requirement(origin, ticker: Ticker, new_req: ComplianceRequirement) {
            let did = T::Asset::ensure_perms_owner_asset(origin, &ticker)?;
            ensure!(Self::get_latest_requirement_id(ticker) >= new_req.id, Error::<T>::InvalidComplianceRequirementId);

            let mut asset_compliance = AssetCompliances::get(ticker);
            let reqs = &mut asset_compliance.requirements;
            if let Some(req) = reqs.iter_mut().find(|req| req.id == new_req.id) {
                let mut new_req = new_req;
                new_req.dedup();

                *req = new_req.clone();
                Self::verify_compliance_complexity(&reqs, ticker, 0)?;
                AssetCompliances::insert(&ticker, asset_compliance);
                Self::deposit_event(Event::ComplianceRequirementChanged(did, ticker, new_req));
            }
        }
    }
}

decl_event!(
    pub enum Event {
        /// Emitted when new compliance requirement is created.
        /// (caller DID, Ticker, ComplianceRequirement).
        ComplianceRequirementCreated(IdentityId, Ticker, ComplianceRequirement),
        /// Emitted when a compliance requirement is removed.
        /// (caller DID, Ticker, requirement_id).
        ComplianceRequirementRemoved(IdentityId, Ticker, u32),
        /// Emitted when an asset compliance is replaced.
        /// Parameters: caller DID, ticker, new asset compliance.
        AssetComplianceReplaced(IdentityId, Ticker, Vec<ComplianceRequirement>),
        /// Emitted when an asset compliance of a ticker is reset.
        /// (caller DID, Ticker).
        AssetComplianceReset(IdentityId, Ticker),
        /// Emitted when an asset compliance for a given ticker gets resume.
        /// (caller DID, Ticker).
        AssetComplianceResumed(IdentityId, Ticker),
        /// Emitted when an asset compliance for a given ticker gets paused.
        /// (caller DID, Ticker).
        AssetCompliancePaused(IdentityId, Ticker),
        /// Emitted when compliance requirement get modified/change.
        /// (caller DID, Ticker, ComplianceRequirement).
        ComplianceRequirementChanged(IdentityId, Ticker, ComplianceRequirement),
        /// Emitted when default claim issuer list for a given ticker gets added.
        /// (caller DID, Ticker, Added TrustedIssuer).
        TrustedDefaultClaimIssuerAdded(IdentityId, Ticker, TrustedIssuer),
        /// Emitted when default claim issuer list for a given ticker get removed.
        /// (caller DID, Ticker, Removed TrustedIssuer).
        TrustedDefaultClaimIssuerRemoved(IdentityId, Ticker, IdentityId),
    }
);

impl<T: Trait> Module<T> {
    /// Fetches all claims of `target` identity with type
    /// and scope from `claim` and generated by any of `issuers`.
    fn fetch_claims<'a>(
        target: IdentityId,
        claim: &'a Claim,
        issuers: &'a [TrustedIssuer],
    ) -> impl 'a + Iterator<Item = Claim> {
        let claim_type = claim.claim_type();
        let scope = claim.as_scope();

        issuers
            .iter()
            .filter(move |issuer| issuer.is_trusted_for(claim_type))
            .filter_map(move |issuer| {
                Identity::<T>::fetch_claim(target, claim_type, issuer.issuer, scope.cloned())
                    .map(|id_claim| id_claim.claim)
            })
    }

    /// Returns trusted issuers specified in `condition` if any,
    /// or otherwise returns the default trusted issuers for `ticker`.
    /// Defaults are cached in `slot`.
    fn issuers_for<'a>(
        ticker: &Ticker,
        condition: &'a Condition,
        slot: &'a mut Option<Vec<TrustedIssuer>>,
    ) -> &'a [TrustedIssuer] {
        if condition.issuers.is_empty() {
            slot.get_or_insert_with(|| Self::trusted_claim_issuer(ticker))
        } else {
            &condition.issuers
        }
    }

    /// Fetches the proposition context for target `id` and specific `condition`.
    /// Default trusted issuers, if fetched, are cached in `slot`.
    fn fetch_context<'a>(
        id: IdentityId,
        ticker: &Ticker,
        slot: &'a mut Option<Vec<TrustedIssuer>>,
        condition: &'a Condition,
        primary_issuance_agent: IdentityId,
    ) -> proposition::Context<impl 'a + Iterator<Item = Claim>> {
        // Because of `-> impl Iterator`, we need to return a **single type** in each of the branches below.
        // To do this, we use `Either<Either<MatchArm1, MatchArm2>, MatchArm3>`,
        // equivalent to a 3-variant enum with iterators in each variant corresponding to the branches below.
        // `Left(Left(arm1))`, `Left(Right(arm2))` and `Right(arm3)` correspond to arms 1, 2 and 3 respectively.
        use either::Either::{Left, Right};

        let claims = match &condition.condition_type {
            ConditionType::IsPresent(claim) | ConditionType::IsAbsent(claim) => Left(Left(
                Self::fetch_claims(id, claim, Self::issuers_for(ticker, condition, slot)),
            )),
            ConditionType::IsAnyOf(claims) | ConditionType::IsNoneOf(claims) => {
                let issuers = Self::issuers_for(ticker, condition, slot);
                Left(Right(claims.iter().flat_map(move |claim| {
                    Self::fetch_claims(id, claim, issuers)
                })))
            }
            ConditionType::IsIdentity(_) => Right(core::iter::empty()),
        };

        proposition::Context {
            claims,
            id,
            primary_issuance_agent,
        }
    }

    /// Loads the context for each condition in `conditions` and verifies that all of them evaluate to `true`.
    fn are_all_conditions_satisfied(
        ticker: &Ticker,
        did: IdentityId,
        conditions: &[Condition],
        primary_issuance_agent: IdentityId,
    ) -> bool {
        let slot = &mut None;
        conditions.iter().all(|condition| {
            Self::is_condition_satisfied(ticker, did, condition, primary_issuance_agent, slot)
        })
    }

    /// Checks whether the given condition is satisfied or not.
    fn is_condition_satisfied(
        ticker: &Ticker,
        did: IdentityId,
        condition: &Condition,
        primary_issuance_agent: IdentityId,
        slot: &mut Option<Vec<TrustedIssuer>>,
    ) -> bool {
        let context = Self::fetch_context(did, ticker, slot, &condition, primary_issuance_agent);
        proposition::run(&condition, context)
    }

    /// Returns whether all conditions, in their proper context, hold when evaluated.
    /// As a side-effect, each condition will be updated with its result,
    /// implying strict (non-lazy) evaluation of the conditions.
    fn evaluate_conditions(
        ticker: &Ticker,
        did: IdentityId,
        conditions: &mut [ConditionResult],
        primary_issuance_agent: IdentityId,
    ) -> bool {
        conditions.iter_mut().fold(true, |result, condition| {
            let cond = &condition.condition;
            let issuers = &mut None;
            let context = Self::fetch_context(did, ticker, issuers, cond, primary_issuance_agent);
            condition.result = proposition::run(cond, context);
            result & condition.result
        })
    }

    /// Pauses or resumes the asset compliance.
    fn pause_resume_asset_compliance(
        origin: T::Origin,
        ticker: Ticker,
        pause: bool,
    ) -> Result<IdentityId, DispatchError> {
        let did = T::Asset::ensure_perms_owner_asset(origin, &ticker)?;
        AssetCompliances::mutate(&ticker, |compliance| compliance.paused = pause);
        Ok(did)
    }

    /// Compute the id of the last requirement in a `ticker`'s compliance rules.
    fn get_latest_requirement_id(ticker: Ticker) -> u32 {
        Self::asset_compliance(ticker)
            .requirements
            .last()
            .map(|r| r.id)
            .unwrap_or(0)
    }

    /// verifies all requirements and returns the result in an array of booleans.
    /// this does not care if the requirements are paused or not. It is meant to be
    /// called only in failure conditions
    pub fn granular_verify_restriction(
        ticker: &Ticker,
        from_did_opt: Option<IdentityId>,
        to_did_opt: Option<IdentityId>,
    ) -> AssetComplianceResult {
        let primary_issuance_agent = T::Asset::primary_issuance_agent_or_owner(ticker);
        let asset_compliance = Self::asset_compliance(ticker);

        let mut asset_compliance_with_results = AssetComplianceResult::from(asset_compliance);

        for requirements in &mut asset_compliance_with_results.requirements {
            if let Some(from_did) = from_did_opt {
                // Evaluate all sender conditions
                if !Self::evaluate_conditions(
                    ticker,
                    from_did,
                    &mut requirements.sender_conditions,
                    primary_issuance_agent,
                ) {
                    // If the result of any of the sender conditions was false, set this requirements result to false.
                    requirements.result = false;
                }
            }
            if let Some(to_did) = to_did_opt {
                // Evaluate all receiver conditions
                if !Self::evaluate_conditions(
                    ticker,
                    to_did,
                    &mut requirements.receiver_conditions,
                    primary_issuance_agent,
                ) {
                    // If the result of any of the receiver conditions was false, set this requirements result to false.
                    requirements.result = false;
                }
            }

            asset_compliance_with_results.result |= requirements.result;
        }
        asset_compliance_with_results
    }

    /// Verify that `asset_compliance`, with `add` number of default issuers to add,
    /// is within the maximum condition complexity allowed.
    pub fn verify_compliance_complexity(
        asset_compliance: &[ComplianceRequirement],
        ticker: Ticker,
        add: usize,
    ) -> DispatchResult {
        let count = TrustedClaimIssuer::decode_len(ticker)
            .unwrap_or_default()
            .saturating_add(add);
        Self::base_verify_compliance_complexity(asset_compliance, count)
    }

    /// Verify that `asset_compliance`, with `default_issuer_count` number of default issuers,
    /// is within the maximum condition complexity allowed.
    pub fn base_verify_compliance_complexity(
        asset_compliance: &[ComplianceRequirement],
        default_issuer_count: usize,
    ) -> DispatchResult {
        let complexity = asset_compliance
            .iter()
            .flat_map(|requirement| {
                requirement
                    .sender_conditions
                    .iter()
                    .chain(requirement.receiver_conditions.iter())
            })
            .fold(0usize, |complexity, condition| {
                let (claims, issuers) = condition.complexity();
                complexity.saturating_add(claims.saturating_mul(match issuers {
                    0 => default_issuer_count,
                    _ => issuers,
                }))
            });
        if let Ok(complexity_u32) = u32::try_from(complexity) {
            if complexity_u32 <= T::MaxConditionComplexity::get() {
                return Ok(());
            }
        }
        Err(Error::<T>::ComplianceRequirementTooComplex.into())
    }
}

impl<T: Trait> ComplianceManagerTrait<T::Balance> for Module<T> {
    ///  Sender restriction verification
    fn verify_restriction(
        ticker: &Ticker,
        from_did_opt: Option<IdentityId>,
        to_did_opt: Option<IdentityId>,
        _value: T::Balance,
        primary_issuance_agent: IdentityId,
    ) -> Result<u8, DispatchError> {
        // Transfer is valid if ALL receiver AND sender conditions of ANY asset conditions are valid.
        let asset_compliance = Self::asset_compliance(ticker);
        if asset_compliance.paused {
            return Ok(ERC1400_TRANSFER_SUCCESS);
        }

        for requirement in asset_compliance.requirements {
            if let Some(from_did) = from_did_opt {
                if !Self::are_all_conditions_satisfied(
                    ticker,
                    from_did,
                    &requirement.sender_conditions,
                    primary_issuance_agent,
                ) {
                    // Skips checking receiver conditions because sender conditions are not satisfied.
                    continue;
                }
            }

            if let Some(to_did) = to_did_opt {
                if Self::are_all_conditions_satisfied(
                    ticker,
                    to_did,
                    &requirement.receiver_conditions,
                    primary_issuance_agent,
                ) {
                    // All conditions satisfied, return early
                    return Ok(ERC1400_TRANSFER_SUCCESS);
                }
            }
        }
        Ok(ERC1400_TRANSFER_FAILURE)
    }
}
