// This file is part of the Polymesh distribution (https://github.com/PolymathNetwork/Polymesh).
// Copyright (c) 2020 Polymath
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

//! # Portfolio Module
//!
//! ## Overview
//!
//! The portfolio module provides the essential extrinsics to manage asset portfolios, public
//! functions for integration of portfolios into other pallets, and implementations of RPC getters.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `create_portfolio`: Creates a new user portfolio.
//! - `delete_portfolio`: Deletes an existing user portfolio.
//! - `move_portfolio_funds`: Moves specified amounts of assets from one portfolio to another portfolio
//!   of the same DID.
//! - `rename_portfolio`: Renames a user portfolio.
//!
//! ### Public Functions
//!
//! - `default_portfolio_balance`: Returns the ticker balance of the identity's default portfolio.
//! - `user_portfolio_balance`: Returns the ticker balance of an identity's user portfolio.
//! - `set_default_portfolio_balance`: Sets the ticker balance of the identity's default portfolio.
//! - `unchecked_transfer_portfolio_balance`: Transfers funds from one portfolio to another.
//! - `ensure_portfolio_custody`: Makes sure that the given identity has custodian access over the portfolio.
//! - `ensure_portfolio_transfer_validity`: Makes sure that a transfer between two portfolios is valid.

#![feature(const_option)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

use codec::{Decode, Encode};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure,
    storage::StorageValue, weights::Weight, IterableStorageDoubleMap,
};
use pallet_identity::{self as identity, PermissionedCallOriginData};
use polymesh_common_utilities::{
    identity::Trait as IdentityTrait, traits::portfolio::PortfolioSubTrait, CommonTrait,
};
use polymesh_primitives::{
    storage_migration_ver, AuthorizationData, AuthorizationError, IdentityId, PortfolioId,
    PortfolioKind, PortfolioName, PortfolioNumber, SecondaryKey, Signatory, Ticker,
};
use sp_arithmetic::traits::{CheckedSub, Saturating};
use sp_std::{iter, mem, prelude::Vec};

storage_migration_ver!(1);

type Identity<T> = identity::Module<T>;

/// The ticker and balance of an asset to be moved from one portfolio to another.
#[derive(Encode, Decode, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct MovePortfolioItem<Balance> {
    /// The ticker of the asset to be moved.
    pub ticker: Ticker,
    /// The balance of the asset to be moved.
    pub amount: Balance,
}

pub trait WeightInfo {
    fn create_portfolio(i: u32) -> Weight;
    fn delete_portfolio() -> Weight;
    fn move_portfolio_funds(i: u32) -> Weight;
    fn rename_portfolio(i: u32) -> Weight;
}

pub trait Trait: CommonTrait + IdentityTrait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type WeightInfo: WeightInfo;
}

decl_storage! {
    trait Store for Module<T: Trait> as Portfolio {
        /// The set of existing portfolios with their names. If a certain pair of a DID and
        /// portfolio number maps to `None` then such a portfolio doesn't exist. Conversely, if a
        /// pair maps to `Some(name)` then such a portfolio exists and is called `name`.
        pub Portfolios get(fn portfolios):
            double_map hasher(identity) IdentityId, hasher(twox_64_concat) PortfolioNumber =>
            PortfolioName;
        /// The asset balances of portfolios.
        pub PortfolioAssetBalances get(fn portfolio_asset_balances):
            double_map hasher(twox_64_concat) PortfolioId, hasher(blake2_128_concat) Ticker =>
            T::Balance;
        /// The next portfolio sequence number of an identity.
        pub NextPortfolioNumber get(fn next_portfolio_number):
            map hasher(identity) IdentityId => PortfolioNumber;
        /// The custodian of a particular portfolio. None implies that the identity owner is the custodian.
        pub PortfolioCustodian get(fn portfolio_custodian):
            map hasher(twox_64_concat) PortfolioId => Option<IdentityId>;
        /// Amount of assets locked in a portfolio.
        /// These assets show up in portfolio balance but can not be transferred away.
        pub PortfolioLockedAssets get(fn locked_assets):
            double_map hasher(twox_64_concat) PortfolioId, hasher(blake2_128_concat) Ticker =>
            T::Balance;
        /// Tracks all the portfolios in custody of a particular identity. Only used by the UIs.
        /// When `true` is stored as the value for a given `(did, pid)`, it means that `pid` is in custody of `did`.
        /// `false` values are never explicitly stored in the map, and are instead inferred by the absence of a key.
        pub PortfoliosInCustody get(fn portfolios_in_custody):
            double_map hasher(identity) IdentityId, hasher(twox_64_concat) PortfolioId => bool;
        /// Storage version.
        StorageVersion get(fn storage_version) build(|_| Version::new(1).unwrap()): Version;
    }
}

