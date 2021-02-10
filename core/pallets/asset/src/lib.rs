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

//! # Asset Module
//!
//! The Asset module is one place to create the security tokens on the Polymesh blockchain.
//! It consist every required functionality related to securityToken and every function
//! execution can be differentiate at the token level by providing the ticker of the token.
//! In Ethereum analogy every token has different smart contract address which act as the unique identity
//! of the token while here token lives at low-level where token ticker act as the differentiator.
//!
//! ## Overview
//!
//! The Asset module provides functions for:
//!
//! - Creating the tokens.
//! - Creation of checkpoints on the token level.
//! - Management of the token (Document mgt etc).
//! - Transfer/redeem functionality of the token.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `register_ticker` - Used to either register a new ticker or extend registration of an existing ticker.
//! - `accept_ticker_transfer` - Used to accept a ticker transfer authorization.
//! - `accept_asset_ownership_transfer` - Used to accept the token transfer authorization.
//! - `create_asset` - Initializes a new security token.
//! - `freeze` - Freezes transfers and minting of a given token.
//! - `unfreeze` - Unfreezes transfers and minting of a given token.
//! - `rename_asset` - Renames a given asset.
//! - `controller_transfer` - Forces a transfer between two DID.
//! - `create_checkpoint` - Function used to create the checkpoint.
//! - `create_checkpoint_schedule` - Creates a checkpoint schedule.
//! - `remove_checkpoint_schedule` - Removes a checkpoint schedule.
//! - `issue` - Function is used to issue(or mint) new tokens to the primary issuance agent.
//! - `redeem` - Redeems tokens from PIA's (Primary Issuance Agent) default portfolio.
//! - `make_divisible` - Change the divisibility of the token to divisible. Only called by the token owner.
//! - `can_transfer` - Checks whether a transaction with given parameters can take place or not.
//! - `add_documents` - Add documents for a given token, Only be called by the token owner.
//! - `remove_documents` - Remove documents for a given token, Only be called by the token owner.
//! - `set_funding_round` - Sets the name of the current funding round.
//! - `update_identifiers` - Updates the asset identifiers. Only called by the token owner.
//! - `add_extension` - It is used to permission the Smart-Extension address for a given ticker.
//! - `archive_extension` - Extension gets archived it means extension is no more use to verify the compliance or any smart logic it posses.
//! - `unarchive_extension` - Extension gets un-archived it means extension is use to verify the compliance or any smart logic it posses.
//!
//! ### Public Functions
//!
//! - `ticker_registration` - Provide ticker registration details.
//! - `ticker_registration_config` - Provide the ticker registration configuration details.
//! - `token_details` - Returns details of the token.
//! - `balance_of` - Returns the balance of the DID corresponds to the ticker.
//! - `identifiers` - It provides the identifiers for a given ticker.
//! - `total_checkpoints_of` - Returns the checkpoint Id.
//! - `total_supply_at` - Returns the total supply at a given checkpoint.
//! - `extension_details` - It provides the list of Smart extension added for the given tokens.
//! - `extensions` - It provides the list of Smart extension added for the given tokens and for the given type.
//! - `frozen` - It tells whether the given ticker is frozen or not.
//! - `is_ticker_available` - It checks whether the given ticker is available or not.
//! - `is_ticker_registry_valid` - It checks whether the ticker is own by a given IdentityId or not.
//! - `is_ticker_available_or_registered_to` - It provides the status of a given ticker.
//! - `total_supply` - It provides the total supply of a ticker.
//! - `get_balance_at` - It provides the balance of a DID at a certain checkpoint.
//! - `verify_restriction` - It is use to verify the restriction implied by the smart extension and the Compliance Manager.
//! - `call_extension` - A helper function that is used to call the smart extension function.

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]
#![feature(bool_to_option, or_patterns, const_option)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
pub mod checkpoint;
pub mod ethereum;

use arrayvec::ArrayVec;
use codec::{Decode, Encode};
use core::mem;
use core::result::Result as StdResult;
use currency::*;
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure,
    traits::{Currency, Get, UnixTime},
    weights::Weight,
};
use frame_system::ensure_root;
use hex_literal::hex;
use pallet_contracts::{ExecResult, Gas};
use pallet_identity::{self as identity, PermissionedCallOriginData};
use pallet_statistics::Counter;
use polymesh_common_utilities::{
    asset::{AssetFnTrait, AssetSubTrait},
    balances::Trait as BalancesTrait,
    compliance_manager::Trait as ComplianceManagerTrait,
    constants::*,
    protocol_fee::{ChargeProtocolFee, ProtocolOp},
    with_transaction, CommonTrait, Context, SystematicIssuers,
};
use polymesh_primitives::{
    asset::{AssetName, AssetType, FundingRoundName},
    calendar::CheckpointId,
    migrate::MigrationError,
    storage_migrate_on, storage_migration_ver, AssetIdentifier, AuthorizationData, Document,
    DocumentId, IdentityId, MetaVersion as ExtVersion, PortfolioId, ScopeId, SecondaryKey,
    Signatory, SmartExtension, SmartExtensionName, SmartExtensionType, Ticker,
};
use sp_runtime::traits::{CheckedAdd, Saturating, Zero};
#[cfg(feature = "std")]
use sp_runtime::{Deserialize, Serialize};
use sp_std::{convert::TryFrom, prelude::*};

type Portfolio<T> = pallet_portfolio::Module<T>;
type Statistics<T> = pallet_statistics::Module<T>;
type Checkpoint<T> = checkpoint::Module<T>;

pub trait WeightInfo {
    fn register_ticker() -> Weight;
    fn accept_ticker_transfer() -> Weight;
    fn accept_asset_ownership_transfer() -> Weight;
    fn create_asset(n: u32, i: u32, f: u32) -> Weight;
    fn freeze() -> Weight;
    fn unfreeze() -> Weight;
    fn rename_asset(n: u32) -> Weight;
    fn issue() -> Weight;
    fn redeem() -> Weight;
    fn make_divisible() -> Weight;
    fn add_documents(d: u32) -> Weight;
    fn remove_documents(d: u32) -> Weight;
    fn set_funding_round(f: u32) -> Weight;
    fn update_identifiers(i: u32) -> Weight;
    fn remove_primary_issuance_agent() -> Weight;
    fn claim_classic_ticker() -> Weight;
    fn reserve_classic_ticker() -> Weight;
    fn add_extension() -> Weight;
    fn remove_smart_extension() -> Weight;
    fn archive_extension() -> Weight;
    fn unarchive_extension() -> Weight;
    fn accept_primary_issuance_agent_transfer() -> Weight;
    fn controller_transfer() -> Weight;
}

/// The module's configuration trait.
pub trait Trait:
    BalancesTrait
    + pallet_session::Trait
    + pallet_statistics::Trait
    + polymesh_contracts::Trait
    + pallet_portfolio::Trait
{
    /// The overarching event type.
    type Event: From<Event<Self>>
        + From<checkpoint::Event<Self>>
        + Into<<Self as frame_system::Trait>::Event>;

    type Currency: Currency<Self::AccountId>;

    type ComplianceManager: ComplianceManagerTrait<Self::Balance>;

    /// Maximum number of smart extensions can attach to a asset.
    /// This hard limit is set to avoid the cases where a asset transfer
    /// gas usage go beyond the block gas limit.
    type MaxNumberOfTMExtensionForAsset: Get<u32>;

    /// Time used in computation of checkpoints.
    type UnixTime: UnixTime;

    /// Max length for the name of an asset.
    type AssetNameMaxLength: Get<usize>;

    /// Max length of the funding round name.
    type FundingRoundNameMaxLength: Get<usize>;

    type AssetFn: AssetFnTrait<Self::Balance, Self::AccountId, Self::Origin>;

    /// Maximum gas used for smart extension execution.
    type AllowedGasLimit: Get<u64>;

    type WeightInfo: WeightInfo;
    type CPWeightInfo: checkpoint::WeightInfo;
}

/// Ownership status of a ticker/token.
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetOwnershipRelation {
    NotOwned,
    TickerOwned,
    AssetOwned,
}

impl Default for AssetOwnershipRelation {
    fn default() -> Self {
        Self::NotOwned
    }
}

/// struct to store the token details.
#[derive(Encode, Decode, Default, Clone, PartialEq, Debug)]
pub struct SecurityToken<U> {
    pub name: AssetName,
    pub total_supply: U,
    pub owner_did: IdentityId,
    pub divisible: bool,
    pub asset_type: AssetType,
    pub primary_issuance_agent: Option<IdentityId>,
}

/// struct to store the ticker registration details.
#[derive(Encode, Decode, Clone, Default, PartialEq, Debug)]
pub struct TickerRegistration<U> {
    pub owner: IdentityId,
    pub expiry: Option<U>,
}

/// struct to store the ticker registration config.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Encode, Decode, Clone, Default, PartialEq, Debug)]
pub struct TickerRegistrationConfig<U> {
    pub max_ticker_length: u8,
    pub registration_length: Option<U>,
}

/// Enum that represents the current status of a ticker.
#[derive(Encode, Decode, Clone, Eq, PartialEq, Debug)]
pub enum TickerRegistrationStatus {
    RegisteredByOther,
    Available,
    RegisteredByDid,
}

/// Enum that uses as the return type for the restriction verification.
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RestrictionResult {
    Valid,
    Invalid,
    ForceValid,
}

impl Default for RestrictionResult {
    fn default() -> Self {
        RestrictionResult::Invalid
    }
}

/// Data imported from Polymath Classic regarding ticker registration/creation.
/// Only used at genesis config and not stored on-chain.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Encode, Decode, Copy, Clone, Debug, PartialEq, Eq)]
pub struct ClassicTickerImport {
    /// Owner of the registration.
    pub eth_owner: ethereum::EthereumAddress,
    /// Name of the ticker registered.
    pub ticker: Ticker,
    /// Is `eth_owner` an Ethereum contract (e.g., in case of a multisig)?
    pub is_contract: bool,
    /// Has the ticker been elevated to a created asset on classic?
    pub is_created: bool,
}

/// Data about a ticker registration from Polymath Classic on-genesis importation.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq)]
pub struct ClassicTickerRegistration {
    /// Owner of the registration.
    pub eth_owner: ethereum::EthereumAddress,
    /// Has the ticker been elevated to a created asset on classic?
    pub is_created: bool,
}

// A value placed in storage that represents the current version of the this storage. This value
// is used by the `on_runtime_upgrade` logic to determine whether we run storage migration logic.
storage_migration_ver!(2);

