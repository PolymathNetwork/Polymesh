// Copyright (c) 2020 Polymath

//! # Sto Module
//!
//! Sto module creates and manages security token offerings
//!
//! ## Overview
//!
//! Primary issuance agent's can create and manage fundraisers of assets.
//! Fundraisers are of fixed supply, with optional expiry and tiered pricing.
//! Fundraisers allow a single payment asset, known as the raising asset.
//! Investors can invest through on-chain balance or off-chain receipts.
//!
//! ## Dispatchable Functions
//!
//! - `create_fundraiser` - Create a new fundraiser.
//! - `invest` - Invest in a fundraiser.
//! - `freeze_fundraiser` - Freeze a fundraiser.
//! - `unfreeze_fundraiser` - Unfreeze a fundraiser.
//! - `modify_fundraiser_window` - Modify the time window a fundraiser is active.
//! - `stop` - stop a fundraiser.

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

use codec::{Decode, Encode};
use core::mem;
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure,
};
use pallet_asset as asset;
use pallet_identity::{self as identity, PermissionedCallOriginData};
use pallet_portfolio::{self as portfolio, Trait as PortfolioTrait};
use pallet_settlement::{
    self as settlement, Leg, ReceiptDetails, SettlementType, Trait as SettlementTrait, VenueInfo,
    VenueType,
};
use pallet_timestamp::{self as timestamp, Trait as TimestampTrait};
use polymesh_common_utilities::{
    portfolio::PortfolioSubTrait,
    traits::{asset::AssetFnTrait, identity::Trait as IdentityTrait},
    with_transaction, CommonTrait,
};
use polymesh_primitives_derive::VecU8StrongTyped;

use frame_support::weights::Weight;
use polymesh_primitives::{IdentityId, PortfolioId, SecondaryKey, Ticker};
use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul, Saturating};
use sp_runtime::DispatchError;
use sp_std::{collections::btree_set::BTreeSet, prelude::*};

pub const MAX_TIERS: usize = 10;

type Identity<T> = identity::Module<T>;
type Settlement<T> = settlement::Module<T>;
type Timestamp<T> = timestamp::Module<T>;
type Portfolio<T> = portfolio::Module<T>;
type Asset<T> = asset::Module<T>;

/// Status of a Fundraiser.
#[derive(Clone, PartialEq, Eq, Encode, Decode, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum FundraiserStatus {
    /// Fundraiser is open for investments if start_time <= current_time < end_time.
    Live,
    /// Fundraiser has been frozen, New investments can not be made right now.
    Frozen,
    /// Fundraiser has been stopped.
    Closed,
}

impl Default for FundraiserStatus {
    fn default() -> Self {
        Self::Closed
    }
}

/// Details about the Fundraiser.
#[derive(Encode, Decode, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Fundraiser<Balance, Moment> {
    /// The primary issuance agent that created the `Fundraiser`.
    pub creator: IdentityId,
    /// Portfolio containing the asset being offered.
    pub offering_portfolio: PortfolioId,
    /// Asset being offered.
    pub offering_asset: Ticker,
    /// Portfolio receiving funds raised.
    pub raising_portfolio: PortfolioId,
    /// Asset to receive payment in.
    pub raising_asset: Ticker,
    /// Tiers of the fundraiser.
    /// Each tier has a set amount of tokens available at a fixed price.
    /// The sum of the tiers is the total amount available in this fundraiser.
    pub tiers: Vec<FundraiserTier<Balance>>,
    /// Id of the venue to use for this fundraise.
    pub venue_id: u64,
    /// Start time of the fundraiser.
    pub start: Moment,
    /// End time of the fundraiser.
    pub end: Option<Moment>,
    /// Fundraiser status.
    pub status: FundraiserStatus,
    /// Minimum raising amount per invest transaction.
    pub minimum_investment: Balance,
}

/// Single tier of a tiered pricing model.
#[derive(Encode, Decode, Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PriceTier<Balance> {
    /// Total amount available.
    pub total: Balance,
    /// Price per unit.
    pub price: Balance,
}

