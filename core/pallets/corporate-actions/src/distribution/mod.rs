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

//! # Capital Distribution Module
//!
//! The capital distributions module provides functionality for distributing benefits,
//! whether predictable, or unpredictable, to tokenholders.
//!
//! The process works by first initiating the corporate action (CA) through `initiate_corporate_action`,
//! and then attaching a capital distribution to it via `distribute`.
//!
//! When attaching a distribution, the portfolio to withdraw from is provided,
//! as is the currency and amount of it to withdraw from the portfolio.
//! Additionally, a date (`payment_at`) is provided at which withdrawals may first happen,
//! as well as an optional expiry date `expires_at`,
//! at which benefits are forfeit and may be reclaimed by the CAA.
//!
//! As aforementioned, once `payment_at` is due, benefits may be withdrawn.
//! This can be done either through `claim`, which is pull-based. That is, holders withdraw themselves.
//! The other mechanism is via `push_benefit`, which with the CAA can push to a holder.
//! Once `expires_at` is reached, however, the remaining amount to distribute is forfeit,
//! and cannot be claimed by any holder, or pushed to them.
//! Instead, that amount can be reclaimed by the CAA.
//!
//! Before `payment_at` is due, however,
//! a planned distribution can be cancelled by calling `remove_distribution`.
//!
//! ## Overview
//!
//! The module provides functions for:
//!
//! - Starting a distribution.
//! - Claiming or pushing benefits of a distribution.
//! - Reclaiming unclaimed dividends.
//!
//! ### Terminology
//!
//! - **Currency:** The ticker being distributed to holders as a benefit, e.g., USDC or some such.
//! - **Payment-at date:** The date at which benefits may be claimed by or pushed to holders.
//! - **Expires-at date:** The date at which benefits are forfeit, and may be reclaimed by the CAA.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `distribute` starts a capital distribution.
//! - `claim` claims (pull) a benefit of an active capital distribution on behalf of a holder.
//! - `push_benefit` pushes a benefit of an active capital distribution to a holder.
//! - `reclaim` reclaims forfeited benefits of a capital distribution that has expired.
//! - `remove_distribution` removes a capital distribution which hasn't reached its payment date yet.

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

use crate as ca;
use ca::{CAId, Tax, Trait};
use codec::{Decode, Encode};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    traits::Get,
    weights::Weight,
};
use pallet_asset::{self as asset, checkpoint};
use pallet_identity::{self as identity, PermissionedCallOriginData};
use polymesh_common_utilities::{
    portfolio::PortfolioSubTrait,
    protocol_fee::{ChargeProtocolFee, ProtocolOp},
    with_transaction, CommonTrait,
};
use polymesh_primitives::{EventDid, IdentityId, Moment, PortfolioId, PortfolioNumber, Ticker};
use sp_runtime::traits::{CheckedDiv, CheckedMul};
#[cfg(feature = "std")]
use sp_runtime::{Deserialize, Serialize};
use sp_std::prelude::*;

type Asset<T> = asset::Module<T>;
type Checkpoint<T> = checkpoint::Module<T>;
type CA<T> = ca::Module<T>;
type Identity<T> = identity::Module<T>;
type Portfolio<T> = pallet_portfolio::Module<T>;

/// A capital distribution's various details.
///
/// All information contained is used by on-chain logic.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Encode, Decode)]
pub struct Distribution<Balance> {
    /// The portfolio to distribute from.
    pub from: PortfolioId,
    /// The currency that payouts happen in.
    pub currency: Ticker,
    /// Total amount to be distributed.
    pub amount: Balance,
    /// Amount left to distribute.
    pub remaining: Balance,
    /// Whether the CAA has claimed remaining funds.
    pub reclaimed: bool,
    /// A timestamp of payout start
    pub payment_at: Moment,
    /// An optional timestamp for payout end
    pub expires_at: Option<Moment>,
}

/// Has the distribution expired?
fn expired(expiry: Option<Moment>, now: Moment) -> bool {
    expiry.filter(|&e| e <= now).is_some()
}