decl_storage! {
    trait Store for Module<T: Trait> as Asset {
        /// Ticker registration details.
        /// (ticker) -> TickerRegistration
        pub Tickers get(fn ticker_registration): map hasher(blake2_128_concat) Ticker => TickerRegistration<T::Moment>;
        /// Ticker registration config.
        /// (ticker) -> TickerRegistrationConfig
        pub TickerConfig get(fn ticker_registration_config) config(): TickerRegistrationConfig<T::Moment>;
        /// Details of the token corresponding to the token ticker.
        /// (ticker) -> SecurityToken details [returns SecurityToken struct]
        pub Tokens get(fn token_details): map hasher(blake2_128_concat) Ticker => SecurityToken<T::Balance>;
        /// The total asset ticker balance per identity.
        /// (ticker, DID) -> Balance
        // NB: It is safe to use `identity` hasher here because assets can not be distributed to non-existent identities.
        pub BalanceOf get(fn balance_of): double_map hasher(blake2_128_concat) Ticker, hasher(identity) IdentityId => T::Balance;
        /// A map of a ticker name and asset identifiers.
        pub Identifiers get(fn identifiers): map hasher(blake2_128_concat) Ticker => Vec<AssetIdentifier>;

        /// The name of the current funding round.
        /// ticker -> funding round
        FundingRound get(fn funding_round): map hasher(blake2_128_concat) Ticker => FundingRoundName;
        /// The total balances of tokens issued in all recorded funding rounds.
        /// (ticker, funding round) -> balance
        IssuedInFundingRound get(fn issued_in_funding_round): map hasher(blake2_128_concat) (Ticker, FundingRoundName) => T::Balance;
        /// List of Smart extension added for the given tokens.
        /// ticker, AccountId (SE address) -> SmartExtension detail
        pub ExtensionDetails get(fn extension_details): map hasher(blake2_128_concat) (Ticker, T::AccountId) => SmartExtension<T::AccountId>;
        /// List of Smart extension added for the given tokens and for the given type.
        /// ticker, type of SE -> address/AccountId of SE
        pub Extensions get(fn extensions): map hasher(blake2_128_concat) (Ticker, SmartExtensionType) => Vec<T::AccountId>;
        /// The set of frozen assets implemented as a membership map.
        /// ticker -> bool
        pub Frozen get(fn frozen): map hasher(blake2_128_concat) Ticker => bool;
        /// Tickers and token owned by a user
        /// (user, ticker) -> AssetOwnership
        pub AssetOwnershipRelations get(fn asset_ownership_relation):
            double_map hasher(identity) IdentityId, hasher(blake2_128_concat) Ticker => AssetOwnershipRelation;
        /// Documents attached to an Asset
        /// (ticker, doc_id) -> document
        pub AssetDocuments get(fn asset_documents):
            double_map hasher(blake2_128_concat) Ticker, hasher(twox_64_concat) DocumentId => Document;
        /// Per-ticker document ID counter.
        /// (ticker) -> doc_id
        pub AssetDocumentsIdSequence get(fn asset_documents_id_sequence): map hasher(blake2_128_concat) Ticker => DocumentId;
        /// Ticker registration details on Polymath Classic / Ethereum.
        pub ClassicTickers get(fn classic_ticker_registration): map hasher(blake2_128_concat) Ticker => Option<ClassicTickerRegistration>;
        /// Supported extension version.
        pub CompatibleSmartExtVersion get(fn compatible_extension_version): map hasher(twox_64_concat) SmartExtensionType => ExtVersion;
        /// Balances get stored on the basis of the `ScopeId`.
        /// Right now it is only helpful for the UI purposes but in future it can be used to do miracles on-chain.
        /// (ScopeId, IdentityId) => Balance.
        pub BalanceOfAtScope get(fn balance_of_at_scope): double_map hasher(identity) ScopeId, hasher(identity) IdentityId => T::Balance;
        /// Store aggregate balance of those identities that has the same `ScopeId`.
        /// (Ticker, ScopeId) => Balance.
        pub AggregateBalance get(fn aggregate_balance_of): double_map hasher(blake2_128_concat) Ticker, hasher(identity) ScopeId => T::Balance;
        /// Tracks the ScopeId of the identity for a given ticker.
        /// (Ticker, IdentityId) => ScopeId.
        pub ScopeIdOf get(fn scope_id_of): double_map hasher(blake2_128_concat) Ticker, hasher(identity) IdentityId => ScopeId;
        /// Storage version.
        StorageVersion get(fn storage_version) build(|_| Version::new(2).unwrap()): Version;
    }
    add_extra_genesis {
        config(classic_migration_tickers): Vec<ClassicTickerImport>;
        config(classic_migration_tconfig): TickerRegistrationConfig<T::Moment>;
        config(classic_migration_contract_did): IdentityId;
        config(reserved_country_currency_codes): Vec<Ticker>;
        /// Smart Extension supported version at genesis.
        config(versions): Vec<(SmartExtensionType, ExtVersion)>;
        build(|config: &GenesisConfig<T>| {
            use frame_system::RawOrigin;

            for &import in &config.classic_migration_tickers {
                <Module<T>>::reserve_classic_ticker(
                    RawOrigin::Root.into(),
                    import,
                    config.classic_migration_contract_did,
                    config.classic_migration_tconfig.clone()
                ).expect("`reserve_classic_ticker` failed on genesis");
            }

            // Reserving country currency logic
            let fiat_tickers_reservation_did = SystematicIssuers::FiatTickersReservation.as_id();
            for currency_ticker in &config.reserved_country_currency_codes {
                <Module<T>>::_register_ticker(&currency_ticker, fiat_tickers_reservation_did, None);
            }
            config.versions
                .iter()
                .filter(|(t, _)| !<CompatibleSmartExtVersion>::contains_key(&t))
                .for_each(|(se_type, ver)| {
                    CompatibleSmartExtVersion::insert(se_type, ver);
            });

        });
    }
}

type Identity<T> = identity::Module<T>;

/// Errors of migration on this pallets.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub enum AssetMigrationError {
    /// Migration of document fails on the given ticker and document id.
    AssetDocumentFail(Ticker, DocumentId),
}

