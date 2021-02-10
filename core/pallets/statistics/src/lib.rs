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

#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

use codec::{Decode, Encode};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure,
    traits::Get, weights::Weight,
};
use polymesh_common_utilities::{asset::AssetFnTrait, identity::Trait as IdentityTrait};
use polymesh_primitives::{IdentityId, ScopeId, Ticker};
use sp_arithmetic::Permill;
#[cfg(feature = "std")]
use sp_runtime::{Deserialize, Serialize};
use sp_std::vec::Vec;

pub type Counter = u64;
pub type Percentage = Permill;

/// Transfer managers that can be attached to a Token for compliance.
#[derive(Eq, PartialEq, Clone, Encode, Decode, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TransferManager {
    /// CTM limits the number of active investors in a Token.
    CountTransferManager(Counter),
    /// PTM limits the percentage of token owned by a single Identity.
    PercentageTransferManager(Percentage),
}

/// The main trait for statistics module
pub trait Trait: frame_system::Trait + IdentityTrait {
    /// The overarching event type.
    type Event: From<Event> + Into<<Self as frame_system::Trait>::Event>;
    /// Asset module
    type Asset: AssetFnTrait<Self::Balance, Self::AccountId, Self::Origin>;
    /// Maximum transfer managers that can be enabled for an Asset
    type MaxTransferManagersPerAsset: Get<u32>;
    /// Weights for extrinsics
    type WeightInfo: WeightInfo;
}

/// Weight info for extrinsics
pub trait WeightInfo {
    fn add_transfer_manager() -> Weight;
    fn remove_transfer_manager() -> Weight;
    fn add_exempted_entities(i: u32) -> Weight;
    fn remove_exempted_entities(i: u32) -> Weight;
}