decl_event! {
    pub enum Event<T> where
        Balance = <T as CommonTrait>::Balance,
    {
        /// The portfolio has been successfully created.
        ///
        /// # Parameters
        /// * origin DID
        /// * portfolio number
        /// * portfolio name
        PortfolioCreated(IdentityId, PortfolioNumber, PortfolioName),
        /// The portfolio has been successfully removed.
        ///
        /// # Parameters
        /// * origin DID
        /// * portfolio number
        PortfolioDeleted(IdentityId, PortfolioNumber),
        /// A token amount has been moved from one portfolio to another.
        ///
        /// # Parameters
        /// * origin DID
        /// * source portfolio
        /// * destination portfolio
        /// * asset ticker
        /// * asset balance that was moved
        MovedBetweenPortfolios(
            IdentityId,
            PortfolioId,
            PortfolioId,
            Ticker,
            Balance
        ),
        /// The portfolio identified with `num` has been renamed to `name`.
        ///
        /// # Parameters
        /// * origin DID
        /// * portfolio number
        /// * portfolio name
        PortfolioRenamed(IdentityId, PortfolioNumber, PortfolioName),
        /// All non-default portfolio numbers and names of a DID.
        ///
        /// # Parameters
        /// * origin DID
        /// * vector of number-name pairs
        UserPortfolios(IdentityId, Vec<(PortfolioNumber, PortfolioName)>),
        /// Custody of a portfolio has been given to a different identity
        ///
        /// # Parameters
        /// * origin DID
        /// * portfolio id
        /// * portfolio custodian did
        PortfolioCustodianChanged(IdentityId, PortfolioId, IdentityId),
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// The portfolio doesn't exist.
        PortfolioDoesNotExist,
        /// Insufficient balance for a transaction.
        InsufficientPortfolioBalance,
        /// The source and destination portfolios should be different.
        DestinationIsSamePortfolio,
        /// The portfolio couldn't be renamed because the chosen name is already in use.
        PortfolioNameAlreadyInUse,
        /// The secondary key is not authorized to access the portfolio(s).
        SecondaryKeyNotAuthorizedForPortfolio,
        /// The porfolio's custody is with someone other than the caller.
        UnauthorizedCustodian,
        /// The authorization is for something other than portfolio custody
        IrrelevantAuthorization,
        /// Can not unlock more tokens than what are locked
        InsufficientTokensLocked,
        /// The portfolio still has some asset balance left
        PortfolioNotEmpty,
        /// The portfolios belong to different identities
        DifferentIdentityPortfolios
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        /// The event logger.
        fn deposit_event() = default;

        fn on_runtime_upgrade() -> Weight {
            use polymesh_primitives::{storage_migrate_on, migrate::move_map_rename_module};

            let storage_ver = StorageVersion::get();

            storage_migrate_on!(storage_ver, 1, {
                move_map_rename_module::<PortfolioName>(b"Session", b"Portfolio", b"Portfolios");
                move_map_rename_module::<T::Balance>(b"Session", b"Portfolio", b"PortfolioAssetBalances");
                move_map_rename_module::<PortfolioNumber>(b"Session", b"Portfolio", b"NextPortfolioNumber");
                move_map_rename_module::<Option<IdentityId>>(b"Session", b"Portfolio", b"PortfolioCustodian");
                move_map_rename_module::<T::Balance>(b"Session", b"Portfolio", b"PortfolioLockedAssets");
                move_map_rename_module::<bool>(b"Session", b"Portfolio", b"PortfoliosInCustody");
            });

            0
        }

        /// Creates a portfolio with the given `name`.
        #[weight = <T as Trait>::WeightInfo::create_portfolio(name.len() as u32)]
        pub fn create_portfolio(origin, name: PortfolioName) {
            let primary_did = Identity::<T>::ensure_perms(origin)?;
            Self::ensure_name_unique(&primary_did, &name)?;
            let num = Self::get_next_portfolio_number(&primary_did);
            Portfolios::insert(&primary_did, &num, name.clone());
            Self::deposit_event(RawEvent::PortfolioCreated(primary_did, num, name));
        }

        /// Deletes a user portfolio. A portfolio can be deleted only if it has no funds.
        ///
        /// # Errors
        /// * `PortfolioDoesNotExist` if `num` doesn't reference a valid portfolio.
        /// * `PortfolioNotEmpty` if the portfolio still holds any asset
        #[weight = <T as Trait>::WeightInfo::delete_portfolio()]
        pub fn delete_portfolio(origin, num: PortfolioNumber) {
            let PermissionedCallOriginData {
                primary_did,
                secondary_key,
                ..
            } = Identity::<T>::ensure_origin_call_permissions(origin)?;

            let portfolio_id = PortfolioId::user_portfolio(primary_did, num);

            // Check that the portfolio exists and the secondary key has access to it
            Self::ensure_user_portfolio_validity(primary_did, num)?;
            Self::ensure_portfolio_custody_and_permission(portfolio_id, primary_did, secondary_key.as_ref())?;

            Portfolios::remove(&primary_did, &num);
            Self::deposit_event(RawEvent::PortfolioDeleted(primary_did, num));
        }

        /// Moves a token amount from one portfolio of an identity to another portfolio of the same
        /// identity. Must be called by the custodian of the sender.
        /// Funds from deleted portfolios can also be recovered via this method.
        ///
        /// # Errors
        /// * `PortfolioDoesNotExist` if one or both of the portfolios reference an invalid portfolio.
        /// * `DestinationIsSamePortfolio` if both sender and receiver portfolio are the same
        /// * `DifferentIdentityPortfolios` if the sender and receiver portfolios belong to different identities
        /// * `UnauthorizedCustodian` if the caller is not the custodian of the from portfolio
        /// * `InsufficientPortfolioBalance` if the sender does not have enough free balance
        #[weight = <T as Trait>::WeightInfo::move_portfolio_funds(items.len() as u32)]
        pub fn move_portfolio_funds(
            origin,
            from: PortfolioId,
            to: PortfolioId,
            items: Vec<MovePortfolioItem<<T as CommonTrait>::Balance>>,
        ) {
            let PermissionedCallOriginData {
                primary_did,
                secondary_key,
                ..
            } = Identity::<T>::ensure_origin_call_permissions(origin)?;

            // Ensure the source and destination portfolios are in fact different.
            ensure!(from != to, Error::<T>::DestinationIsSamePortfolio);
            // Ensure the source and destination DID are in fact same.
            ensure!(from.did == to.did, Error::<T>::DifferentIdentityPortfolios);

            // Ensure the sender is the custodian & secondary key has access to the portfolio.
            Self::ensure_portfolio_custody_and_permission(from, primary_did, secondary_key.as_ref())?;

            // Ensure the receiving portfolio exists.
            Self::ensure_portfolio_validity(&to)?;
            // Ensure that the secondary key has access to the receiver's portfolio.
            Self::ensure_user_portfolio_permission(secondary_key.as_ref(), to)?;

            // Ensure there are sufficient funds for all moves.
            for item in &items {
                Self::ensure_sufficient_balance(&from, &item.ticker, &item.amount)?;
            }

            // Commit changes.
            for item in items {
                Self::unchecked_transfer_portfolio_balance(&from, &to, &item.ticker, item.amount);
                Self::deposit_event(RawEvent::MovedBetweenPortfolios(
                    primary_did,
                    from,
                    to,
                    item.ticker,
                    item.amount
                ));
            }
        }

        /// Renames a non-default portfolio.
        ///
        /// # Errors
        /// * `PortfolioDoesNotExist` if `num` doesn't reference a valid portfolio.
        #[weight = <T as Trait>::WeightInfo::rename_portfolio(to_name.len() as u32)]
        pub fn rename_portfolio(
            origin,
            num: PortfolioNumber,
            to_name: PortfolioName,
        ) {
            let PermissionedCallOriginData {
                primary_did,
                secondary_key,
                ..
            } = Identity::<T>::ensure_origin_call_permissions(origin)?;
            Self::ensure_user_portfolio_permission(secondary_key.as_ref(), PortfolioId::user_portfolio(primary_did, num))?;
            Self::ensure_user_portfolio_validity(primary_did, num)?;
            Self::ensure_name_unique(&primary_did, &to_name)?;
            Portfolios::mutate(&primary_did, &num, |p| *p = to_name.clone());
            Self::deposit_event(RawEvent::PortfolioRenamed(
                primary_did,
                num,
                to_name,
            ));
        }
    }
}