// Public interface for this runtime module.
decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {

        type Error = Error<T>;

        const AllowedGasLimit: u64 = T::AllowedGasLimit::get();

        /// initialize the default event for this module
        fn deposit_event() = default;

        fn on_runtime_upgrade() -> frame_support::weights::Weight {

            // Migrate `AssetDocuments`.
            use frame_support::{Blake2_128Concat, Twox64Concat};
            use polymesh_primitives::{ migrate::{migrate_double_map_only_values, Migrate, Empty}, document::DocumentOld};

            let storage_ver = StorageVersion::get();
            storage_migrate_on!(storage_ver, 2, {
                migrate_double_map_only_values::<_, _, Blake2_128Concat, _, Twox64Concat, _, _, _>(
                    b"Asset", b"AssetDocuments",
                    |t: Ticker, id: DocumentId, doc: DocumentOld|
                        doc.migrate(Empty).ok_or_else(|| AssetMigrationError::AssetDocumentFail(t, id)))
                .for_each(|doc_migrate_status| {
                    if let Err(migrate_err) = doc_migrate_status {
                        Self::deposit_event( RawEvent::MigrationFailure(migrate_err));
                    }
                })
            });

            1_000
        }

        /// This function is used to either register a new ticker or extend validity of an existing ticker.
        /// NB: Ticker validity does not get carry forward when renewing ticker.
        ///
        /// # Arguments
        /// * `origin` It contains the secondary key of the caller (i.e who signed the transaction to execute this function).
        /// * `ticker` ticker to register.
        #[weight = <T as Trait>::WeightInfo::register_ticker()]
        pub fn register_ticker(origin, ticker: Ticker) {
            let to_did = Identity::<T>::ensure_perms(origin)?;
            let expiry = Self::ticker_registration_checks(&ticker, to_did, false, || Self::ticker_registration_config())?;
            T::ProtocolFee::charge_fee(ProtocolOp::AssetRegisterTicker)?;
            Self::_register_ticker(&ticker, to_did, expiry);
        }

        /// This function is used to accept a ticker transfer.
        /// NB: To reject the transfer, call remove auth function in identity module.
        ///
        /// # Arguments
        /// * `origin` It contains the secondary key of the caller (i.e who signed the transaction to execute this function).
        /// * `auth_id` Authorization ID of ticker transfer authorization.
        #[weight = <T as Trait>::WeightInfo::accept_ticker_transfer()]
        pub fn accept_ticker_transfer(origin, auth_id: u64) -> DispatchResult {
            let to_did = Identity::<T>::ensure_perms(origin)?;
            Self::_accept_ticker_transfer(to_did, auth_id)
        }

        /// This function is used to accept a primary issuance agent transfer.
        /// NB: To reject the transfer, call remove auth function in identity module.
        ///
        /// # Arguments
        /// * `origin` It contains the signing key of the caller (i.e who signed the transaction to execute this function).
        /// * `auth_id` Authorization ID of primary issuance agent transfer authorization.
        #[weight = <T as Trait>::WeightInfo::accept_primary_issuance_agent_transfer()]
        pub fn accept_primary_issuance_agent_transfer(origin, auth_id: u64) -> DispatchResult {
            let to_did = Identity::<T>::ensure_perms(origin)?;
            Self::_accept_primary_issuance_agent_transfer(to_did, auth_id)
        }

        /// This function is used to accept a token ownership transfer.
        /// NB: To reject the transfer, call remove auth function in identity module.
        ///
        /// # Arguments
        /// * `origin` It contains the secondary key of the caller (i.e who signed the transaction to execute this function).
        /// * `auth_id` Authorization ID of the token ownership transfer authorization.
        #[weight = <T as Trait>::WeightInfo::accept_asset_ownership_transfer()]
        pub fn accept_asset_ownership_transfer(origin, auth_id: u64) -> DispatchResult {
            let to_did = Identity::<T>::ensure_perms(origin)?;
            Self::_accept_token_ownership_transfer(to_did, auth_id)
        }

        /// Initializes a new security token
        /// makes the initiating account the owner of the security token
        /// & the balance of the owner is set to total supply.
        ///
        /// # Arguments
        /// * `origin` - contains the secondary key of the caller (i.e who signed the transaction to execute this function).
        /// * `name` - the name of the token.
        /// * `ticker` - the ticker symbol of the token.
        /// * `total_supply` - the total supply of the token.
        /// * `divisible` - a boolean to identify the divisibility status of the token.
        /// * `asset_type` - the asset type.
        /// * `identifiers` - a vector of asset identifiers.
        /// * `funding_round` - name of the funding round.
        ///
        /// # Weight
        /// `3_000_000_000 + 20_000 * identifiers.len()`
        #[weight = <T as Trait>::WeightInfo::create_asset(
            name.len() as u32,
            identifiers.len() as u32,
            funding_round.as_ref().map_or(0, |name| name.len()) as u32)]
        pub fn create_asset(
            origin,
            name: AssetName,
            ticker: Ticker,
            total_supply: T::Balance,
            divisible: bool,
            asset_type: AssetType,
            identifiers: Vec<AssetIdentifier>,
            funding_round: Option<FundingRoundName>,
        ) {
            ensure!(name.len() <= T::AssetNameMaxLength::get(), Error::<T>::MaxLengthOfAssetNameExceeded);
            ensure!(funding_round.as_ref().map_or(0, |name| name.len()) <= T::FundingRoundNameMaxLength::get(), Error::<T>::FundingRoundNameMaxLengthExceeded);

            let PermissionedCallOriginData {
                sender,
                primary_did: did,
                secondary_key
            } = Identity::<T>::ensure_origin_call_permissions(origin)?;

            // Check total supply here to avoid any later failure
            Self::ensure_create_asset_parameters(&ticker, total_supply)?;
            ensure!(divisible || Self::is_unit_multiple(total_supply), Error::<T>::InvalidTotalSupply);

            // Ensure its registered by DID or at least expired, thus available.
            let available = match Self::is_ticker_available_or_registered_to(&ticker, did) {
                TickerRegistrationStatus::RegisteredByOther => return Err(Error::<T>::TickerAlreadyRegistered.into()),
                TickerRegistrationStatus::RegisteredByDid => false,
                TickerRegistrationStatus::Available => true,
            };

            // If `ticker` isn't registered, it will be, so ensure it is fully ascii.
            if available {
                Self::ensure_ticker_ascii(&ticker)?;
            }

            let token_did = Identity::<T>::get_token_did(&ticker)?;
            // Ensure there's no pre-existing entry for the DID.
            // This should never happen, but let's be defensive here.
            Identity::<T>::ensure_no_id_record(token_did)?;

            // Ensure that the caller has relevant portfolio permissions
            let user_default_portfolio = PortfolioId::default_portfolio(did);
            Portfolio::<T>::ensure_portfolio_custody_and_permission(user_default_portfolio, did, secondary_key.as_ref())?;

            // Charge protocol fees.
            T::ProtocolFee::charge_fees(&{
                let mut fees = ArrayVec::<[_; 2]>::new();
                if available {
                    fees.push(ProtocolOp::AssetRegisterTicker);
                }
                // Waive the asset fee iff classic ticker hasn't expired,
                // and it was already created on classic.
                if available || ClassicTickers::get(&ticker).filter(|r| r.is_created).is_none() {
                    fees.push(ProtocolOp::AssetCreateAsset);
                }
                fees
            })?;

            //==========================================================================
            // At this point all checks have been made; **only** storage changes follow!
            //==========================================================================

            Identity::<T>::commit_token_did(token_did, ticker);

            // Register the ticker or finish its registration.
            if available {
                // Ticker not registered by anyone (or registry expired), so register.
                Self::_register_ticker(&ticker, did, None);
            } else {
                // Ticker already registered by the user.
                <Tickers<T>>::mutate(&ticker, |tr| tr.expiry = None);
            }

            let token = SecurityToken {
                name,
                total_supply: Zero::zero(),
                owner_did: did,
                divisible,
                asset_type: asset_type.clone(),
                primary_issuance_agent: None,
            };
            <Tokens<T>>::insert(&ticker, token);
            // NB - At the time of asset creation it is obvious that asset issuer/ primary issuance agent will not have
            // `InvestorUniqueness` claim. So we are skipping the scope claim based stats update as
            // those data points will get added in to the system whenever asset issuer/ primary issuance agent
            // have InvestorUniqueness claim. This also applies when issuing assets.
            <AssetOwnershipRelations>::insert(did, ticker, AssetOwnershipRelation::AssetOwned);
            Self::deposit_event(RawEvent::AssetCreated(
                did,
                ticker,
                total_supply,
                divisible,
                asset_type,
                did,
            ));

            let identifiers: Vec<AssetIdentifier> = identifiers
                .into_iter()
                .filter_map(|identifier| identifier.validate())
                .collect();

            Identifiers::insert(ticker, identifiers.clone());

            // Add funding round name.
            FundingRound::insert(ticker, funding_round.unwrap_or_default());

            Self::deposit_event(RawEvent::IdentifiersUpdated(did, ticker, identifiers));

            // Mint total supply to PIA
            if total_supply > Zero::zero() {
                Self::_mint(&ticker, sender, did, total_supply, None)?;
            }
        }

        /// Freezes transfers and minting of a given token.
        ///
        /// # Arguments
        /// * `origin` - the secondary key of the sender.
        /// * `ticker` - the ticker of the token.
        #[weight = <T as Trait>::WeightInfo::freeze()]
        pub fn freeze(origin, ticker: Ticker) {
            // Verify the ownership of the token
            let sender_did = Self::ensure_perms_owner_asset(origin, &ticker)?;
            Self::ensure_asset_exists(&ticker)?;
            ensure!(!Self::frozen(&ticker), Error::<T>::AlreadyFrozen);
            Frozen::insert(&ticker, true);
            Self::deposit_event(RawEvent::AssetFrozen(sender_did, ticker));
        }

        /// Unfreezes transfers and minting of a given token.
        ///
        /// # Arguments
        /// * `origin` - the secondary key of the sender.
        /// * `ticker` - the ticker of the frozen token.
        #[weight = <T as Trait>::WeightInfo::unfreeze()]
        pub fn unfreeze(origin, ticker: Ticker) {
            // Verify the ownership of the token
            let sender_did = Self::ensure_perms_owner_asset(origin, &ticker)?;
            Self::ensure_asset_exists(&ticker)?;
            ensure!(Self::frozen(&ticker), Error::<T>::NotFrozen);
            Frozen::insert(&ticker, false);
            Self::deposit_event(RawEvent::AssetUnfrozen(sender_did, ticker));
        }

        /// Renames a given token.
        ///
        /// # Arguments
        /// * `origin` - the secondary key of the sender.
        /// * `ticker` - the ticker of the token.
        /// * `name` - the new name of the token.
        #[weight = <T as Trait>::WeightInfo::rename_asset(ticker.len() as u32)]
        pub fn rename_asset(origin, ticker: Ticker, name: AssetName) {
            ensure!(name.len() <= T::AssetNameMaxLength::get(), Error::<T>::MaxLengthOfAssetNameExceeded);

            // Verify the ownership of the token
            let sender_did = Self::ensure_perms_owner_asset(origin, &ticker)?;
            Self::ensure_asset_exists(&ticker)?;
            <Tokens<T>>::mutate(&ticker, |token| token.name = name.clone());
            Self::deposit_event(RawEvent::AssetRenamed(sender_did, ticker, name));
        }

        /// Function is used to issue(or mint) new tokens to the primary issuance agent.
        /// It can be executed by the token owner or the PIA.
        ///
        /// # Arguments
        /// * `origin` Secondary key of token owner.
        /// * `ticker` Ticker of the token.
        /// * `value` Amount of tokens that get issued.
        #[weight = <T as Trait>::WeightInfo::issue()]
        pub fn issue(origin, ticker: Ticker, value: T::Balance) -> DispatchResult {
            // Ensure origin is PIA with custody and permissions for default portfolio.
            let PermissionedCallOriginData {
                sender,
                primary_did,
                ..
            } = Self::ensure_pia_with_custody_and_permissions(origin, ticker)?;

            Self::_mint(&ticker, sender, primary_did, value, Some(ProtocolOp::AssetIssue))
        }

        /// Redeems existing tokens by reducing the balance of the PIA's default portfolio and the total supply of the token
        ///
        /// # Arguments
        /// * `origin` Secondary key of token owner.
        /// * `ticker` Ticker of the token.
        /// * `value` Amount of tokens to redeem.
        ///
        /// # Errors
        /// - `Unauthorized` If called by someone other than the token owner or the PIA
        /// - `InvalidGranularity` If the amount is not divisible by 10^6 for non-divisible tokens
        /// - `InsufficientPortfolioBalance` If the PIA's default portfolio doesn't have enough free balance
        #[weight = <T as Trait>::WeightInfo::redeem()]
        pub fn redeem(origin, ticker: Ticker, value: T::Balance) {
            // Ensure origin is PIA with custody and permissions for default portfolio.
            let pia = Self::ensure_pia_with_custody_and_permissions(origin, ticker)?.primary_did;

            Self::ensure_granular(&ticker, value)?;

            // Reduce PIA's portfolio balance. This makes sure that the PIA has enough unlocked tokens.
            // If `advance_update_balances` fails, `reduce_portfolio_balance` shouldn't modify storage.
            let pia_portfolio = PortfolioId::default_portfolio(pia);
            with_transaction(|| {
                Portfolio::<T>::reduce_portfolio_balance(&pia_portfolio, &ticker, &value)?;

                <Checkpoint<T>>::advance_update_balances(&ticker, &[(pia, Self::balance_of(ticker, pia))])
            })?;

            let updated_balance = Self::balance_of(ticker, pia) - value;

            // Update identity balances and total supply
            <BalanceOf<T>>::insert(ticker, &pia, updated_balance);
            <Tokens<T>>::mutate(ticker, |token| token.total_supply -= value);

            // Update scope balances
            let scope_id = Self::scope_id_of(ticker, &pia);
            Self::update_scope_balance(&ticker, value, scope_id, pia, updated_balance, true);

            // Update statistic info.
            // Using the aggregate balance to update the unique investor count.
            Statistics::<T>::update_transfer_stats(
                &ticker,
                Some(Self::aggregate_balance_of(ticker, &scope_id)),
                None,
                value,
            );

            Self::deposit_event(RawEvent::Transfer(
                pia,
                ticker,
                pia_portfolio,
                PortfolioId::default(),
                value
            ));
            Self::deposit_event(RawEvent::Redeemed(
                pia,
                ticker,
                pia,
                value
            ));
        }

        /// Makes an indivisible token divisible. Only called by the token owner.
        ///
        /// # Arguments
        /// * `origin` Secondary key of the token owner.
        /// * `ticker` Ticker of the token.
        #[weight = <T as Trait>::WeightInfo::make_divisible()]
        pub fn make_divisible(origin, ticker: Ticker) {
            let did = Self::ensure_perms_owner_asset(origin, &ticker)?;
            // Read the token details
            let mut token = Self::token_details(&ticker);
            ensure!(!token.divisible, Error::<T>::AssetAlreadyDivisible);
            token.divisible = true;
            <Tokens<T>>::insert(&ticker, token);
            Self::deposit_event(RawEvent::DivisibilityChanged(did, ticker, true));
        }

        /// Add documents for a given token. To be called only by the token owner.
        ///
        /// # Arguments
        /// * `origin` Secondary key of the token owner.
        /// * `ticker` Ticker of the token.
        /// * `docs` Documents to be attached to `ticker`.
        ///
        /// # Weight
        /// `500_000_000 + 600_000 * docs.len()`
        #[weight = <T as Trait>::WeightInfo::add_documents(docs.len() as u32)]
        pub fn add_documents(origin, docs: Vec<Document>, ticker: Ticker) {
            let did = Self::ensure_perms_owner_asset(origin, &ticker)?;
            let len = docs.len();
            T::ProtocolFee::batch_charge_fee(ProtocolOp::AssetAddDocument, len)?;

            AssetDocumentsIdSequence::mutate(ticker, |DocumentId(ref mut id)| {
                for (id, doc) in (*id..).map(DocumentId).zip(docs) {
                    AssetDocuments::insert(ticker, id, doc.clone());
                    Self::deposit_event(RawEvent::DocumentAdded(did, ticker, id, doc));
                }
                *id += len as u32;
            });
        }

        /// Remove documents for a given token. To be called only by the token owner.
        ///
        /// # Arguments
        /// * `origin` Secondary key of the token owner.
        /// * `ticker` Ticker of the token.
        /// * `ids` Documents ids to be removed from `ticker`.
        ///
        /// # Weight
        /// `500_000_000 + 600_000 * ids.len()`
        #[weight = <T as Trait>::WeightInfo::remove_documents(ids.len() as u32)]
        pub fn remove_documents(origin, ids: Vec<DocumentId>, ticker: Ticker) {
            let did = Self::ensure_perms_owner_asset(origin, &ticker)?;
            for id in ids {
                AssetDocuments::remove(ticker, id);
                Self::deposit_event(RawEvent::DocumentRemoved(did, ticker, id));
            }
        }

        /// Sets the name of the current funding round.
        ///
        /// # Arguments
        /// * `origin` - the secondary key of the token owner DID.
        /// * `ticker` - the ticker of the token.
        /// * `name` - the desired name of the current funding round.
        #[weight = <T as Trait>::WeightInfo::set_funding_round( name.len() as u32 )]
        pub fn set_funding_round(origin, ticker: Ticker, name: FundingRoundName) {
            ensure!(name.len() <= T::FundingRoundNameMaxLength::get(), Error::<T>::FundingRoundNameMaxLengthExceeded);
            let did = Self::ensure_perms_owner_asset(origin, &ticker)?;

            FundingRound::insert(ticker, name.clone());
            Self::deposit_event(RawEvent::FundingRoundSet(did, ticker, name));
        }

        /// Updates the asset identifiers. Can only be called by the token owner.
        ///
        /// # Arguments
        /// * `origin` - the secondary key of the token owner.
        /// * `ticker` - the ticker of the token.
        /// * `identifiers` - the asset identifiers to be updated in the form of a vector of pairs
        ///    of `IdentifierType` and `AssetIdentifier` value.
        ///
        /// # Weight
        /// `150_000 + 20_000 * identifiers.len()`
        #[weight = <T as Trait>::WeightInfo::update_identifiers( identifiers.len() as u32)]
        pub fn update_identifiers(
            origin,
            ticker: Ticker,
            identifiers: Vec<AssetIdentifier>
        ) {
            let did = Self::ensure_perms_owner_asset(origin, &ticker)?;
            let identifiers: Vec<AssetIdentifier> = identifiers
                .into_iter()
                .filter_map(|identifier| identifier.validate())
                .collect();

            Identifiers::insert(ticker, identifiers.clone());
            Self::deposit_event(RawEvent::IdentifiersUpdated(did, ticker, identifiers));
        }

        /// Permissioning the Smart-Extension address for a given ticker.
        ///
        /// # Arguments
        /// * `origin` - Signatory who owns to ticker/asset.
        /// * `ticker` - ticker for whom extension get added.
        /// * `extension_details` - Details of the smart extension.
        #[weight = <T as Trait>::WeightInfo::add_extension()]
        pub fn add_extension(origin, ticker: Ticker, extension_details: SmartExtension<T::AccountId>) {
            let my_did = Self::ensure_perms_owner_asset(origin, &ticker)?;

            // Verify the details of smart extension & store it
            ensure!(!<ExtensionDetails<T>>::contains_key((ticker, &extension_details.extension_id)), Error::<T>::ExtensionAlreadyPresent);
            // Ensure the version compatibility with the asset.
            ensure!(Self::is_ext_compatible(&extension_details.extension_type, &extension_details.extension_id), Error::<T>::IncompatibleExtensionVersion);
            // Ensure the hard limit on the count of maximum transfer manager an asset can have.
            Self::ensure_max_limit_for_tm_extension(&extension_details.extension_type, &ticker)?;

            // Update the storage
            <ExtensionDetails<T>>::insert((ticker, &extension_details.extension_id), extension_details.clone());
            <Extensions<T>>::append((ticker, &extension_details.extension_type), extension_details.extension_id.clone());
            Self::deposit_event(RawEvent::ExtensionAdded(my_did, ticker, extension_details.extension_id, extension_details.extension_name, extension_details.extension_type));
        }

        /// Archived the extension. Extension is use to verify the compliance or any smart logic it posses.
        ///
        /// # Arguments
        /// * `origin` - Signatory who owns the ticker/asset.
        /// * `ticker` - Ticker symbol of the asset.
        /// * `extension_id` - AccountId of the extension that need to be archived.
        #[weight = <T as Trait>::WeightInfo::archive_extension()]
        pub fn archive_extension(origin, ticker: Ticker, extension_id: T::AccountId) {
            // Ensure the extrinsic is signed and have valid extension id.
            let did = Self::ensure_signed_and_validate_extension_id(origin, &ticker, &extension_id)?;

            // Mutate the extension details
            ensure!(!(<ExtensionDetails<T>>::get((ticker, &extension_id))).is_archive, Error::<T>::AlreadyArchived);
            <ExtensionDetails<T>>::mutate((ticker, &extension_id), |details| details.is_archive = true);
            Self::deposit_event(RawEvent::ExtensionArchived(did, ticker, extension_id));
        }

        /// Un-archived the extension. Extension is use to verify the compliance or any smart logic it posses.
        ///
        /// # Arguments
        /// * `origin` - Signatory who owns the ticker/asset.
        /// * `ticker` - Ticker symbol of the asset.
        /// * `extension_id` - AccountId of the extension that need to be un-archived.
        #[weight = <T as Trait>::WeightInfo::unarchive_extension()]
        pub fn unarchive_extension(origin, ticker: Ticker, extension_id: T::AccountId) {
            // Ensure the extrinsic is signed and have valid extension id.
            let did = Self::ensure_signed_and_validate_extension_id(origin, &ticker, &extension_id)?;

            // Mutate the extension details
            ensure!((<ExtensionDetails<T>>::get((ticker, &extension_id))).is_archive, Error::<T>::AlreadyUnArchived);
            <ExtensionDetails<T>>::mutate((ticker, &extension_id), |details| details.is_archive = false);
            Self::deposit_event(RawEvent::ExtensionUnArchived(did, ticker, extension_id));
        }

        /// Sets the primary issuance agent back to None. The caller must be the asset issuer. The asset
        /// issuer can always update the primary issuance agent using `transfer_primary_issuance_agent`.
        ///
        /// # Arguments
        /// * `origin` - The asset issuer.
        /// * `ticker` - Ticker symbol of the asset.
        #[weight = <T as Trait>::WeightInfo::remove_primary_issuance_agent()]
        pub fn remove_primary_issuance_agent(
            origin,
            ticker: Ticker,
        ) {
            let did = Self::ensure_perms_owner_asset(origin, &ticker)?;
            let old_pia = <Tokens<T>>::mutate(&ticker, |t| mem::replace(&mut t.primary_issuance_agent, None));
            Self::deposit_event(RawEvent::PrimaryIssuanceAgentTransferred(did, ticker, old_pia, None));
        }

        /// Remove the given smart extension id from the list of extension under a given ticker.
        ///
        /// # Arguments
        /// * `origin` - The asset issuer.
        /// * `ticker` - Ticker symbol of the asset.
        #[weight = <T as Trait>::WeightInfo::remove_smart_extension()]
        pub fn remove_smart_extension(origin, ticker: Ticker, extension_id: T::AccountId) {
            // Ensure the extrinsic is signed and have valid extension id.
            let did = Self::ensure_signed_and_validate_extension_id(origin, &ticker, &extension_id)?;

            let extension_type = Self::extension_details((&ticker, &extension_id)).extension_type;

            // Remove the storage reference for the given extension_id.
            // The order of SEs do not matter, so `swap_remove` is OK.
            <Extensions<T>>::mutate(&(ticker, extension_type), |extension_list| {
                if let Some(pos) = extension_list.iter().position(|ext| ext == &extension_id) {
                    extension_list.swap_remove(pos);
                }
            });
            <ExtensionDetails<T>>::remove((&ticker, &extension_id));
            Self::deposit_event(RawEvent::ExtensionRemoved(did, ticker, extension_id));
        }

        /// Claim a systematically reserved Polymath Classic (PMC) `ticker`
        /// and transfer it to the `origin`'s identity.
        ///
        /// To verify that the `origin` is in control of the Ethereum account on the books,
        /// an `ethereum_signature` containing the `origin`'s DID as the message
        /// must be provided by that Ethereum account.
        ///
        /// # Errors
        /// - `NoSuchClassicTicker` if this is not a systematically reserved PMC ticker.
        /// - `TickerAlreadyRegistered` if the ticker was already registered, e.g., by `origin`.
        /// - `TickerRegistrationExpired` if the ticker's registration has expired.
        /// - `BadOrigin` if not signed.
        /// - `InvalidEthereumSignature` if the `ethereum_signature` is not valid.
        /// - `NotAnOwner` if the ethereum account is not the owner of the PMC ticker.
        #[weight = <T as Trait>::WeightInfo::claim_classic_ticker()]
        pub fn claim_classic_ticker(origin, ticker: Ticker, ethereum_signature: ethereum::EcdsaSignature) {
            // Ensure the ticker is a classic one and fetch details.
            let ClassicTickerRegistration { eth_owner, .. } = ClassicTickers::get(ticker)
                .ok_or(Error::<T>::NoSuchClassicTicker)?;

            // Ensure ticker registration is still attached to the systematic DID.
            let sys_did = SystematicIssuers::ClassicMigration.as_id();
            match Self::is_ticker_available_or_registered_to(&ticker, sys_did) {
                TickerRegistrationStatus::RegisteredByOther => return Err(Error::<T>::TickerAlreadyRegistered.into()),
                TickerRegistrationStatus::Available => return Err(Error::<T>::TickerRegistrationExpired.into()),
                TickerRegistrationStatus::RegisteredByDid => {}
            }

            // Ensure we're signed & get did.
            let owner_did = Identity::<T>::ensure_perms(origin)?;

            // Have the caller prove that they own *some* Ethereum account
            // by having the signed signature contain the `owner_did`.
            //
            // We specifically use `owner_did` rather than `sender` such that
            // if the signing key's owner DID is changed after the creating
            // `ethereum_signature`, then the call is rejected
            // (caller might not have Ethereum account's private key).
            let eth_signer = ethereum::eth_check(owner_did, b"classic_claim", &ethereum_signature)
                .ok_or(Error::<T>::InvalidEthereumSignature)?;

            // Now we have an Ethereum account; ensure it's the *right one*.
            ensure!(eth_signer == eth_owner, Error::<T>::NotAnOwner);

            // Success; transfer the ticker to `owner_did`.
            Self::transfer_ticker(ticker, owner_did, sys_did);

            // Emit event.
            Self::deposit_event(RawEvent::ClassicTickerClaimed(owner_did, ticker, eth_signer));
        }

        /// Reserve a Polymath Classic (PMC) ticker.
        /// Must be called by root, and assigns the ticker to a systematic DID.
        ///
        /// # Arguments
        /// * `origin` which must be root.
        /// * `classic_ticker_import` specification for the PMC ticker.
        /// * `contract_did` to reserve the ticker to if `classic_ticker_import.is_contract` holds.
        /// * `config` to use for expiry and ticker length.
        ///
        /// # Errors
        /// * `AssetAlreadyCreated` if `classic_ticker_import.ticker` was created as an asset.
        /// * `TickerTooLong` if the `config` considers the `classic_ticker_import.ticker` too long.
        /// * `TickerAlreadyRegistered` if `classic_ticker_import.ticker` was already registered.
        #[weight = <T as Trait>::WeightInfo::reserve_classic_ticker()]
        pub fn reserve_classic_ticker(
            origin,
            classic_ticker_import: ClassicTickerImport,
            contract_did: IdentityId,
            config: TickerRegistrationConfig<T::Moment>,
        ) {
            ensure_root(origin)?;

            let cm_did = SystematicIssuers::ClassicMigration.as_id();
            // Use DID of someone at Polymath if it's a contract-made ticker registration.
            let did = if classic_ticker_import.is_contract { contract_did } else { cm_did };

            // Register the ticker...
            let expiry = Self::ticker_registration_checks(&classic_ticker_import.ticker, did, true, || config)?;
            Self::_register_ticker(&classic_ticker_import.ticker, did, expiry);

            // ..and associate it with additional info needed for claiming.
            let classic_ticker = ClassicTickerRegistration {
                eth_owner: classic_ticker_import.eth_owner,
                is_created: classic_ticker_import.is_created,
            };
            ClassicTickers::insert(&classic_ticker_import.ticker, classic_ticker);
        }

        /// Forces a transfer of token from `from_portfolio` to the PIA's default portfolio.
        /// Only PIA is allowed to execute this.
        ///
        /// # Arguments
        /// * `origin` Must be a PIA for a given ticker.
        /// * `ticker` Ticker symbol of the asset.
        /// * `value`  Amount of tokens need to force transfer.
        /// * `from_portfolio` From whom portfolio tokens gets transferred.
        #[weight = <T as Trait>::WeightInfo::controller_transfer()]
        pub fn controller_transfer(origin, ticker: Ticker, value: T::Balance, from_portfolio: PortfolioId) {
            // Ensure that `origin` is the PIA or the token owner.
            let pia = Self::ensure_pia_with_custody_and_permissions(origin, ticker)?.primary_did;

            // Transfer `value` of ticker tokens from `investor_did` to controller
            Self::unsafe_transfer(
                from_portfolio,
                PortfolioId::default_portfolio(pia),
                &ticker,
                value,
            )?;
            Self::deposit_event(RawEvent::ControllerTransfer(pia, ticker, from_portfolio, value));
        }
    }
}