/// Weight abstraction for the corporate actions module.
pub trait WeightInfo {
    fn distribute() -> Weight;
    fn claim(target_ids: u32, did_whts: u32) -> Weight;
    fn push_benefit(target_ids: u32, did_whts: u32) -> Weight;
    fn reclaim() -> Weight;
    fn remove_distribution() -> Weight;
}

decl_storage! {
    trait Store for Module<T: Trait> as CapitalDistribution {
        /// All capital distributions, tied to their respective corporate actions (CAs).
        ///
        /// (CAId) => Distribution
        Distributions get(fn distributions): map hasher(blake2_128_concat) CAId => Option<Distribution<T::Balance>>;

        /// Has an asset holder been paid yet?
        ///
        /// (CAId, DID) -> Was DID paid in the CAId?
        HolderPaid get(fn holder_paid): map hasher(blake2_128_concat) (CAId, IdentityId) => bool;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Start and attach a capital distribution, to the CA identified by `ca_id`,
        /// with `amount` funds in `currency` withdrawn from `portfolio` belonging to `origin`'s DID.
        ///
        /// The distribution will commence at `payment_at` and expire at `expires_at`,
        /// if provided, or if `None`, then there's no expiry.
        ///
        /// ## Arguments
        /// - `origin` which must be a signer for the CAA of `ca_id`.
        /// - `ca_id` identifies the CA to start a capital distribution for.
        /// - `portfolio` specifies the portfolio number of the CAA to distribute `amount` from.
        /// - `currency` to withdraw and distribute from the `portfolio`.
        /// - `amount` of `currency` to withdraw and distribute.
        /// - `payment_at` specifies when benefits may first be pushed or claimed.
        /// - `expires_at` specifies, if provided, when remaining benefits are forfeit
        ///    and may be reclaimed by `origin`.
        ///
        /// # Errors
        /// - `UnauthorizedAsAgent` if `origin` is not `ticker`'s sole CAA (owner is not necessarily the CAA).
        /// - `DistributingAsset` if `ca_id.ticker == currency`.
        /// - `ExpiryBeforePayment` if `expires_at.unwrap() <= payment_at`.
        /// - `NowAfterPayment` if `payment_at < now`.
        /// - `NoSuchCA` if `ca_id` does not identify an existing CA.
        /// - `NoRecordDate` if CA has no record date.
        /// - `RecordDateAfterStart` if CA's record date > payment_at.
        /// - `UnauthorizedCustodian` if CAA is not the custodian of `portfolio`.
        /// - `InsufficientPortfolioBalance` if `portfolio` has less than `amount` of `currency`.
        /// - `InsufficientBalance` if the protocol fee couldn't be charged.
        #[weight = <T as Trait>::DistWeightInfo::distribute()]
        pub fn distribute(
            origin,
            ca_id: CAId,
            portfolio: Option<PortfolioNumber>,
            currency: Ticker,
            amount: T::Balance,
            payment_at: Moment,
            expires_at: Option<Moment>,
        ) {
            // Ensure CA's asset is distinct from the distributed currency.
            ensure!(ca_id.ticker != currency, Error::<T>::DistributingAsset);

            // Ensure that any expiry date doesn't come before the payment date.
            ensure!(!expired(expires_at, payment_at), Error::<T>::ExpiryBeforePayment);

            // Ensure `now <= payment_at`.
            ensure!(<Checkpoint<T>>::now_unix() <= payment_at, Error::<T>::NowAfterPayment);

            // Ensure CA doesn't have a distribution yet.
            ensure!(!<Distributions<T>>::contains_key(ca_id), Error::<T>::AlreadyExists);

            // Ensure origin is CAA and that they have custody over `from`.
            // Also ensure secondary key has perms for `from` + portfolio is valid.
            let PermissionedCallOriginData {
                primary_did: caa,
                secondary_key,
                ..
            } = <CA<T>>::ensure_ca_agent_with_perms(origin, ca_id.ticker)?;
            let from = PortfolioId { did: caa, kind: portfolio.into() };
            <Portfolio<T>>::ensure_portfolio_custody(from, caa)?;
            <Portfolio<T>>::ensure_user_portfolio_permission(secondary_key.as_ref(), from)?;
            <Portfolio<T>>::ensure_portfolio_validity(&from)?;

            // Ensure that `ca_id` exists, that its a benefit.
            let caa = caa.for_event();
            let ca = <CA<T>>::ensure_ca_exists(ca_id)?;
            ensure!(ca.kind.is_benefit(), Error::<T>::CANotBenefit);

            // Ensure CA has a record `date <= payment_at`.
            // If we cannot, deriving a checkpoint,
            // used to determine each holder's allotment of the total `amount`,
            // is not possible.
            <CA<T>>::ensure_record_date_before_start(&ca, payment_at)?;

            // Ensure `from` has at least `amount` to later lock (1).
            <Portfolio<T>>::ensure_sufficient_balance(&from, &currency, &amount)?;

            // Charge the protocol fee. Last check; we are in commit phase after this.
            T::ProtocolFee::charge_fee(ProtocolOp::DistributionDistribute)?;

            // (1) Lock `amount` in `from`.
            <Portfolio<T>>::unchecked_lock_tokens(&from, &currency, &amount);

            // Commit to storage.
            let distribution = Distribution {
                from,
                currency,
                amount,
                remaining: amount,
                reclaimed: false,
                payment_at,
                expires_at,
            };
            <Distributions<T>>::insert(ca_id, distribution);

            // Emit event.
            Self::deposit_event(Event::<T>::Created(caa, ca_id, distribution));
        }

        /// Claim a benefit of the capital distribution attached to `ca_id`.
        ///
        /// Taxes are withheld as specified by the CA.
        /// Post-tax earnings are then transferred to the default portfolio of the `origin`'s DID.
        ///
        /// All benefits are rounded by truncation (down to first integer below).
        ///
        /// ## Arguments
        /// - `origin` which must be a holder of for the CAA of `ca_id`.
        /// - `ca_id` identifies the CA to start a capital distribution for.
        ///
        /// # Errors
        /// - `HolderAlreadyPaid` if `origin`'s DID has already received its benefit.
        /// - `NoSuchDistribution` if there's no capital distribution for `ca_id`.
        /// - `CannotClaimBeforeStart` if `now < payment_at`.
        /// - `CannotClaimAfterExpiry` if `now > expiry_at.unwrap()`.
        /// - `NoSuchCA` if `ca_id` does not identify an existing CA.
        /// - `NotTargetedByCA` if the CA does not target `origin`'s DID.
        /// - `BalanceAmountProductOverflowed` if `ba = balance * amount` would overflow.
        /// - `BalanceAmountProductSupplyDivisionFailed` if `ba * supply` would overflow.
        /// - Other errors can occur if the compliance manager rejects the transfer.
        #[weight = <T as Trait>::DistWeightInfo::claim(T::MaxTargetIds::get(), T::MaxDidWhts::get())]
        pub fn claim(origin, ca_id: CAId) {
            let did = <Identity<T>>::ensure_perms(origin)?;
            Self::transfer_benefit(did.for_event(), did, ca_id)?;
        }

        /// Push benefit of an ongoing distribution to the given `holder`.
        ///
        /// Taxes are withheld as specified by the CA.
        /// Post-tax earnings are then transferred to the default portfolio of the `origin`'s DID.
        ///
        /// All benefits are rounded by truncation (down to first integer below).
        ///
        /// ## Arguments
        /// - `origin` which must be a holder of for the CAA of `ca_id`.
        /// - `ca_id` identifies the CA with a capital distributions to push benefits for.
        /// - `holder` to push benefits to.
        ///
        /// # Errors
        /// - `UnauthorizedAsAgent` if `origin` is not the `ticker`'s CAA or owner.
        /// - `NoSuchDistribution` if there's no capital distribution for `ca_id`.
        /// - `CannotClaimBeforeStart` if `now < payment_at`.
        /// - `CannotClaimAfterExpiry` if `now > expiry_at.unwrap()`.
        /// - `NoSuchCA` if `ca_id` does not identify an existing CA.
        /// - `NotTargetedByCA` if the CA does not target `holder`.
        /// - `BalanceAmountProductOverflowed` if `ba = balance * amount` would overflow.
        /// - `BalanceAmountProductSupplyDivisionFailed` if `ba * supply` would overflow.
        /// - Other errors can occur if the compliance manager rejects the transfer.
        #[weight = <T as Trait>::DistWeightInfo::push_benefit(T::MaxTargetIds::get(), T::MaxDidWhts::get())]
        pub fn push_benefit(origin, ca_id: CAId, holder: IdentityId) {
            // N.B. we allow the asset owner to call this as well, not just the CAA.
            let caa_ish = Self::ensure_caa_or_owner(origin, ca_id.ticker)?.for_event();
            Self::transfer_benefit(caa_ish, holder, ca_id)?;
        }

        /// Assuming a distribution has expired,
        /// unlock the remaining amount in the distributor portfolio.
        ///
        /// ## Arguments
        /// - `origin` which must be the creator of the capital distribution tied to `ca_id`.
        /// - `ca_id` identifies the CA with a capital distribution to reclaim for.
        ///
        /// # Errors
        /// - `NoSuchDistribution` if there's no capital distribution for `ca_id`.
        /// - `NotDistributionCreator` if `origin` is not the original creator of the distribution.
        /// - `AlreadyReclaimed` if this function has already been called successfully.
        /// - `NotExpired` if `now < expiry`.
        #[weight = <T as Trait>::DistWeightInfo::reclaim()]
        pub fn reclaim(origin, ca_id: CAId) {
            // Ensure DID is the dist creator, they haven't reclaimed, and that expiry has passed.
            let did = <Identity<T>>::ensure_perms(origin)?;
            let dist = Self::ensure_distribution_exists(ca_id)?;
            ensure!(did == dist.from.did, Error::<T>::NotDistributionCreator);
            let did = did.for_event();
            ensure!(!dist.reclaimed, Error::<T>::AlreadyReclaimed);
            ensure!(expired(dist.expires_at, <Checkpoint<T>>::now_unix()), Error::<T>::NotExpired);

            // Unlock `remaining` of `currency` from DID's portfolio.
            // This won't fail, as we've already locked the requisite amount prior.
            Self::unlock(&dist, dist.remaining)?;

            // Zero `remaining` + note that we've reclaimed.
            <Distributions<T>>::insert(ca_id, Distribution { reclaimed: true, remaining:0u32.into(), ..dist });

            // Emit event.
            Self::deposit_event(Event::<T>::Reclaimed(did, ca_id, dist.remaining));
        }

        /// Removes a distribution that hasn't started yet,
        /// unlocking the full amount in the distributor portfolio.
        ///
        /// ## Arguments
        /// - `origin` which must be a signer for the CAA of `ca_id`.
        /// - `ca_id` identifies the CA with a not-yet-started capital distribution to remove.
        ///
        /// # Errors
        /// - `UnauthorizedAsAgent` if `origin` is not `ticker`'s sole CAA (owner is not necessarily the CAA).
        /// - `NoSuchDistribution` if there's no capital distribution for `ca_id`.
        /// - `DistributionStarted` if `payment_at >= now`.
        #[weight = <T as Trait>::DistWeightInfo::remove_distribution()]
        pub fn remove_distribution(origin, ca_id: CAId) {
            let caa = <CA<T>>::ensure_ca_agent(origin, ca_id.ticker)?.for_event();
            let dist = Self::ensure_distribution_exists(ca_id)?;
            Self::remove_distribution_base(caa, ca_id, &dist)?;
        }
    }
}