impl<T: Trait> Module<T> {
    /// Returns the ticker balance of the identity's default portfolio.
    pub fn default_portfolio_balance(
        did: IdentityId,
        ticker: &Ticker,
    ) -> <T as CommonTrait>::Balance {
        Self::portfolio_asset_balances(PortfolioId::default_portfolio(did), ticker)
    }

    /// Returns the ticker balance of an identity's user portfolio.
    pub fn user_portfolio_balance(
        did: IdentityId,
        num: PortfolioNumber,
        ticker: &Ticker,
    ) -> <T as CommonTrait>::Balance {
        Self::portfolio_asset_balances(PortfolioId::user_portfolio(did, num), ticker)
    }

    /// Sets the ticker balance of the identity's default portfolio to the given value.
    pub fn set_default_portfolio_balance(
        did: IdentityId,
        ticker: &Ticker,
        balance: <T as CommonTrait>::Balance,
    ) {
        <PortfolioAssetBalances<T>>::insert(PortfolioId::default_portfolio(did), ticker, balance);
    }

    /// Returns the next portfolio number of a given identity and increments the stored number.
    fn get_next_portfolio_number(did: &IdentityId) -> PortfolioNumber {
        NextPortfolioNumber::mutate(did, |num| mem::replace(num, PortfolioNumber(num.0 + 1)))
    }