decl_event! {
    pub enum Event<T>
    where
        Balance = <T as CommonTrait>::Balance,
        Moment = <T as pallet_timestamp::Trait>::Moment,
        AccountId = <T as frame_system::Trait>::AccountId,
    {
        /// Event for transfer of tokens.
        /// caller DID, ticker, from portfolio, to portfolio, value
        Transfer(IdentityId, Ticker, PortfolioId, PortfolioId, Balance),
        /// Emit when tokens get issued.
        /// caller DID, ticker, beneficiary DID, value, funding round, total issued in this funding round
        Issued(IdentityId, Ticker, IdentityId, Balance, FundingRoundName, Balance),
        /// Emit when tokens get redeemed.
        /// caller DID, ticker,  from DID, value
        Redeemed(IdentityId, Ticker, IdentityId, Balance),
        /// Event for creation of the asset.
        /// caller DID/ owner DID, ticker, total supply, divisibility, asset type, beneficiary DID
        AssetCreated(IdentityId, Ticker, Balance, bool, AssetType, IdentityId),
        /// Event emitted when a token identifiers are updated.
        /// caller DID, ticker, a vector of (identifier type, identifier value)
        IdentifiersUpdated(IdentityId, Ticker, Vec<AssetIdentifier>),
        /// Event for change in divisibility.
        /// caller DID, ticker, divisibility
        DivisibilityChanged(IdentityId, Ticker, bool),
        /// An additional event to Transfer; emitted when transfer_with_data is called.
        /// caller DID , ticker, from DID, to DID, value, data
        TransferWithData(IdentityId, Ticker, IdentityId, IdentityId, Balance, Vec<u8>),
        /// is_issuable() output
        /// ticker, return value (true if issuable)
        IsIssuable(Ticker, bool),
        /// Emit when ticker is registered.
        /// caller DID / ticker owner did, ticker, ticker owner, expiry
        TickerRegistered(IdentityId, Ticker, Option<Moment>),
        /// Emit when ticker is transferred.
        /// caller DID / ticker transferred to DID, ticker, from
        TickerTransferred(IdentityId, Ticker, IdentityId),
        /// Emit when token ownership is transferred.
        /// caller DID / token ownership transferred to DID, ticker, from
        AssetOwnershipTransferred(IdentityId, Ticker, IdentityId),
        /// An event emitted when an asset is frozen.
        /// Parameter: caller DID, ticker.
        AssetFrozen(IdentityId, Ticker),
        /// An event emitted when an asset is unfrozen.
        /// Parameter: caller DID, ticker.
        AssetUnfrozen(IdentityId, Ticker),
        /// An event emitted when a token is renamed.
        /// Parameters: caller DID, ticker, new token name.
        AssetRenamed(IdentityId, Ticker, AssetName),
        /// An event carrying the name of the current funding round of a ticker.
        /// Parameters: caller DID, ticker, funding round name.
        FundingRoundSet(IdentityId, Ticker, FundingRoundName),
        /// Emitted when extension is added successfully.
        /// caller DID, ticker, extension AccountId, extension name, type of smart Extension
        ExtensionAdded(IdentityId, Ticker, AccountId, SmartExtensionName, SmartExtensionType),
        /// Emitted when extension get archived.
        /// caller DID, ticker, AccountId
        ExtensionArchived(IdentityId, Ticker, AccountId),
        /// Emitted when extension get archived.
        /// caller DID, ticker, AccountId
        ExtensionUnArchived(IdentityId, Ticker, AccountId),
        /// An event emitted when the primary issuance agent of an asset is transferred.
        /// First DID is the old primary issuance agent and the second DID is the new primary issuance agent.
        PrimaryIssuanceAgentTransferred(IdentityId, Ticker, Option<IdentityId>, Option<IdentityId>),
        /// A new document attached to an asset
        DocumentAdded(IdentityId, Ticker, DocumentId, Document),
        /// A document removed from an asset
        DocumentRemoved(IdentityId, Ticker, DocumentId),
        /// A extension got removed.
        /// caller DID, ticker, AccountId
        ExtensionRemoved(IdentityId, Ticker, AccountId),
        /// A Polymath Classic token was claimed and transferred to a non-systematic DID.
        ClassicTickerClaimed(IdentityId, Ticker, ethereum::EthereumAddress),
        /// Migration error event.
        MigrationFailure(MigrationError<AssetMigrationError>),
        /// Event for when a forced transfer takes place.
        /// caller DID/ controller DID, ticker, Portfolio of token holder, value.
        ControllerTransfer(IdentityId, Ticker, PortfolioId, Balance),
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// Not a ticker transfer auth.
        NoTickerTransferAuth,
        /// Not a primary issuance agent transfer auth.
        NoPrimaryIssuanceAgentTransferAuth,
        /// Not a token ownership transfer auth.
        NotTickerOwnershipTransferAuth,
        /// The user is not authorized.
        Unauthorized,
        /// When extension already archived.
        AlreadyArchived,
        /// When extension already un-archived.
        AlreadyUnArchived,
        /// When extension is already added.
        ExtensionAlreadyPresent,
        /// The token has already been created.
        AssetAlreadyCreated,
        /// The ticker length is over the limit.
        TickerTooLong,
        /// The ticker has non-ascii-encoded parts.
        TickerNotAscii,
        /// The ticker is already registered to someone else.
        TickerAlreadyRegistered,
        /// An invalid total supply.
        InvalidTotalSupply,
        /// The total supply is above the limit.
        TotalSupplyAboveLimit,
        /// No such token.
        NoSuchAsset,
        /// The token is already frozen.
        AlreadyFrozen,
        /// Not an owner of the token.
        NotAnOwner,
        /// An overflow while calculating the balance.
        BalanceOverflow,
        /// An overflow while calculating the total supply.
        TotalSupplyOverflow,
        /// An invalid granularity.
        InvalidGranularity,
        /// The asset must be frozen.
        NotFrozen,
        /// No such smart extension.
        NoSuchSmartExtension,
        /// Transfer validation check failed.
        InvalidTransfer,
        /// The sender balance is not sufficient.
        InsufficientBalance,
        /// The token is already divisible.
        AssetAlreadyDivisible,
        /// Number of Transfer Manager extensions attached to an asset is equal to MaxNumberOfTMExtensionForAsset.
        MaximumTMExtensionLimitReached,
        /// Given smart extension is not compatible with the asset.
        IncompatibleExtensionVersion,
        /// An invalid Ethereum `EcdsaSignature`.
        InvalidEthereumSignature,
        /// The given ticker is not a classic one.
        NoSuchClassicTicker,
        /// Registration of ticker has expired.
        TickerRegistrationExpired,
        /// Transfers to self are not allowed
        SenderSameAsReceiver,
        /// The given Document does not exist.
        NoSuchDoc,
        /// The secondary key does not have the required Asset permission
        SecondaryKeyNotAuthorizedForAsset,
        /// Maximum length of asset name has been exceeded.
        MaxLengthOfAssetNameExceeded,
        /// Maximum length of the funding round name has been exceeded.
        FundingRoundNameMaxLengthExceeded,
    }
}