decl_storage! {
    trait Store for Module<T: Trait> as statistics {
        /// Transfer managers currently enabled for an Asset.
        pub ActiveTransferManagers get(fn transfer_managers): map hasher(blake2_128_concat) Ticker => Vec<TransferManager>;
        /// Number of current investors in an asset.
        pub InvestorCountPerAsset get(fn investor_count): map hasher(blake2_128_concat) Ticker => Counter;
        /// Entities exempt from transfer managers. Exemptions requirements are based on TMS.
        /// TMs may require just the sender, just the receiver, both or either to be exempted.
        /// CTM requires sender to be exempted while PTM requires receiver to be exempted.
        pub ExemptEntities get(fn entity_exempt):
            double_map
                hasher(blake2_128_concat) (Ticker, TransferManager),
                hasher(blake2_128_concat) ScopeId
            =>
                bool;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        /// initialize the default event for this module
        fn deposit_event() = default;

        /// Adds a new transfer manager.
        ///
        /// # Arguments
        /// * `origin` It contains the secondary key of the caller (i.e who signed the transaction to execute this function).
        /// * `ticker` ticker for which the transfer managers are being updated.
        /// * `new_transfer_manager` Transfer manager being added.
        ///
        /// # Errors
        /// * `Unauthorized` if `origin` is not the owner of the ticker.
        /// * `DuplicateTransferManager` if `new_transfer_manager` is already enabled for the ticker.
        /// * `TransferManagersLimitReached` if the `ticker` already has max TMs attached
        ///
        #[weight = <T as Trait>::WeightInfo::add_transfer_manager()]
        pub fn add_transfer_manager(origin, ticker: Ticker, new_transfer_manager: TransferManager) {
            let did = T::Asset::ensure_perms_owner_asset(origin, &ticker)?;
            ActiveTransferManagers::try_mutate(&ticker, |transfer_managers| {
                ensure!((transfer_managers.len() as u32) < T::MaxTransferManagersPerAsset::get(), Error::<T>::TransferManagersLimitReached);
                ensure!(!transfer_managers.contains(&new_transfer_manager), Error::<T>::DuplicateTransferManager);
                transfer_managers.push(new_transfer_manager.clone());
                Ok(()) as DispatchResult
            })?;
            Self::deposit_event(Event::TransferManagerAdded(did, ticker, new_transfer_manager));
        }

        /// Removes a transfer manager.
        ///
        /// # Arguments
        /// * `origin` It contains the secondary key of the caller (i.e who signed the transaction to execute this function).
        /// * `ticker` ticker for which the transfer managers are being updated.
        /// * `transfer_manager` Transfer manager being removed.
        ///
        /// # Errors
        /// * `Unauthorized` if `origin` is not the owner of the ticker.
        /// * `TransferManagerMissing` if `asset_compliance` contains multiple entries with the same `requirement_id`.
        ///
        #[weight = <T as Trait>::WeightInfo::remove_transfer_manager()]
        pub fn remove_transfer_manager(origin, ticker: Ticker, transfer_manager: TransferManager) {
            let did = T::Asset::ensure_perms_owner_asset(origin, &ticker)?;
            ActiveTransferManagers::try_mutate(&ticker, |transfer_managers| {
                let before = transfer_managers.len();
                transfer_managers.retain(|tm| *tm != transfer_manager);
                ensure!(before != transfer_managers.len(), Error::<T>::TransferManagerMissing);
                Ok(()) as DispatchResult
            })?;
            Self::deposit_event(Event::TransferManagerRemoved(did, ticker, transfer_manager));
        }

        /// Exempt entities from a transfer manager
        ///
        /// # Arguments
        /// * `origin` It contains the secondary key of the caller (i.e who signed the transaction to execute this function).
        /// * `ticker` ticker for which the exemption list is being modified.
        /// * `transfer_manager` Transfer manager for which the exemption list is being modified.
        /// * `exempted_entities` ScopeIds for which the exemption list is being modified.
        ///
        /// # Errors
        /// * `Unauthorized` if `origin` is not the owner of the ticker.
        ///
        #[weight = <T as Trait>::WeightInfo::add_exempted_entities(exempted_entities.len() as u32)]
        pub fn add_exempted_entities(origin, ticker: Ticker, transfer_manager: TransferManager, exempted_entities: Vec<ScopeId>) {
            let did = T::Asset::ensure_perms_owner_asset(origin, &ticker)?;
            let ticker_tm = (ticker, transfer_manager.clone());
            for entity in &exempted_entities {
                ExemptEntities::insert(&ticker_tm, entity, true);
            }
            Self::deposit_event(Event::ExemptionsAdded(did, ticker, transfer_manager, exempted_entities));
        }

        /// remove entities from exemption list of a transfer manager
        ///
        /// # Arguments
        /// * `origin` It contains the secondary key of the caller (i.e who signed the transaction to execute this function).
        /// * `ticker` ticker for which the exemption list is being modified.
        /// * `transfer_manager` Transfer manager for which the exemption list is being modified.
        /// * `scope_ids` ScopeIds for which the exemption list is being modified.
        ///
        /// # Errors
        /// * `Unauthorized` if `origin` is not the owner of the ticker.
        ///
        #[weight = <T as Trait>::WeightInfo::remove_exempted_entities(entities.len() as u32)]
        pub fn remove_exempted_entities(origin, ticker: Ticker, transfer_manager: TransferManager, entities: Vec<ScopeId>) {
            let did = T::Asset::ensure_perms_owner_asset(origin, &ticker)?;
            let ticker_tm = (ticker, transfer_manager.clone());
            for entity in &entities {
                ExemptEntities::remove(&ticker_tm, entity);
            }
            Self::deposit_event(Event::ExemptionsRemoved(did, ticker, transfer_manager, entities));
        }
    }
}