    /// Ensures that there is no portfolio with the desired `name` yet.
    fn ensure_name_unique(did: &IdentityId, name: &PortfolioName) -> DispatchResult {
        let name_uniq = Portfolios::iter_prefix(&did).all(|n| &n.1 != name);
        ensure!(name_uniq, Error::<T>::PortfolioNameAlreadyInUse);
        Ok(())
    }

    /// Transfers some funds from one portfolio to another.
    /// This function does not do any data validity checks.
    /// The caller must make sure that the portfolio, custodianship and free balance are valid before calling this function.
    pub fn unchecked_transfer_portfolio_balance(
        from_portfolio: &PortfolioId,
        to_portfolio: &PortfolioId,
        ticker: &Ticker,
        amount: <T as CommonTrait>::Balance,
    ) {
        <PortfolioAssetBalances<T>>::mutate(from_portfolio, ticker, |from_balance| {
            *from_balance = from_balance.saturating_sub(amount)
        });

        <PortfolioAssetBalances<T>>::mutate(to_portfolio, ticker, |to_balance| {
            *to_balance = to_balance.saturating_add(amount)
        });
    }

    /// Ensure that the `portfolio` exists.
    pub fn ensure_portfolio_validity(portfolio: &PortfolioId) -> DispatchResult {
        // Default portfolio are always valid. Custom portfolios must be created explicitly.
        if let PortfolioKind::User(num) = portfolio.kind {
            Self::ensure_user_portfolio_validity(portfolio.did, num)?;
        }
        Ok(())
    }

    /// Ensure that the `PortfolioNumber` is valid.
    fn ensure_user_portfolio_validity(did: IdentityId, num: PortfolioNumber) -> DispatchResult {
        ensure!(
            Portfolios::contains_key(did, num),
            Error::<T>::PortfolioDoesNotExist
        );
        Ok(())
    }