decl_event! {
    pub enum Event<T>
    where
        Balance = <T as CommonTrait>::Balance,
    {
        /// A capital distribution, with details included,
        /// was created by the DID (the CAA) for the CA specified by the `CAId`.
        ///
        /// (CAA of CAId's ticker, CA's ID, distribution details)
        Created(EventDid, CAId, Distribution<Balance>),

        /// A token holder's benefit of a capital distribution for the given `CAId` was claimed.
        ///
        /// (Caller DID, Holder/Claimant DID, CA's ID, updated distribution details, DID's benefit, DID's tax %)
        BenefitClaimed(EventDid, EventDid, CAId, Distribution<Balance>, Balance, Tax),

        /// Stats from `push_benefit` was emitted.
        ///
        /// (CAA/owner of CA's ticker, CA's ID, max requested DIDs, processed DIDs, failed DIDs)
        Reclaimed(EventDid, CAId, Balance),

        /// A capital distribution was removed.
        ///
        /// (Ticker's CAA, CA's ID)
        Removed(EventDid, CAId),
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// A corporate ballot was made for a non-benefit CA.
        CANotBenefit,
        /// A distribution already exists for this CA.
        AlreadyExists,
        /// A distributions provided expiry date was strictly before its payment date.
        /// In other words, everything to distribute would immediately be forfeited.
        ExpiryBeforePayment,
        /// Start-of-payment date is in the past, since it is strictly before now.
        NowAfterPayment,
        /// Currency that is distributed is the same as the CA's ticker.
        /// CAA is attempting a form of stock split, which is not what the extrinsic is for.
        DistributingAsset,
        /// The token holder has already been paid their benefit.
        HolderAlreadyPaid,
        /// A capital distribution doesn't exist for this CA.
        NoSuchDistribution,
        /// Distribution allotment cannot be claimed as the current time is before start-of-payment.
        CannotClaimBeforeStart,
        /// Distribution's expiry has passed. DID cannot claim anymore and has forfeited the benefits.
        CannotClaimAfterExpiry,
        /// Multiplication of the balance with the total payout amount overflowed.
        BalanceAmountProductOverflowed,
        /// A failed division of the balance amount product by the total supply.
        BalanceAmountProductSupplyDivisionFailed,
        /// DID is not the one who created the distribution.
        NotDistributionCreator,
        /// DID who created the distribution already did reclaim.
        AlreadyReclaimed,
        /// Distribution had not expired yet, or there's no expiry date.
        NotExpired,
        /// A distribution has been activated, as `payment_at <= now` holds.
        DistributionStarted,
    }
}