/// Single price tier of a `Fundraiser`.
/// Similar to a `PriceTier` but with an extra field `remaining` for tracking the amount available for purchase in a tier.
#[derive(Encode, Decode, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct FundraiserTier<Balance> {
    /// Total amount available.
    pub total: Balance,
    /// Price per unit.
    pub price: Balance,
    /// Total amount remaining for sale, set to `total` and decremented until `0`.
    pub remaining: Balance,
}

impl<Balance: Clone> Into<FundraiserTier<Balance>> for PriceTier<Balance> {
    fn into(self) -> FundraiserTier<Balance> {
        FundraiserTier {
            total: self.total.clone(),
            price: self.price,
            remaining: self.total,
        }
    }
}

/// Wrapper type for Fundraiser name
#[derive(
    Decode, Encode, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, VecU8StrongTyped,
)]
pub struct FundraiserName(Vec<u8>);

pub trait WeightInfo {
    fn create_fundraiser(i: u32) -> Weight;
    fn invest() -> Weight;
    fn freeze_fundraiser() -> Weight;
    fn unfreeze_fundraiser() -> Weight;
    fn modify_fundraiser_window() -> Weight;
    fn stop() -> Weight;
}

pub trait Trait: frame_system::Trait + IdentityTrait + SettlementTrait + PortfolioTrait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// Weight information for extrinsic of the sto pallet.
    type WeightInfo: WeightInfo;
}

decl_event!(
    pub enum Event<T>
    where
        Balance = <T as CommonTrait>::Balance,
        Moment = <T as TimestampTrait>::Moment,
    {
        /// A new fundraiser has been created.
        /// (primary issuance agent, fundraiser id, fundraiser name, fundraiser details)
        FundraiserCreated(IdentityId, u64, FundraiserName, Fundraiser<Balance, Moment>),
        /// An investor invested in the fundraiser.
        /// (Investor, fundraiser_id, offering token, raise token, offering_token_amount, raise_token_amount)
        Invested(IdentityId, u64, Ticker, Ticker, Balance, Balance),
        /// An fundraiser has been frozen.
        /// (primary issuance agent, fundraiser id)
        FundraiserFrozen(IdentityId, u64),
        /// An fundraiser has been unfrozen.
        /// (primary issuance agent, fundraiser id)
        FundraiserUnfrozen(IdentityId, u64),
        /// An fundraiser has been stopped.
        /// (primary issuance agent, fundraiser id)
        FundraiserClosed(IdentityId, u64),
    }
);