impl<T: Trait> AssetFnTrait<T::Balance, T::AccountId, T::Origin> for Module<T> {
    fn is_owner(ticker: &Ticker, did: IdentityId) -> bool {
        Self::_is_owner(ticker, did)
    }

    /// Get the asset `id` balance of `who`.
    fn balance(ticker: &Ticker, who: IdentityId) -> T::Balance {
        Self::balance_of(ticker, &who)
    }

    /// Get the total supply of an asset `id`
    fn total_supply(ticker: &Ticker) -> T::Balance {
        Self::token_details(ticker).total_supply
    }

    fn get_balance_at(ticker: &Ticker, did: IdentityId, at: CheckpointId) -> T::Balance {
        Self::get_balance_at(*ticker, did, at)
    }

    /// Returns the PIA if it's assigned or else the owner of the token
    fn primary_issuance_agent_or_owner(ticker: &Ticker) -> IdentityId {
        let token_details = Self::token_details(ticker);
        token_details
            .primary_issuance_agent
            .unwrap_or(token_details.owner_did)
    }

    fn base_transfer(
        from_portfolio: PortfolioId,
        to_portfolio: PortfolioId,
        ticker: &Ticker,
        value: T::Balance,
    ) -> DispatchResult {
        Self::base_transfer(from_portfolio, to_portfolio, ticker, value)
    }

    fn ensure_perms_owner_asset(
        origin: T::Origin,
        ticker: &Ticker,
    ) -> Result<IdentityId, DispatchError> {
        Self::ensure_perms_owner_asset(origin, ticker)
    }

    #[inline]
    fn create_asset(
        origin: T::Origin,
        name: AssetName,
        ticker: Ticker,
        total_supply: T::Balance,
        divisible: bool,
        asset_type: AssetType,
        identifiers: Vec<AssetIdentifier>,
        funding_round: Option<FundingRoundName>,
    ) -> DispatchResult {
        Self::create_asset(
            origin,
            name,
            ticker,
            total_supply,
            divisible,
            asset_type,
            identifiers,
            funding_round,
        )
    }

    #[inline]
    fn register_ticker(origin: T::Origin, ticker: Ticker) -> DispatchResult {
        Self::register_ticker(origin, ticker)
    }

    #[cfg(feature = "runtime-benchmarks")]
    /// Adds an artificial IU claim for benchmarks
    fn add_investor_uniqueness_claim(did: IdentityId, ticker: Ticker) {
        use polymesh_primitives::{CddId, Claim, InvestorUid, Scope};
        Identity::<T>::base_add_claim(
            did,
            Claim::InvestorUniqueness(
                Scope::Ticker(ticker),
                did,
                CddId::new(did, InvestorUid::from(did.to_bytes())),
            ),
            did,
            None,
        );
        let current_balance = Self::balance_of(ticker, did);
        <AggregateBalance<T>>::insert(ticker, &did, current_balance);
        <BalanceOfAtScope<T>>::insert(did, did, current_balance);
        <ScopeIdOf>::insert(ticker, did, did);
    }
}

impl<T: Trait> AssetSubTrait<T::Balance> for Module<T> {
    fn accept_ticker_transfer(to_did: IdentityId, auth_id: u64) -> DispatchResult {
        Self::_accept_ticker_transfer(to_did, auth_id)
    }

    fn accept_primary_issuance_agent_transfer(to_did: IdentityId, auth_id: u64) -> DispatchResult {
        Self::_accept_primary_issuance_agent_transfer(to_did, auth_id)
    }