impl<T: Trait> Module<T> {
    /// Kill the distribution identified by `ca_id`.
    crate fn remove_distribution_base(
        caa: EventDid,
        ca_id: CAId,
        dist: &Distribution<T::Balance>,
    ) -> DispatchResult {
        // Cannot remove payment has started.
        Self::ensure_distribution_not_started(&dist)?;

        // Unlock and remove chain data.
        Self::unlock(&dist, dist.amount)?;
        <Distributions<T>>::remove(ca_id);

        // Emit event.
        Self::deposit_event(Event::<T>::Removed(caa, ca_id));
        Ok(())
    }

    /// Ensure that `now < payment_at`.
    crate fn ensure_distribution_not_started(dist: &Distribution<T::Balance>) -> DispatchResult {
        ensure!(
            <Checkpoint<T>>::now_unix() < dist.payment_at,
            Error::<T>::DistributionStarted
        );
        Ok(())
    }

    /// Transfer `holder`'s benefit in `ca_id` to them.
    fn transfer_benefit(actor: EventDid, holder: IdentityId, ca_id: CAId) -> DispatchResult {
        // Ensure holder not paid yet.
        ensure!(
            !HolderPaid::get((ca_id, holder)),
            Error::<T>::HolderAlreadyPaid
        );

        // Ensure we have an active distribution.
        let mut dist = Self::ensure_active_distribution(ca_id)?;

        // Fetch the CA data (cannot fail) + ensure CA targets DID.
        let ca = <CA<T>>::ensure_ca_exists(ca_id)?;
        <CA<T>>::ensure_ca_targets(&ca, &holder)?;

        // Extract CP + total supply at the record date.
        let cp_id = <CA<T>>::record_date_cp(&ca, ca_id);
        let supply = <CA<T>>::supply_at_cp(ca_id, cp_id);

        // Compute `balance * amount / supply`, i.e. DID's benefit.
        let balance = <CA<T>>::balance_at_cp(holder, ca_id, cp_id);
        let benefit = Self::benefit_of(balance, dist.amount, supply)?;

        // Compute withholding tax + gain.
        let tax = ca.tax_of(&holder);
        let gain = benefit - tax * benefit;

        with_transaction(|| {
            // Unlock `benefit` of `currency` from CAAs portfolio.
            Self::unlock(&dist, benefit)?;

            // Transfer remainder (`gain`) to DID.
            let to = PortfolioId::default_portfolio(holder);
            <Asset<T>>::base_transfer(dist.from, to, &dist.currency, gain)
        })?;

        // Note that DID was paid.
        HolderPaid::insert((ca_id, holder), true);
        let holder = holder.for_event();

        // Commit `dist` change to storage.
        dist.remaining -= benefit;
        <Distributions<T>>::insert(ca_id, dist);

        // Emit event.
        Self::deposit_event(Event::<T>::BenefitClaimed(
            actor, holder, ca_id, dist, benefit, tax,
        ));

        Ok(())
    }