impl<T: Trait> Module<T> {
    /// It updates our statistics after transfer execution.
    /// The following counters could be updated:
    ///     - *Investor count per asset*.
    ///
    pub fn update_transfer_stats(
        ticker: &Ticker,
        updated_from_balance: Option<T::Balance>,
        updated_to_balance: Option<T::Balance>,
        amount: T::Balance,
    ) {
        // 1. Unique investor count per asset.
        if amount != 0u128.into() {
            let counter = Self::investor_count(ticker);
            let mut new_counter = counter;

            if let Some(from_balance) = updated_from_balance {
                if from_balance == 0u128.into() {
                    new_counter = new_counter.checked_sub(1).unwrap_or(new_counter);
                }
            }

            if let Some(to_balance) = updated_to_balance {
                if to_balance == amount {
                    new_counter = new_counter.checked_add(1).unwrap_or(new_counter);
                }
            }

            // Only updates extrinsics if counter has been changed.
            if new_counter != counter {
                InvestorCountPerAsset::insert(ticker, new_counter)
            }
        }
    }

    /// Verify transfer restrictions for a transfer
    pub fn verify_tm_restrictions(
        ticker: &Ticker,
        sender: ScopeId,
        receiver: ScopeId,
        value: T::Balance,
        sender_balance: T::Balance,
        receiver_balance: T::Balance,
        total_supply: T::Balance,
    ) -> DispatchResult {
        Self::transfer_managers(ticker)
            .into_iter()
            .try_for_each(|tm| match tm {
                TransferManager::CountTransferManager(max_count) => Self::ensure_ctm(
                    ticker,
                    sender,
                    value,
                    sender_balance,
                    receiver_balance,
                    max_count,
                ),
                TransferManager::PercentageTransferManager(max_percentage) => Self::ensure_ptm(
                    ticker,
                    receiver,
                    value,
                    receiver_balance,
                    total_supply,
                    max_percentage,
                ),
            })
    }

    fn ensure_ctm(
        ticker: &Ticker,
        sender: ScopeId,
        value: T::Balance,
        sender_balance: T::Balance,
        receiver_balance: T::Balance,
        max_count: Counter,
    ) -> DispatchResult {
        let current_count = Self::investor_count(ticker);
        ensure!(
            current_count < max_count
                || sender_balance == value
                || receiver_balance > 0u32.into()
                || Self::entity_exempt(
                    (*ticker, TransferManager::CountTransferManager(max_count)),
                    sender
                ),
            Error::<T>::InvalidTransfer
        );
        Ok(())
    }

    fn ensure_ptm(
        ticker: &Ticker,
        receiver: ScopeId,
        value: T::Balance,
        receiver_balance: T::Balance,
        total_supply: T::Balance,
        max_percentage: Percentage,
    ) -> DispatchResult {
        let new_percentage =
            Permill::from_rational_approximation(receiver_balance + value, total_supply);
        ensure!(
            new_percentage <= max_percentage
                || Self::entity_exempt(
                    (
                        *ticker,
                        TransferManager::PercentageTransferManager(max_percentage)
                    ),
                    receiver
                ),
            Error::<T>::InvalidTransfer
        );
        Ok(())
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub fn set_investor_count(ticker: &Ticker, count: Counter) {
        InvestorCountPerAsset::insert(ticker, count);
    }
}

decl_event!(
    pub enum Event {
        /// A new transfer manager was added.
        TransferManagerAdded(IdentityId, Ticker, TransferManager),
        /// An existing transfer manager was removed.
        TransferManagerRemoved(IdentityId, Ticker, TransferManager),
        /// `ScopeId`s were added to the exemption list.
        ExemptionsAdded(IdentityId, Ticker, TransferManager, Vec<ScopeId>),
        /// `ScopeId`s were removed from the exemption list.
        ExemptionsRemoved(IdentityId, Ticker, TransferManager, Vec<ScopeId>),
    }
);

decl_error! {
    /// Statistics module errors.
    pub enum Error for Module<T: Trait> {
        /// The transfer manager already exists
        DuplicateTransferManager,
        /// Transfer manager is not enabled
        TransferManagerMissing,
        /// Transfer not allowed
        InvalidTransfer,
        /// The limit of transfer managers allowed for an asset has been reached
        TransferManagersLimitReached
    }
}