    fn accept_asset_ownership_transfer(to_did: IdentityId, auth_id: u64) -> DispatchResult {
        Self::_accept_token_ownership_transfer(to_did, auth_id)
    }

    fn update_balance_of_scope_id(
        of: ScopeId,
        target_did: IdentityId,
        ticker: Ticker,
    ) -> DispatchResult {
        let balance_at_scope = Self::balance_of_at_scope(of, target_did);
        // Used `balance_at_scope` variable to skip re-updating the aggregate balance of the given identityId whom
        // has the scope claim already.
        if balance_at_scope == Zero::zero() {
            let current_balance = Self::balance_of(ticker, target_did);
            // Update the balance on the identityId under the given scopeId.
            <BalanceOfAtScope<T>>::insert(of, target_did, current_balance);
            // current aggregate balance + current identity balance is always less then the total_supply of given ticker.
            <AggregateBalance<T>>::mutate(ticker, of, |bal| *bal = *bal + current_balance);
        }
        // Caches the `ScopeId` for a given IdentityId and ticker.
        // this is needed to avoid the on-chain iteration of the claims to find the ScopeId.
        ScopeIdOf::insert(ticker, target_did, of);
        Ok(())
    }

    /// Returns balance for a given scope id and target DID.
    fn balance_of_at_scope(scope_id: &ScopeId, target: &IdentityId) -> T::Balance {
        Self::balance_of_at_scope(scope_id, target)
    }
}

/// All functions in the decl_module macro become part of the public interface of the module
/// If they are there, they are accessible via extrinsic calls whether they are public or not
/// However, in the impl module section (this, below) the functions can be public and private
/// Private functions are internal to this module e.g.: _transfer
/// Public functions can be called from other modules e.g.: lock and unlock (being called from the tcr module)
/// All functions in the impl module section are not part of public interface because they are not part of the Call enum.
impl<T: Trait> Module<T> {
    /// Returns the max number of extensions that can be attached to an asset
    pub fn max_number_of_tm_extension() -> u32 {
        T::MaxNumberOfTMExtensionForAsset::get()
    }

    fn ensure_pia_with_custody_and_permissions(
        origin: T::Origin,
        ticker: Ticker,
    ) -> Result<PermissionedCallOriginData<T::AccountId>, DispatchError> {
        let data = Identity::<T>::ensure_origin_call_permissions(origin)?;
        let skey = data.secondary_key.as_ref();

        // Ensure that the secondary key has asset permission
        Self::ensure_asset_perms(skey, &ticker)?;

        // Ensure that the sender is the PIA
        Self::ensure_pia(&ticker, data.primary_did)?;

        // Ensure that the caller has relevant portfolio permissions
        let portfolio = PortfolioId::default_portfolio(data.primary_did);

        // Ensure the PIA has not assigned custody of their default portfolio, and that caller is permissioned
        Portfolio::<T>::ensure_portfolio_custody_and_permission(portfolio, data.primary_did, skey)?;
        Ok(data)
    }

    /// Ensure that `origin` is permissioned for this call and that its identity is `ticker`'s owner.
    pub fn ensure_perms_owner(
        origin: T::Origin,
        ticker: &Ticker,
    ) -> Result<IdentityId, DispatchError> {
        let did = Identity::<T>::ensure_perms(origin)?;
        Self::ensure_owner(ticker, did)?;
        Ok(did)
    }

    /// Ensure that `origin` is permissioned for this call, its identity is `ticker`'s owner and,
    /// the secondary key has relevant asset permissions.
    pub fn ensure_perms_owner_asset(
        origin: T::Origin,
        ticker: &Ticker,
    ) -> Result<IdentityId, DispatchError> {
        // Ensure that the caller has extrinsic permission.
        let PermissionedCallOriginData {
            primary_did,
            secondary_key,
            ..
        } = Identity::<T>::ensure_origin_call_permissions(origin)?;

        // Ensure that the caller's did is the owner of the token.
        Self::ensure_owner(ticker, primary_did)?;

        // Ensure that the secondary key has asset permission
        Self::ensure_asset_perms(secondary_key.as_ref(), ticker)?;

        Ok(primary_did)
    }

    /// Ensure that the secondary key has relevant asset permissions.
    /// If `secondary_key` is None, the caller is the primary key and has all permissions.
    pub fn ensure_asset_perms(
        secondary_key: Option<&SecondaryKey<T::AccountId>>,
        ticker: &Ticker,
    ) -> DispatchResult {
        if let Some(sk) = secondary_key {
            ensure!(
                sk.has_asset_permission(*ticker),
                Error::<T>::SecondaryKeyNotAuthorizedForAsset
            );
        }
        Ok(())
    }

    /// Ensure that `did` is the owner of `ticker`.
    pub fn ensure_owner(ticker: &Ticker, did: IdentityId) -> DispatchResult {
        ensure!(Self::is_owner(ticker, did), Error::<T>::Unauthorized);
        Ok(())
    }

    /// Ensure that `ticker` is a valid created asset.
    fn ensure_asset_exists(ticker: &Ticker) -> DispatchResult {
        ensure!(<Tokens<T>>::contains_key(&ticker), Error::<T>::NoSuchAsset);
        Ok(())
    }

    /// Ensure that the document `doc` exists for `ticker`.
    pub fn ensure_doc_exists(ticker: &Ticker, doc: &DocumentId) -> DispatchResult {
        ensure!(
            AssetDocuments::contains_key(ticker, doc),
            Error::<T>::NoSuchDoc
        );
        Ok(())
    }

    pub fn is_owner(ticker: &Ticker, did: IdentityId) -> bool {
        Self::_is_owner(ticker, did)
    }

    pub fn _is_owner(ticker: &Ticker, did: IdentityId) -> bool {
        Self::token_details(ticker).owner_did == did
    }

    fn maybe_ticker(ticker: &Ticker) -> Option<TickerRegistration<T::Moment>> {
        <Tickers<T>>::contains_key(ticker).then(|| <Tickers<T>>::get(ticker))
    }

    pub fn is_ticker_available(ticker: &Ticker) -> bool {
        // Assumes uppercase ticker
        if let Some(ticker) = Self::maybe_ticker(ticker) {
            ticker
                .expiry
                .filter(|&e| <pallet_timestamp::Module<T>>::get() > e)
                .is_some()
        } else {
            true
        }
    }

    /// Returns `true` iff the ticker exists, is owned by `did`, and ticker hasn't expired.
    pub fn is_ticker_registry_valid(ticker: &Ticker, did: IdentityId) -> bool {
        // Assumes uppercase ticker
        if let Some(ticker) = Self::maybe_ticker(ticker) {
            let now = <pallet_timestamp::Module<T>>::get();
            ticker.owner == did && ticker.expiry.filter(|&e| now > e).is_none()
        } else {
            false
        }
    }

    /// Returns:
    /// - `RegisteredByOther` if ticker is registered to someone else.
    /// - `Available` if ticker is available for registry.
    /// - `RegisteredByDid` if ticker is already registered to provided did.
    pub fn is_ticker_available_or_registered_to(
        ticker: &Ticker,
        did: IdentityId,
    ) -> TickerRegistrationStatus {
        // Assumes uppercase ticker
        match Self::maybe_ticker(ticker) {
            Some(TickerRegistration { expiry, owner }) => match expiry {
                // Ticker registered to someone but expired and can be registered again.
                Some(expiry) if <pallet_timestamp::Module<T>>::get() > expiry => {
                    TickerRegistrationStatus::Available
                }
                // Ticker is already registered to provided did (may or may not expire in future).
                _ if owner == did => TickerRegistrationStatus::RegisteredByDid,
                // Ticker registered to someone else and hasn't expired.
                _ => TickerRegistrationStatus::RegisteredByOther,
            },
            // Ticker not registered yet.
            None => TickerRegistrationStatus::Available,
        }
    }

    /// Ensure `ticker` is fully printable ASCII (SPACE to '~').
    fn ensure_ticker_ascii(ticker: &Ticker) -> DispatchResult {
        let bytes = ticker.as_slice();
        // Find first byte not printable ASCII.
        let good = bytes
            .iter()
            .position(|b| !matches!(b, 32..=126))
            // Everything after must be a NULL byte.
            .map_or(true, |nm_pos| bytes[nm_pos..].iter().all(|b| *b == 0));
        ensure!(good, Error::<T>::TickerNotAscii);
        Ok(())
    }

    /// Before registering a ticker, do some checks, and return the expiry moment.
    fn ticker_registration_checks(
        ticker: &Ticker,
        to_did: IdentityId,
        no_re_register: bool,
        config: impl FnOnce() -> TickerRegistrationConfig<T::Moment>,
    ) -> Result<Option<T::Moment>, DispatchError> {
        Self::ensure_ticker_ascii(&ticker)?;
        Self::ensure_asset_fresh(&ticker)?;

        let config = config();

        // Ensure the ticker is not too long.
        Self::ensure_ticker_length(&ticker, &config)?;

        // Ensure that the ticker is not registered by someone else (or `to_did`, possibly).
        if match Self::is_ticker_available_or_registered_to(&ticker, to_did) {
            TickerRegistrationStatus::RegisteredByOther => true,
            TickerRegistrationStatus::RegisteredByDid => no_re_register,
            _ => false,
        } {
            return Err(Error::<T>::TickerAlreadyRegistered.into());
        }

        Ok(config
            .registration_length
            .map(|exp| <pallet_timestamp::Module<T>>::get() + exp))
    }

    /// Without charging any fees,
    /// register the given `ticker` to the `owner` identity,
    /// with the registration being removed at `expiry`.
    fn _register_ticker(ticker: &Ticker, owner: IdentityId, expiry: Option<T::Moment>) {
        if let Some(ticker_details) = Self::maybe_ticker(ticker) {
            AssetOwnershipRelations::remove(ticker_details.owner, ticker);
        }

        let ticker_registration = TickerRegistration { owner, expiry };

        // Store ticker registration details
        <Tickers<T>>::insert(ticker, ticker_registration);
        AssetOwnershipRelations::insert(owner, ticker, AssetOwnershipRelation::TickerOwned);

        // Not a classic ticker anymore if it was.
        ClassicTickers::remove(&ticker);

        Self::deposit_event(RawEvent::TickerRegistered(owner, *ticker, expiry));
    }

    // Get the total supply of an asset `id`.
    pub fn total_supply(ticker: Ticker) -> T::Balance {
        Self::token_details(ticker).total_supply
    }

    pub fn get_balance_at(ticker: Ticker, did: IdentityId, at: CheckpointId) -> T::Balance {
        <Checkpoint<T>>::balance_at(ticker, did, at)
            .unwrap_or_else(|| Self::balance_of(&ticker, &did))
    }