    /// Unlock `amount` of `dist.currency` in the `dist.from` portfolio.
    fn unlock(dist: &Distribution<T::Balance>, amount: T::Balance) -> DispatchResult {
        <Portfolio<T>>::unlock_tokens(&dist.from, &dist.currency, &amount)
    }

    // Compute `balance * amount / supply`, i.e. DID's benefit.
    fn benefit_of(
        balance: T::Balance,
        amount: T::Balance,
        supply: T::Balance,
    ) -> Result<T::Balance, DispatchError> {
        balance
            .checked_mul(&amount)
            .ok_or(Error::<T>::BalanceAmountProductOverflowed)?
            .checked_div(&supply)
            .ok_or_else(|| Error::<T>::BalanceAmountProductSupplyDivisionFailed.into())
    }

    /// Ensure `ca_id` has some distribution and return it.
    fn ensure_distribution_exists(ca_id: CAId) -> Result<Distribution<T::Balance>, DispatchError> {
        <Distributions<T>>::get(ca_id).ok_or_else(|| Error::<T>::NoSuchDistribution.into())
    }

    /// Ensure `ca_id` has a started and non-expired, i.e. active, distribution, which is returned.
    fn ensure_active_distribution(ca_id: CAId) -> Result<Distribution<T::Balance>, DispatchError> {
        // Fetch the distribution, ensuring it exists + start date is satisfied + not expired.
        let dist = Self::ensure_distribution_exists(ca_id)?;
        let now = <Checkpoint<T>>::now_unix();
        ensure!(now >= dist.payment_at, Error::<T>::CannotClaimBeforeStart);
        ensure!(
            !expired(dist.expires_at, now),
            Error::<T>::CannotClaimAfterExpiry
        );
        Ok(dist)
    }

    /// Ensure that `origin` is authorized as a CA agent of the asset `ticker` or its owner.
    /// When `origin` is unsigned, `BadOrigin` occurs.
    /// If DID is not the CAA of `ticker` or its owner, `UnauthorizedAsAgent` occurs.
    fn ensure_caa_or_owner(origin: T::Origin, ticker: Ticker) -> Result<IdentityId, DispatchError> {
        let did = <Identity<T>>::ensure_perms(origin)?;
        ensure!(
            <CA<T>>::agent(ticker).filter(|caa| caa == &did).is_some()
                || <Asset<T>>::is_owner(&ticker, did),
            ca::Error::<T>::UnauthorizedAsAgent
        );
        Ok(did)
    }
}