    /// Ensure that the `secondary_key` has access to `portfolio`.
    pub fn ensure_user_portfolio_permission(
        secondary_key: Option<&SecondaryKey<T::AccountId>>,
        portfolio: PortfolioId,
    ) -> DispatchResult {
        // If `sk` is None, caller is primary key and has full permissions.
        if let Some(sk) = secondary_key {
            // Check that the secondary signer is allowed to work with this portfolio.
            ensure!(
                sk.has_portfolio_permission(iter::once(portfolio)),
                Error::<T>::SecondaryKeyNotAuthorizedForPortfolio
            );
        }
        Ok(())
    }

    /// Makes sure that the portfolio's custody is with the provided identity
    pub fn ensure_portfolio_custody(
        portfolio: PortfolioId,
        custodian: IdentityId,
    ) -> DispatchResult {
        // If a custodian is assigned, only they are allowed.
        // Else, only the portfolio owner is allowed
        // TODO: support portfolio permissions
        ensure!(
            Self::portfolio_custodian(portfolio).unwrap_or(portfolio.did) == custodian,
            Error::<T>::UnauthorizedCustodian
        );

        Ok(())
    }

    /// Makes sure that a portfolio transfer is valid. Portfolio access is not checked.
    pub fn ensure_portfolio_transfer_validity(
        from_portfolio: &PortfolioId,
        to_portfolio: &PortfolioId,
        ticker: &Ticker,
        amount: &<T as CommonTrait>::Balance,
    ) -> DispatchResult {
        // 1. Ensure from and to portfolio are different
        ensure!(
            from_portfolio != to_portfolio,
            Error::<T>::DestinationIsSamePortfolio
        );

        // 2. Ensure that the portfolios exist
        Self::ensure_portfolio_validity(from_portfolio)?;
        Self::ensure_portfolio_validity(to_portfolio)?;

        // 3. Ensure sender has enough free balance
        Self::ensure_sufficient_balance(&from_portfolio, ticker, amount)
    }

    /// Reduces the balance of a portfolio.
    /// Throws an error if enough free balance is not available.
    pub fn reduce_portfolio_balance(
        portfolio: &PortfolioId,
        ticker: &Ticker,
        amount: &<T as CommonTrait>::Balance,
    ) -> DispatchResult {
        // Ensure portfolio has enough free balance
        let total_balance = Self::portfolio_asset_balances(&portfolio, ticker);
        let locked_balance = Self::locked_assets(&portfolio, ticker);
        let remaining_balance = total_balance
            .checked_sub(amount)
            .filter(|rb| rb >= &locked_balance)
            .ok_or(Error::<T>::InsufficientPortfolioBalance)?;

        // Update portfolio balance
        <PortfolioAssetBalances<T>>::insert(portfolio, ticker, remaining_balance);

        Ok(())
    }

    /// Ensures that the portfolio's custody is with the provided identity
    /// And the secondary key has the relevant portfolio permission
    pub fn ensure_portfolio_custody_and_permission(
        portfolio: PortfolioId,
        custodian: IdentityId,
        secondary_key: Option<&SecondaryKey<T::AccountId>>,
    ) -> DispatchResult {
        Self::ensure_portfolio_custody(portfolio, custodian)?;
        Self::ensure_user_portfolio_permission(secondary_key, portfolio)
    }

    /// Ensure `portfolio` has sufficient balance of `ticker` to lock/withdraw `amount`.
    pub fn ensure_sufficient_balance(
        portfolio: &PortfolioId,
        ticker: &Ticker,
        amount: &T::Balance,
    ) -> DispatchResult {
        Self::portfolio_asset_balances(portfolio, ticker)
            .saturating_sub(Self::locked_assets(portfolio, ticker))
            .checked_sub(&amount)
            .ok_or_else(|| Error::<T>::InsufficientPortfolioBalance.into())
            .map(drop)
    }

    /// Locks `amount` of `ticker` in `portfolio` without checking that this is sane.
    ///
    /// Locks are stacked so if there were X tokens already locked, there will now be X + N tokens locked
    pub fn unchecked_lock_tokens(portfolio: &PortfolioId, ticker: &Ticker, amount: &T::Balance) {
        <PortfolioLockedAssets<T>>::mutate(portfolio, ticker, |l| *l = l.saturating_add(*amount));
    }
}