    pub fn _is_valid_transfer(
        ticker: &Ticker,
        from_portfolio: PortfolioId,
        to_portfolio: PortfolioId,
        value: T::Balance,
    ) -> StdResult<u8, DispatchError> {
        if Self::frozen(ticker) {
            return Ok(ERC1400_TRANSFERS_HALTED);
        }

        if !Identity::<T>::verify_iu_claims_for_transfer(
            *ticker,
            to_portfolio.did,
            from_portfolio.did,
        ) {
            return Ok(SCOPE_CLAIM_MISSING);
        }

        if Portfolio::<T>::ensure_portfolio_transfer_validity(
            &from_portfolio,
            &to_portfolio,
            ticker,
            &value,
        )
        .is_err()
        {
            return Ok(PORTFOLIO_FAILURE);
        }

        let from_scope_id = Self::scope_id_of(ticker, &from_portfolio.did);
        let to_scope_id = Self::scope_id_of(ticker, &to_portfolio.did);
        let token = <Tokens<T>>::get(ticker);
        if Statistics::<T>::verify_tm_restrictions(
            ticker,
            from_scope_id,
            to_scope_id,
            value,
            Self::aggregate_balance_of(ticker, &from_scope_id),
            Self::aggregate_balance_of(ticker, &to_scope_id),
            token.total_supply,
        )
        .is_err()
        {
            return Ok(TRANSFER_MANAGER_FAILURE);
        }

        let status_code = T::ComplianceManager::verify_restriction(
            ticker,
            Some(from_portfolio.did),
            Some(to_portfolio.did),
            value,
            Self::primary_issuance_agent_or_owner(&ticker),
        )
        .unwrap_or(COMPLIANCE_MANAGER_FAILURE);

        if status_code != ERC1400_TRANSFER_SUCCESS {
            return Ok(COMPLIANCE_MANAGER_FAILURE);
        }

        // SE are currently disabled
        // Self::compute_transfer_result_using_se()

        Ok(ERC1400_TRANSFER_SUCCESS)
    }

    // Transfers tokens from one identity to another
    pub fn unsafe_transfer(
        from_portfolio: PortfolioId,
        to_portfolio: PortfolioId,
        ticker: &Ticker,
        value: T::Balance,
    ) -> DispatchResult {
        Self::ensure_granular(ticker, value)?;

        ensure!(
            from_portfolio.did != to_portfolio.did,
            Error::<T>::SenderSameAsReceiver
        );

        let from_total_balance = Self::balance_of(ticker, from_portfolio.did);
        ensure!(from_total_balance >= value, Error::<T>::InsufficientBalance);
        let updated_from_total_balance = from_total_balance - value;

        let to_total_balance = Self::balance_of(ticker, to_portfolio.did);
        let updated_to_total_balance = to_total_balance
            .checked_add(&value)
            .ok_or(Error::<T>::BalanceOverflow)?;

        <Checkpoint<T>>::advance_update_balances(
            ticker,
            &[
                (from_portfolio.did, from_total_balance),
                (to_portfolio.did, to_total_balance),
            ],
        )?;

        // reduce sender's balance
        <BalanceOf<T>>::insert(ticker, &from_portfolio.did, updated_from_total_balance);
        // increase receiver's balance
        <BalanceOf<T>>::insert(ticker, &to_portfolio.did, updated_to_total_balance);
        // transfer portfolio balances
        Portfolio::<T>::unchecked_transfer_portfolio_balance(
            &from_portfolio,
            &to_portfolio,
            ticker,
            value,
        );

        let from_scope_id = Self::scope_id_of(ticker, &from_portfolio.did);
        let to_scope_id = Self::scope_id_of(ticker, &to_portfolio.did);

        Self::update_scope_balance(
            ticker,
            value,
            from_scope_id,
            from_portfolio.did,
            updated_from_total_balance,
            true,
        );
        Self::update_scope_balance(
            ticker,
            value,
            to_scope_id,
            to_portfolio.did,
            updated_to_total_balance,
            false,
        );

        // Update statistic info.
        // Using the aggregate balance to update the unique investor count.
        Statistics::<T>::update_transfer_stats(
            ticker,
            Some(Self::aggregate_balance_of(ticker, &from_scope_id)),
            Some(Self::aggregate_balance_of(ticker, &to_scope_id)),
            value,
        );

        Self::deposit_event(RawEvent::Transfer(
            from_portfolio.did,
            *ticker,
            from_portfolio,
            to_portfolio,
            value,
        ));
        Ok(())
    }

    /// Updates scope balances after a transfer
    pub fn update_scope_balance(
        ticker: &Ticker,
        value: T::Balance,
        scope_id: ScopeId,
        did: IdentityId,
        updated_balance: T::Balance,
        is_sender: bool,
    ) {
        // Calculate the new aggregate balance for given did.
        // It should not underflow/overflow but still to be defensive.
        let aggregate_balance = Self::aggregate_balance_of(ticker, &scope_id);
        let new_aggregate_balance = if is_sender {
            aggregate_balance.saturating_sub(value)
        } else {
            aggregate_balance.saturating_add(value)
        };

        <AggregateBalance<T>>::insert(ticker, &scope_id, new_aggregate_balance);
        <BalanceOfAtScope<T>>::insert(scope_id, did, updated_balance);
    }

    /// Ensure that `did` is the assigned PIA, or the token owner if no PIA is assigned
    pub fn ensure_pia(ticker: &Ticker, did: IdentityId) -> DispatchResult {
        ensure!(
            Self::primary_issuance_agent_or_owner(ticker) == did,
            Error::<T>::Unauthorized
        );
        Ok(())
    }

    pub fn _mint(
        ticker: &Ticker,
        caller: T::AccountId,
        to_did: IdentityId,
        value: T::Balance,
        protocol_fee_data: Option<ProtocolOp>,
    ) -> DispatchResult {
        Self::ensure_granular(ticker, value)?;

        // Read the token details
        let mut token = Self::token_details(ticker);
        // Prepare the updated total supply.
        let updated_total_supply = token
            .total_supply
            .checked_add(&value)
            .ok_or(Error::<T>::TotalSupplyOverflow)?;
        Self::ensure_within_max_supply(updated_total_supply)?;
        // Increase receiver balance.
        let current_to_balance = Self::balance_of(ticker, to_did);
        // No check since the total balance is always <= the total supply. The
        // total supply is already checked above.
        let updated_to_balance = current_to_balance + value;
        // No check since the default portfolio balance is always <= the total
        // supply. The total supply is already checked above.
        let updated_to_def_balance = Portfolio::<T>::portfolio_asset_balances(
            PortfolioId::default_portfolio(to_did),
            ticker,
        ) + value;

        // In transaction because we don't want fee to be charged if advancing fails.
        with_transaction(|| {
            // Charge the fee.
            if let Some(op) = protocol_fee_data {
                T::ProtocolFee::charge_fee(op)?;
            }

            // Advance checkpoint schedules and update last checkpoint.
            <Checkpoint<T>>::advance_update_balances(ticker, &[(to_did, current_to_balance)])
        })?;

        // Increase total supply
        token.total_supply = updated_total_supply;
        <BalanceOf<T>>::insert(ticker, &to_did, updated_to_balance);
        Portfolio::<T>::set_default_portfolio_balance(to_did, ticker, updated_to_def_balance);
        <Tokens<T>>::insert(ticker, token);

        if ScopeIdOf::contains_key(ticker, &to_did) {
            let scope_id = Self::scope_id_of(ticker, &to_did);
            Self::update_scope_balance(&ticker, value, scope_id, to_did, updated_to_balance, false);
            // Using the aggregate balance to update the unique investor count.
            Statistics::<T>::update_transfer_stats(
                &ticker,
                None,
                Some(Self::aggregate_balance_of(ticker, &scope_id)),
                value,
            );
        } else {
            // Since the PIA does not have a scope claim yet, we assume this is their only identity
            Statistics::<T>::update_transfer_stats(&ticker, None, Some(value), value);
        }

        let round = Self::funding_round(ticker);
        let ticker_round = (*ticker, round.clone());
        // No check since the issued balance is always <= the total
        // supply. The total supply is already checked above.
        let issued_in_this_round = Self::issued_in_funding_round(&ticker_round) + value;
        <IssuedInFundingRound<T>>::insert(&ticker_round, issued_in_this_round);
        let did = Context::current_identity_or::<Identity<T>>(&caller)?;
        Self::deposit_event(RawEvent::Transfer(
            did,
            *ticker,
            PortfolioId::default(),
            PortfolioId::default_portfolio(to_did),
            value,
        ));
        Self::deposit_event(RawEvent::Issued(
            did,
            *ticker,
            to_did,
            value,
            round,
            issued_in_this_round,
        ));

        Ok(())
    }

    fn ensure_granular(ticker: &Ticker, value: T::Balance) -> DispatchResult {
        ensure!(
            Self::check_granularity(&ticker, value),
            Error::<T>::InvalidGranularity
        );
        Ok(())
    }

    fn check_granularity(ticker: &Ticker, value: T::Balance) -> bool {
        // Read the token details
        let token = Self::token_details(ticker);
        token.divisible || Self::is_unit_multiple(value)
    }

    /// Is `value` a multiple of "one unit"?
    fn is_unit_multiple(value: T::Balance) -> bool {
        value % ONE_UNIT.into() == 0u32.into()
    }

    /// Accept and process a ticker transfer.
    pub fn _accept_ticker_transfer(to_did: IdentityId, auth_id: u64) -> DispatchResult {
        let auth = <Identity<T>>::ensure_authorization(&to_did.into(), auth_id)?;

        let ticker = match auth.authorization_data {
            AuthorizationData::TransferTicker(ticker) => ticker,
            _ => return Err(Error::<T>::NoTickerTransferAuth.into()),
        };

        Self::ensure_asset_fresh(&ticker)?;
        let ticker_details = Self::ticker_registration(&ticker);

        <Identity<T>>::consume_auth(ticker_details.owner, Signatory::from(to_did), auth_id)?;

        Self::transfer_ticker(ticker, to_did, ticker_details.owner);
        ClassicTickers::remove(&ticker); // Not a classic ticker anymore if it was.
        Ok(())
    }

    /// Transfer the given `ticker`'s registration from `from` to `to`.
    fn transfer_ticker(ticker: Ticker, to: IdentityId, from: IdentityId) {
        AssetOwnershipRelations::remove(from, ticker);
        AssetOwnershipRelations::insert(to, ticker, AssetOwnershipRelation::TickerOwned);
        <Tickers<T>>::mutate(&ticker, |tr| tr.owner = to);
        Self::deposit_event(RawEvent::TickerTransferred(to, ticker, from));
    }

    /// Accept and process a primary issuance agent transfer.
    pub fn _accept_primary_issuance_agent_transfer(
        to_did: IdentityId,
        auth_id: u64,
    ) -> DispatchResult {
        let auth = <Identity<T>>::ensure_authorization(&to_did.into(), auth_id)?;

        let ticker = match auth.authorization_data {
            AuthorizationData::TransferPrimaryIssuanceAgent(ticker) => ticker,
            _ => return Err(Error::<T>::NoPrimaryIssuanceAgentTransferAuth.into()),
        };

        Self::consume_auth_by_owner(&ticker, to_did, auth_id)?;

        let pia = Some(to_did);
        let old_pia = <Tokens<T>>::mutate(&ticker, |token| {
            mem::replace(&mut token.primary_issuance_agent, pia)
        });

        Self::deposit_event(RawEvent::PrimaryIssuanceAgentTransferred(
            to_did, ticker, old_pia, pia,
        ));

        Ok(())
    }

    /// Accept and process a token ownership transfer.
    pub fn _accept_token_ownership_transfer(to_did: IdentityId, auth_id: u64) -> DispatchResult {
        let auth = <Identity<T>>::ensure_authorization(&to_did.into(), auth_id)?;

        let ticker = match auth.authorization_data {
            AuthorizationData::TransferAssetOwnership(ticker) => ticker,
            _ => return Err(Error::<T>::NotTickerOwnershipTransferAuth.into()),
        };

        Self::ensure_asset_exists(&ticker)?;
        Self::consume_auth_by_owner(&ticker, to_did, auth_id)?;

        let ticker_details = Self::ticker_registration(&ticker);
        AssetOwnershipRelations::remove(ticker_details.owner, ticker);

        AssetOwnershipRelations::insert(to_did, ticker, AssetOwnershipRelation::AssetOwned);

        <Tickers<T>>::mutate(&ticker, |tr| tr.owner = to_did);
        let owner = <Tokens<T>>::mutate(&ticker, |tr| mem::replace(&mut tr.owner_did, to_did));

        Self::deposit_event(RawEvent::AssetOwnershipTransferred(to_did, ticker, owner));

        Ok(())
    }