decl_error! {
    /// Errors for the Settlement module.
    pub enum Error for Module<T: Trait> {
        /// Sender does not have required permissions.
        Unauthorized,
        /// An arithmetic operation overflowed.
        Overflow,
        /// Not enough tokens left for sale.
        InsufficientTokensRemaining,
        /// Fundraiser not found.
        FundraiserNotFound,
        /// Fundraiser is either frozen or stopped.
        FundraiserNotLive,
        /// Fundraiser has been closed/stopped already.
        FundraiserClosed,
        /// Interacting with a fundraiser past the end `Moment`.
        FundraiserExpired,
        /// An invalid venue provided.
        InvalidVenue,
        /// An individual price tier was invalid or a set of price tiers was invalid.
        InvalidPriceTiers,
        /// Window (start time, end time) has invalid parameters, e.g start time is after end time.
        InvalidOfferingWindow,
        /// Price of the investment exceeded the max price.
        MaxPriceExceeded,
        /// Investment amount is lower than minimum investment amount.
        InvestmentAmountTooLow
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as StoCapped {
        /// All fundraisers that are currently running.
        /// (ticker, fundraiser_id) -> Fundraiser
        Fundraisers get(fn fundraisers): double_map hasher(blake2_128_concat) Ticker, hasher(twox_64_concat) u64 => Option<Fundraiser<T::Balance, T::Moment>>;
        /// Total fundraisers created for a token.
        FundraiserCount get(fn fundraiser_count): map hasher(blake2_128_concat) Ticker => u64;
        /// Name for the Fundraiser. It is only used offchain.
        /// (ticker, fundraiser_id) -> Fundraiser name
        FundraiserNames get(fn fundraiser_name): double_map hasher(blake2_128_concat) Ticker, hasher(twox_64_concat) u64 => FundraiserName;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: <T as frame_system::Trait>::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Create a new fundraiser.
        ///
        /// * `offering_portfolio` - Portfolio containing the `offering_asset`.
        /// * `offering_asset` - Asset being offered.
        /// * `raising_portfolio` - Portfolio containing the `raising_asset`.
        /// * `raising_asset` - Asset being exchanged for `offering_asset` on investment.
        /// * `tiers` - Price tiers to charge investors on investment.
        /// * `venue_id` - Venue to handle settlement.
        /// * `start` - Fundraiser start time, if `None` the fundraiser will start immediately.
        /// * `end` - Fundraiser end time, if `None` the fundraiser will never expire.
        /// * `minimum_investment` - Minimum amount of `raising_asset` that an investor needs to spend to invest in this raise.
        /// * `fundraiser_name` - Fundraiser name, only used in the UIs.
        ///
        /// # Weight
        /// `800_000_000` placeholder
        #[weight = <T as Trait>::WeightInfo::create_fundraiser(tiers.len() as u32)]
        pub fn create_fundraiser(
            origin,
            offering_portfolio: PortfolioId,
            offering_asset: Ticker,
            raising_portfolio: PortfolioId,
            raising_asset: Ticker,
            tiers: Vec<PriceTier<T::Balance>>,
            venue_id: u64,
            start: Option<T::Moment>,
            end: Option<T::Moment>,
            minimum_investment: T::Balance,
            fundraiser_name: FundraiserName
        ) {
            let (did, secondary_key) = Self::ensure_perms_pia(origin, &offering_asset)?;

            VenueInfo::get(venue_id)
                .filter(|v| v.creator == did && v.venue_type == VenueType::Sto)
                .ok_or(Error::<T>::InvalidVenue)?;

            <Portfolio<T>>::ensure_portfolio_custody_and_permission(raising_portfolio, did, secondary_key.as_ref())?;
            <Portfolio<T>>::ensure_portfolio_custody_and_permission(offering_portfolio, did, secondary_key.as_ref())?;

            ensure!(
                tiers.len() > 0 && tiers.len() <= MAX_TIERS && tiers.iter().all(|t| t.total > 0u32.into()),
                Error::<T>::InvalidPriceTiers
            );

            let offering_amount: T::Balance = tiers
                .iter()
                .map(|t| t.total)
                .try_fold(0u32.into(), |total: T::Balance, x| total.checked_add(&x))
                .ok_or(Error::<T>::InvalidPriceTiers)?;

            let start = start.unwrap_or_else(Timestamp::<T>::get);
            if let Some(end) = end {
                ensure!(start < end, Error::<T>::InvalidOfferingWindow);
            }

            <Portfolio<T>>::lock_tokens(&offering_portfolio, &offering_asset, &offering_amount)?;

            let fundraiser = Fundraiser {
                creator: did,
                offering_portfolio,
                offering_asset,
                raising_portfolio,
                raising_asset,
                tiers: tiers.into_iter().map(Into::into).collect(),
                venue_id,
                start,
                end,
                status: FundraiserStatus::Live,
                minimum_investment
            };

            let id = FundraiserCount::mutate(offering_asset, |id| mem::replace(id, *id + 1));
            <Fundraisers<T>>::insert(offering_asset, id, fundraiser.clone());
            FundraiserNames::insert(offering_asset, id, fundraiser_name.clone());

            Self::deposit_event(RawEvent::FundraiserCreated(did, id, fundraiser_name, fundraiser));
        }

        /// Invest in a fundraiser.
        ///
        /// * `investment_portfolio` - Portfolio that `offering_asset` will be deposited in.
        /// * `funding_portfolio` - Portfolio that will fund the investment.
        /// * `offering_asset` - Asset to invest in.
        /// * `fundraiser_id` - ID of the fundraiser to invest in.
        /// * `purchase_amount` - Amount of `offering_asset` to purchase.
        /// * `max_price` - Maximum price to pay per unit of `offering_asset`, If `None`there are no constraints on price.
        /// * `receipt` - Off-chain receipt to use instead of on-chain balance in `funding_portfolio`.
        ///
        /// # Weight
        /// `2_000_000_000` placeholder
        #[weight = <T as Trait>::WeightInfo::invest()]
        pub fn invest(
            origin,
            investment_portfolio: PortfolioId,
            funding_portfolio: PortfolioId,
            offering_asset: Ticker,
            fundraiser_id: u64,
            purchase_amount: T::Balance,
            max_price: Option<T::Balance>,
            receipt: Option<ReceiptDetails<T::AccountId, T::OffChainSignature>>
        ) {
            let PermissionedCallOriginData {
                primary_did: did,
                secondary_key,
                ..
            } = Identity::<T>::ensure_origin_call_permissions(origin.clone())?;

            <Portfolio<T>>::ensure_portfolio_custody_and_permission(investment_portfolio, did, secondary_key.as_ref())?;
            <Portfolio<T>>::ensure_portfolio_custody_and_permission(funding_portfolio, did, secondary_key.as_ref())?;

            let mut fundraiser = <Fundraisers<T>>::get(offering_asset, fundraiser_id).ok_or(Error::<T>::FundraiserNotFound)?;

            ensure!(fundraiser.status == FundraiserStatus::Live, Error::<T>::FundraiserNotLive);

            let now = Timestamp::<T>::get();
            ensure!(
                fundraiser.start <= now && fundraiser.end.filter(|e| now >= *e).is_none(),
                Error::<T>::FundraiserExpired
            );

            // Remaining tokens to fulfil the investment amount
            let mut remaining = purchase_amount;
            // Total cost to to fulfil the investment amount.
            // Primary use is to calculate the blended price (offering_token_amount / cost).
            // Blended price must be <= to max_price or the investment will fail.
            let mut cost = T::Balance::from(0u32);

            // Price is entered as a multiple of 1_000_000
            // i.e. a price of 1 unit is 1_000_000
            // a price of 1.5 units is 1_500_00
            let price_divisor = T::Balance::from(1_000_000u32);
            // Individual purchases from each tier that accumulate to fulfil the investment amount.
            // Tuple of (tier_id, amount to purchase from that tier).
            let mut purchases = Vec::new();

            for (id, tier) in fundraiser.tiers.iter().enumerate().filter(|(_, tier)| tier.remaining > 0u32.into()) {
                // fulfilled the investment amount
                if remaining == 0u32.into() {
                    break
                }

                // Check if this tier can fulfil the remaining investment amount.
                // If it can, purchase the remaining amount.
                // If it can't, purchase what's remaining in the tier.
                let purchase_amount = if tier.remaining >= remaining {
                    remaining
                } else {
                    tier.remaining
                };

                remaining -= purchase_amount;
                purchases.push((id, purchase_amount));
                cost = purchase_amount
                    .checked_mul(&tier.price)
                    .ok_or(Error::<T>::Overflow)?
                    .checked_div(&price_divisor)
                    .and_then(|pa| cost.checked_add(&pa))
                    .ok_or(Error::<T>::Overflow)?;
            }

            ensure!(remaining == 0u32.into(), Error::<T>::InsufficientTokensRemaining);
            ensure!(cost >= fundraiser.minimum_investment, Error::<T>::InvestmentAmountTooLow);
            ensure!(
                max_price.map(|max_price| cost <= max_price.saturating_mul(purchase_amount) / price_divisor).unwrap_or(true),
                Error::<T>::MaxPriceExceeded
            );

            let legs = vec![
                Leg {
                    from: fundraiser.offering_portfolio,
                    to: investment_portfolio,
                    asset: fundraiser.offering_asset,
                    amount: purchase_amount
                },
                Leg {
                    from: funding_portfolio,
                    to: fundraiser.raising_portfolio,
                    asset: fundraiser.raising_asset,
                    amount: cost
                }
            ];

            with_transaction(|| {
                <Portfolio<T>>::unlock_tokens(&fundraiser.offering_portfolio, &fundraiser.offering_asset, &purchase_amount)?;

                let instruction_id = Settlement::<T>::base_add_instruction(
                    fundraiser.creator,
                    fundraiser.venue_id,
                    SettlementType::SettleOnAffirmation,
                    None,
                    None,
                    legs
                )?;

                let portfolios = [fundraiser.offering_portfolio, fundraiser.raising_portfolio].iter().copied().collect::<BTreeSet<_>>();
                Settlement::<T>::unsafe_affirm_instruction(fundraiser.creator, instruction_id, portfolios, 1, None)?;

                let portfolios = vec![investment_portfolio, funding_portfolio];
                match receipt {
                    Some(receipt) => Settlement::<T>::affirm_with_receipts_and_execute_instruction(
                        origin,
                        instruction_id,
                        vec![receipt],
                        portfolios,
                        2
                    ),
                    None => Settlement::<T>::affirm_and_execute_instruction(origin, instruction_id, portfolios, 1),
                }
            })?;

            for (id, amount) in purchases {
                fundraiser.tiers[id].remaining -= amount;
            }

            Self::deposit_event(RawEvent::Invested(did, fundraiser_id, offering_asset, fundraiser.raising_asset, purchase_amount, cost));
            <Fundraisers<T>>::insert(offering_asset, fundraiser_id, fundraiser);
        }

        /// Freeze a fundraiser.
        ///
        /// * `offering_asset` - Asset to freeze.
        /// * `fundraiser_id` - ID of the fundraiser to freeze.
        ///
        /// # Weight
        /// `1_000` placeholder
        #[weight = <T as Trait>::WeightInfo::freeze_fundraiser()]
        pub fn freeze_fundraiser(origin, offering_asset: Ticker, fundraiser_id: u64) -> DispatchResult {
            Self::set_frozen(origin, offering_asset, fundraiser_id, true)
        }

        /// Unfreeze a fundraiser.
        ///
        /// * `offering_asset` - Asset to unfreeze.
        /// * `fundraiser_id` - ID of the fundraiser to unfreeze.
        ///
        /// # Weight
        /// `1_000` placeholder
        #[weight = <T as Trait>::WeightInfo::unfreeze_fundraiser()]
        pub fn unfreeze_fundraiser(origin, offering_asset: Ticker, fundraiser_id: u64) -> DispatchResult {
            Self::set_frozen(origin, offering_asset, fundraiser_id, false)
        }

        /// Modify the time window a fundraiser is active
        ///
        /// * `offering_asset` - Asset to modify.
        /// * `fundraiser_id` - ID of the fundraiser to modify.
        /// * `start` - New start of the fundraiser.
        /// * `end` - New end of the fundraiser to modify.
        ///
        /// # Weight
        /// `1_000` placeholder
        #[weight = <T as Trait>::WeightInfo::modify_fundraiser_window()]
        pub fn modify_fundraiser_window(origin, offering_asset: Ticker, fundraiser_id: u64, start: T::Moment, end: Option<T::Moment>) -> DispatchResult {
            Self::ensure_perms_pia(origin, &offering_asset)?;

            <Fundraisers<T>>::try_mutate(offering_asset, fundraiser_id, |fundraiser| {
                let fundraiser = fundraiser.as_mut().ok_or(Error::<T>::FundraiserNotFound)?;
                ensure!(fundraiser.status != FundraiserStatus::Closed, Error::<T>::FundraiserClosed);
                if let Some(end) = fundraiser.end {
                    ensure!(Timestamp::<T>::get() < end, Error::<T>::FundraiserExpired);
                }
                if let Some(end) = end {
                    ensure!(start < end, Error::<T>::InvalidOfferingWindow);
                }
                fundraiser.start = start;
                fundraiser.end = end;
                Ok(())
            })
        }

        /// Stop a fundraiser.
        ///
        /// * `offering_asset` - Asset to stop.
        /// * `fundraiser_id` - ID of the fundraiser to stop.
        ///
        /// # Weight
        /// `1_000` placeholder
        #[weight = <T as Trait>::WeightInfo::stop()]
        pub fn stop(origin, offering_asset: Ticker, fundraiser_id: u64) {
            let did = Self::ensure_perms(origin, &offering_asset)?.0;

            let mut fundraiser = <Fundraisers<T>>::get(offering_asset, fundraiser_id)
                .ok_or(Error::<T>::FundraiserNotFound)?;

            ensure!(
                <Asset<T>>::primary_issuance_agent_or_owner(&offering_asset) == did || fundraiser.creator == did,
                Error::<T>::Unauthorized
            );

            ensure!(fundraiser.status != FundraiserStatus::Closed, Error::<T>::FundraiserClosed);

            let remaining_amount: T::Balance = fundraiser.tiers
                .iter()
                .map(|t| t.remaining)
                .fold(0u32.into(), |remaining, x| remaining + x);

            <Portfolio<T>>::unlock_tokens(&fundraiser.offering_portfolio, &fundraiser.offering_asset, &remaining_amount)?;
            fundraiser.status = FundraiserStatus::Closed;
            <Fundraisers<T>>::insert(offering_asset, fundraiser_id, fundraiser);
        }
    }
}

impl<T: Trait> Module<T> {
    fn set_frozen(
        origin: <T as frame_system::Trait>::Origin,
        offering_asset: Ticker,
        fundraiser_id: u64,
        frozen: bool,
    ) -> DispatchResult {
        let did = Self::ensure_perms_pia(origin, &offering_asset)?.0;
        let mut fundraiser = <Fundraisers<T>>::get(offering_asset, fundraiser_id)
            .ok_or(Error::<T>::FundraiserNotFound)?;
        ensure!(
            fundraiser.status != FundraiserStatus::Closed,
            Error::<T>::FundraiserClosed
        );
        if frozen {
            fundraiser.status = FundraiserStatus::Frozen;
            Self::deposit_event(RawEvent::FundraiserFrozen(did, fundraiser_id));
        } else {
            fundraiser.status = FundraiserStatus::Live;
            Self::deposit_event(RawEvent::FundraiserUnfrozen(did, fundraiser_id));
        }
        <Fundraisers<T>>::insert(offering_asset, fundraiser_id, fundraiser);
        Ok(())
    }

    /// Ensure that `origin` is permissioned, returning its DID.
    fn ensure_perms(
        origin: <T as frame_system::Trait>::Origin,
        asset: &Ticker,
    ) -> Result<(IdentityId, Option<SecondaryKey<T::AccountId>>), DispatchError> {
        let PermissionedCallOriginData {
            primary_did,
            secondary_key,
            ..
        } = Identity::<T>::ensure_origin_call_permissions(origin)?;
        <Asset<T>>::ensure_asset_perms(secondary_key.as_ref(), asset)?;
        Ok((primary_did, secondary_key))
    }

    /// Ensure that `origin` is permissioned and the PIA, returning its DID.
    fn ensure_perms_pia(
        origin: <T as frame_system::Trait>::Origin,
        asset: &Ticker,
    ) -> Result<(IdentityId, Option<SecondaryKey<T::AccountId>>), DispatchError> {
        let (primary_did, secondary_key) = Self::ensure_perms(origin, asset)?;
        ensure!(
            <Asset<T>>::primary_issuance_agent_or_owner(asset) == primary_did,
            Error::<T>::Unauthorized
        );
        Ok((primary_did, secondary_key))
    }
}