impl<T: Trait> PortfolioSubTrait<T::Balance, T::AccountId> for Module<T> {
    /// Accepts custody of a portfolio. The authorization must have been issued by the current custodian.
    ///
    /// # Errors
    /// * `AuthorizationError::Invalid` if auth_id reference an invalid authorization id
    /// * `AuthorizationError::Unauthorized` if identity who created the authorization is not the current custodian
    /// * `AuthorizationError::Expired` if the authorization has expired
    /// * `IrrelevantAuthorization` if the authorization is for something other than a portfolio custody
    fn accept_portfolio_custody(new_custodian: IdentityId, auth_id: u64) -> DispatchResult {
        ensure!(
            <identity::Authorizations<T>>::contains_key(Signatory::from(new_custodian), auth_id),
            AuthorizationError::Invalid
        );

        let auth = <identity::Authorizations<T>>::get(Signatory::from(new_custodian), auth_id);

        let portfolio_id = match auth.authorization_data {
            AuthorizationData::PortfolioCustody(pid) => pid,
            // Since this function is only called by the Identity pallet after making this check, this line will never be triggered.
            // Being defensive here anyway since we might want to expose this function via other forms some day.
            _ => return Err(Error::<T>::IrrelevantAuthorization.into()),
        };

        let current_custodian = PortfolioCustodian::get(&portfolio_id).unwrap_or(portfolio_id.did);

        <identity::Module<T>>::consume_auth(
            current_custodian,
            Signatory::from(new_custodian),
            auth_id,
        )?;

        // Transfer custody of `portfolio_id` over to `new_custodian`, removing it from `current_custodian`.
        PortfolioCustodian::insert(&portfolio_id, new_custodian);
        PortfoliosInCustody::remove(&current_custodian, &portfolio_id);
        PortfoliosInCustody::insert(&new_custodian, &portfolio_id, true);

        Self::deposit_event(RawEvent::PortfolioCustodianChanged(
            new_custodian,
            portfolio_id,
            new_custodian,
        ));
        Ok(())
    }

    /// Locks some user tokens so that they can not be used for transfers.
    /// This is used internally by the settlement engine to prevent users from using the same funds
    /// in multiple ongoing settlements
    ///
    /// # Errors
    /// * `InsufficientPortfolioBalance` if the portfolio does not have enough free balance to lock
    fn lock_tokens(
        portfolio: &PortfolioId,
        ticker: &Ticker,
        amount: &T::Balance,
    ) -> DispatchResult {
        Self::ensure_sufficient_balance(portfolio, ticker, amount)?;
        Self::unchecked_lock_tokens(portfolio, ticker, amount);
        Ok(())
    }

    /// Unlocks some locked tokens of a user.
    /// Since this is only ever called by the settlement engine,
    /// it will never be called under circumstances when it has to return an error.
    /// We are being defensive with the checks anyway.
    ///
    /// # Errors
    /// * `InsufficientTokensLocked` if the portfolio does not have enough locked tokens to unlock
    fn unlock_tokens(
        portfolio: &PortfolioId,
        ticker: &Ticker,
        amount: &T::Balance,
    ) -> DispatchResult {
        // 1. Ensure portfolio has enough locked tokens
        let locked = Self::locked_assets(portfolio, ticker);
        ensure!(locked >= *amount, Error::<T>::InsufficientTokensLocked);

        // 2. Unlock tokens. Can not underflow due to above ensure.
        <PortfolioLockedAssets<T>>::insert(portfolio, ticker, locked - *amount);
        Ok(())
    }

    /// Ensures that the portfolio's custody is with the provided identity
    fn ensure_portfolio_custody(portfolio: PortfolioId, custodian: IdentityId) -> DispatchResult {
        Self::ensure_portfolio_custody(portfolio, custodian)
    }

    /// Ensures that the portfolio's custody is with the provided identity
    /// And the secondary key has the relevant portfolio permission
    fn ensure_portfolio_custody_and_permission(
        portfolio: PortfolioId,
        custodian: IdentityId,
        secondary_key: Option<&SecondaryKey<T::AccountId>>,
    ) -> DispatchResult {
        Self::ensure_portfolio_custody_and_permission(portfolio, custodian, secondary_key)
    }
}