    pub fn consume_auth_by_owner(
        ticker: &Ticker,
        to_did: IdentityId,
        auth_id: u64,
    ) -> DispatchResult {
        let owner = Self::token_details(ticker).owner_did;
        <Identity<T>>::consume_auth(owner, Signatory::from(to_did), auth_id)
    }

    pub fn verify_restriction(
        ticker: &Ticker,
        extension_caller: T::AccountId,
        from_did: Option<IdentityId>,
        to_did: Option<IdentityId>,
        value: T::Balance,
        holder_count: Counter,
        dest: T::AccountId,
    ) -> RestrictionResult {
        // 4 byte selector of verify_transfer - 0xD9386E41
        let selector = hex!("D1140AC9");
        let balance = |did| {
            T::Balance::encode(&match did {
                None => 0u32.into(),
                Some(did) => {
                    let scope_id = Self::scope_id_of(ticker, &did);
                    // Using aggregate balance instead of individual identity balance.
                    Self::aggregate_balance_of(ticker, &scope_id)
                }
            })
        };
        let balance_to = balance(to_did);
        let balance_from = balance(from_did);
        let encoded_to = Option::<IdentityId>::encode(&to_did);
        let encoded_from = Option::<IdentityId>::encode(&from_did);
        let encoded_value = T::Balance::encode(&value);
        let total_supply = T::Balance::encode(&<Tokens<T>>::get(&ticker).total_supply);
        let current_holder_count = Counter::encode(&holder_count);

        // Creation of the encoded data for the verifyTransfer function of the extension
        // i.e fn verify_transfer(
        //        from: Option<IdentityId>,
        //        to: Option<IdentityId>,
        //        value: Balance,
        //        balance_from: Balance,
        //        balance_to: Balance,
        //        total_supply: Balance,
        //        current_holder_count: Counter
        //    ) -> RestrictionResult { }

        let encoded_data = [
            &selector[..],
            &encoded_from[..],
            &encoded_to[..],
            &encoded_value[..],
            &balance_from[..],
            &balance_to[..],
            &total_supply[..],
            &current_holder_count[..],
        ]
        .concat();

        // Calling extension to verify the compliance requirement
        // native currency value should be `0` as no funds need to transfer to the smart extension
        // We are passing arbitrary high `gas_limit` value to make sure extension's function execute successfully
        // TODO: Once gas estimate function will be introduced, arbitrary gas value will be replaced by the estimated gas
        let (res, _gas_spent) = Self::call_extension(
            extension_caller,
            dest,
            T::AllowedGasLimit::get(),
            encoded_data,
        );
        if let Ok(is_allowed) = &res {
            if is_allowed.is_success() {
                if let Ok(allowed) = RestrictionResult::decode(&mut &is_allowed.data[..]) {
                    return allowed;
                }
            }
        }
        RestrictionResult::Invalid
    }

    /// A helper function that is used to call the smart extension function.
    ///
    /// # Arguments
    /// * `from` - Caller of the extension.
    /// * `dest` - Address/AccountId of the smart extension whom get called.
    /// * `value` - Amount of native currency that need to transfer to the extension.
    /// * `gas_limit` - Maximum amount of gas passed to successfully execute the function.
    /// * `data` - Encoded data that contains function selector and function arguments values.
    pub fn call_extension(
        from: T::AccountId,
        dest: T::AccountId,
        gas_limit: Gas,
        data: Vec<u8>,
    ) -> (ExecResult, Gas) {
        <pallet_contracts::Module<T>>::bare_call(from, dest, 0u32.into(), gas_limit, data)
    }

    /// RPC: Function allows external users to know wether the transfer extrinsic
    /// will be valid or not beforehand.
    pub fn unsafe_can_transfer(
        from_custodian: Option<IdentityId>,
        from_portfolio: PortfolioId,
        to_custodian: Option<IdentityId>,
        to_portfolio: PortfolioId,
        ticker: &Ticker,
        value: T::Balance,
    ) -> StdResult<u8, &'static str> {
        Ok(if !Self::check_granularity(&ticker, value) {
            // Granularity check
            INVALID_GRANULARITY
        } else if from_portfolio.did == to_portfolio.did {
            INVALID_RECEIVER_DID
        } else if !Identity::<T>::has_valid_cdd(from_portfolio.did) {
            INVALID_SENDER_DID
        } else if !Identity::<T>::verify_iu_claims_for_transfer(
            *ticker,
            to_portfolio.did,
            from_portfolio.did,
        ) {
            SCOPE_CLAIM_MISSING
        } else if let Err(_) = Portfolio::<T>::ensure_portfolio_custody(
            from_portfolio,
            from_custodian.unwrap_or(from_portfolio.did),
        ) {
            CUSTODIAN_ERROR
        } else if !Identity::<T>::has_valid_cdd(to_portfolio.did) {
            INVALID_RECEIVER_DID
        } else if let Err(_) = Portfolio::<T>::ensure_portfolio_custody(
            to_portfolio,
            to_custodian.unwrap_or(to_portfolio.did),
        ) {
            CUSTODIAN_ERROR
        } else if Self::balance_of(&ticker, from_portfolio.did) < value {
            ERC1400_INSUFFICIENT_BALANCE
        } else if let Err(_) = Portfolio::<T>::ensure_portfolio_transfer_validity(
            &from_portfolio,
            &to_portfolio,
            ticker,
            &value,
        ) {
            PORTFOLIO_FAILURE
        } else {
            // Compliance manager & Smart Extension check
            Self::_is_valid_transfer(&ticker, from_portfolio, to_portfolio, value)
                .unwrap_or(ERC1400_TRANSFER_FAILURE)
        })
    }

    /// Transfers an asset from one identity portfolio to another
    pub fn base_transfer(
        from_portfolio: PortfolioId,
        to_portfolio: PortfolioId,
        ticker: &Ticker,
        value: T::Balance,
    ) -> DispatchResult {
        // NB: This function does not check if the sender/receiver have custodian permissions on the portfolios.
        // The custodian permissions must be checked before this function is called.
        // The only place this function is used right now is the settlement engine and the settlement engine
        // checks custodial permissions when the instruction is authorized.

        // Validate the transfer
        let is_transfer_success =
            Self::_is_valid_transfer(&ticker, from_portfolio, to_portfolio, value)?;

        ensure!(
            is_transfer_success == ERC1400_TRANSFER_SUCCESS,
            Error::<T>::InvalidTransfer
        );

        Self::unsafe_transfer(from_portfolio, to_portfolio, ticker, value)?;

        Ok(())
    }

    /// Performs necessary checks on parameters of `create_asset`.
    fn ensure_create_asset_parameters(ticker: &Ticker, total_supply: T::Balance) -> DispatchResult {
        Self::ensure_asset_fresh(&ticker)?;
        Self::ensure_within_max_supply(total_supply)?;
        Self::ensure_ticker_length(&ticker, &Self::ticker_registration_config())
    }

    /// Ensure asset `ticker` doesn't exist yet.
    fn ensure_asset_fresh(ticker: &Ticker) -> DispatchResult {
        ensure!(
            !<Tokens<T>>::contains_key(ticker),
            Error::<T>::AssetAlreadyCreated
        );
        Ok(())
    }

    /// Ensure `supply <= MAX_SUPPLY`.
    fn ensure_within_max_supply(supply: T::Balance) -> DispatchResult {
        ensure!(
            supply <= MAX_SUPPLY.into(),
            Error::<T>::TotalSupplyAboveLimit
        );
        Ok(())
    }

    /// Ensure ticker length is within limit per `config`.
    fn ensure_ticker_length<U>(
        ticker: &Ticker,
        config: &TickerRegistrationConfig<U>,
    ) -> DispatchResult {
        ensure!(
            ticker.len() <= usize::try_from(config.max_ticker_length).unwrap_or_default(),
            Error::<T>::TickerTooLong
        );
        Ok(())
    }

    // Return bool to know whether the given extension is compatible with the supported version of asset.
    fn is_ext_compatible(ext_type: &SmartExtensionType, extension_id: &T::AccountId) -> bool {
        // Access version.
        let ext_version = <polymesh_contracts::Module<T>>::extension_info(extension_id).version;
        Self::compatible_extension_version(ext_type) == ext_version
    }

    /// Ensure the number of attached transfer manager extension should be < `MaxNumberOfTMExtensionForAsset`.
    fn ensure_max_limit_for_tm_extension(
        ext_type: &SmartExtensionType,
        ticker: &Ticker,
    ) -> DispatchResult {
        if *ext_type == SmartExtensionType::TransferManager {
            let no_of_ext = u32::try_from(
                <Extensions<T>>::get((ticker, SmartExtensionType::TransferManager)).len(),
            )
            .unwrap_or_default();
            ensure!(
                no_of_ext < T::MaxNumberOfTMExtensionForAsset::get(),
                Error::<T>::MaximumTMExtensionLimitReached
            );
        }
        Ok(())
    }

    /// Compute the result of the transfer. It is currently not used but might be at a later date.
    pub fn compute_transfer_result_using_se(
        ticker: &Ticker,
        extension_caller: T::AccountId,
        from_portfolio: PortfolioId,
        to_portfolio: PortfolioId,
        value: T::Balance,
    ) -> u8 {
        let mut result = true;
        let mut is_valid = false;
        let mut is_invalid = false;
        let mut force_valid = false;
        let current_holder_count = Statistics::<T>::investor_count(ticker);
        let tms = Self::extensions((ticker, SmartExtensionType::TransferManager))
            .into_iter()
            .filter(|tm| {
                !Self::extension_details((ticker, tm)).is_archive
                    && Self::is_ext_compatible(&SmartExtensionType::TransferManager, &tm)
            })
            .collect::<Vec<T::AccountId>>();
        if !tms.is_empty() {
            for tm in tms.into_iter() {
                let result = Self::verify_restriction(
                    ticker,
                    extension_caller.clone(),
                    Some(from_portfolio.did),
                    Some(to_portfolio.did),
                    value,
                    current_holder_count,
                    tm,
                );
                match result {
                    RestrictionResult::Valid => is_valid = true,
                    RestrictionResult::Invalid => is_invalid = true,
                    RestrictionResult::ForceValid => force_valid = true,
                }
            }
            //is_valid = force_valid ? true : (is_invalid ? false : is_valid);
            result = force_valid || !is_invalid && is_valid;
        }
        if result {
            ERC1400_TRANSFER_SUCCESS
        } else {
            SMART_EXTENSION_FAILURE
        }
    }

    /// Ensure the extrinsic is signed and have valid extension id.
    fn ensure_signed_and_validate_extension_id(
        origin: T::Origin,
        ticker: &Ticker,
        id: &T::AccountId,
    ) -> Result<IdentityId, DispatchError> {
        let did = Self::ensure_perms_owner_asset(origin, ticker)?;
        ensure!(
            <ExtensionDetails<T>>::contains_key((ticker, id)),
            Error::<T>::NoSuchSmartExtension
        );
        Ok(did)
    }
}
