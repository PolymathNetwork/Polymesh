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

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

use codec::Encode;
use core::mem;
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult, DispatchResultWithPostInfo},
    ensure,
    traits::Get,
    weights::Weight,
};
use pallet_contracts::{BalanceOf, CodeHash, ContractAddressFor, Gas, Schedule};
use pallet_identity as identity;
use polymesh_common_utilities::{
    identity::Trait as IdentityTrait,
    protocol_fee::{ChargeProtocolFee, ProtocolOp},
    with_transaction,
};
use polymesh_primitives::{
    ExtensionAttributes, IdentityId, MetaUrl, TemplateDetails, TemplateMetadata,
};
use sp_core::crypto::UncheckedFrom;
use sp_runtime::{
    traits::{Hash, Saturating, StaticLookup},
    Perbill, SaturatedConversion,
};
use sp_std::{marker::PhantomData, prelude::*};

type Identity<T> = identity::Module<T>;

/// Nonce based contract address determiner.
///
/// Address calculated from the code (of the constructor), input data to the constructor,
/// the account id that requested the account creation and the latest nonce of the account id.
///
/// Formula: `blake2_256(blake2_256(code) + blake2_256(data) + origin + blake2_256(nonce))`
pub struct NonceBasedAddressDeterminer<T: Trait>(PhantomData<T>);
impl<T: Trait> ContractAddressFor<CodeHash<T>, T::AccountId> for NonceBasedAddressDeterminer<T>
where
    T::AccountId: UncheckedFrom<T::Hash> + AsRef<[u8]>,
{
    fn contract_address_for(
        code_hash: &CodeHash<T>,
        data: &[u8],
        origin: &T::AccountId,
    ) -> T::AccountId {
        let data_hash = T::Hashing::hash(data);
        let nonce = ExtensionNonce::get();
        let nonce_hash = T::Hashing::hash(&(nonce.encode()));
        let mut buf = Vec::with_capacity(
            code_hash.as_ref().len()
                + data_hash.as_ref().len()
                + origin.as_ref().len()
                + nonce_hash.as_ref().len(),
        );
        buf.extend_from_slice(code_hash.as_ref());
        buf.extend_from_slice(data_hash.as_ref());
        buf.extend_from_slice(origin.as_ref());
        buf.extend_from_slice(nonce_hash.as_ref());

        UncheckedFrom::unchecked_from(T::Hashing::hash(&buf[..]))
    }
}

pub trait WeightInfo {
    fn put_code(l: u32, u: u32, d: u32) -> Weight;
    fn instantiate() -> Weight;
    fn freeze_instantiation() -> Weight;
    fn unfreeze_instantiation() -> Weight;
    fn transfer_template_ownership() -> Weight;
    fn change_template_fees() -> Weight;
    fn change_template_meta_url(u: u32) -> Weight;
    fn update_schedule() -> Weight;
}

pub trait Trait: pallet_contracts::Trait + IdentityTrait {
    /// Event type
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// Percentage distribution of instantiation fee to the validators and treasury.
    type NetworkShareInFee: Get<Perbill>;
    /// Weight information for extrinsic in this pallet.
    type WeightInfo: WeightInfo;
}

decl_storage! {
    trait Store for Module<T: Trait> as ContractsWrapper {
        /// Store the meta details of the smart extension template.
        pub MetadataOfTemplate get(fn get_metadata_of): map hasher(identity) CodeHash<T> => TemplateMetadata<BalanceOf<T>>;
        /// Store the details of the template (Ex- owner, frozen etc).
        pub TemplateInfo get(fn get_template_details): map hasher(identity) CodeHash<T> => TemplateDetails<BalanceOf<T>>;
        /// Details of extension get updated.
        pub ExtensionInfo get(fn extension_info): map hasher(identity) T::AccountId => ExtensionAttributes<BalanceOf<T>>;
        /// Nonce for the smart extension account id generation.
        /// Using explicit nonce as in batch transaction accounts nonce doesn't get incremented.
        pub ExtensionNonce get(fn extension_nonce): u64;
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// Instantiation is not allowed.
        InstantiationIsNotAllowed,
        /// Smart extension template not exist in the storage.
        TemplateNotExists,
        /// When instantiation of the template is already frozen.
        InstantiationAlreadyFrozen,
        /// When instantiation of the template is already un-frozen.
        InstantiationAlreadyUnFrozen,
        /// When un-authorized personnel try to access the un-authorized extrinsic.
        UnAuthorizedOrigin,
        /// Given identityId is not CDD.
        NewOwnerIsNotCDD,
        /// Insufficient max_fee provided by the user to instantiate the SE.
        InsufficientMaxFee,
    }
}

decl_event! {
    pub enum Event<T>
        where
        Balance = BalanceOf<T>,
        CodeHash = <T as frame_system::Trait>::Hash,
    {
        /// Emitted when instantiation fee of a template get changed.
        /// IdentityId of the owner, Code hash of the template, old instantiation fee, new instantiation fee.
        InstantiationFeeChanged(IdentityId, CodeHash, Balance, Balance),
        /// Emitted when the instantiation of the template get frozen.
        /// IdentityId of the owner, Code hash of the template.
        InstantiationFreezed(IdentityId, CodeHash),
        /// Emitted when the instantiation of the template gets un-frozen.
        /// IdentityId of the owner, Code hash of the template.
        InstantiationUnFreezed(IdentityId, CodeHash),
        /// Emitted when the template ownership get transferred.
        /// IdentityId of the owner, Code hash of the template, IdentityId of the new owner of the template.
        TemplateOwnershipTransferred(IdentityId, CodeHash, IdentityId),
        /// Emitted when the template usage fees gets changed.
        /// IdentityId of the owner, Code hash of the template,Old usage fee, New usage fee.
        TemplateUsageFeeChanged(IdentityId, CodeHash, Balance, Balance),
        /// Emitted when the template instantiation fees gets changed.
        /// IdentityId of the owner, Code hash of the template, Old instantiation fee, New instantiation fee.
        TemplateInstantiationFeeChanged(IdentityId, CodeHash, Balance, Balance),
        /// Emitted when the template meta url get changed.
        /// IdentityId of the owner, Code hash of the template, old meta url, new meta url.
        TemplateMetaUrlChanged(IdentityId, CodeHash, Option<MetaUrl>, Option<MetaUrl>),
    }
}

decl_module! {
    // Wrap dispatchable functions for contracts so that we can add additional gating logic.
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {

        /// Initialize the default event for this module.
        fn deposit_event() = default;

        /// Error type.
        type Error = Error<T>;

        /// The minimum amount required to generate a tombstone.
        const NetworkShareInInstantiationFee: Perbill = T::NetworkShareInFee::get();

        // Simply forwards to the `update_schedule` function in the Contract module.
        #[weight = pallet_contracts::Call::<T>::update_schedule(schedule.clone()).get_dispatch_info().weight]
        pub fn update_schedule(origin, schedule: Schedule) -> DispatchResult {
            <pallet_contracts::Module<T>>::update_schedule(origin, schedule)
        }

        /// Simply forwards to the `put_code` function in the Contract module.
        ///
        /// # Additional functionality
        /// 1. Allow origin to pass some meta-details related to template code.
        /// 2. Charge protocol fee for deploying the template.
        #[weight = 50_000_000.saturating_add(pallet_contracts::Call::<T>::put_code(code.to_vec()).get_dispatch_info().weight)]
        pub fn put_code(
            origin,
            meta_info: TemplateMetadata<BalanceOf<T>>,
            instantiation_fee: BalanceOf<T>,
            code: Vec<u8>
        ) {
            let did = Identity::<T>::ensure_perms(origin.clone())?;

            // Save metadata related to the SE template
            // Generate the code_hash here as well because there is no way
            // to read it directly from the upstream `pallet-contracts` module.
            let code_hash = T::Hashing::hash(&code);

            // Rollback the `put_code()` if user is not able to pay the protocol-fee.
            with_transaction(|| {
                // Call underlying function
                <pallet_contracts::Module<T>>::put_code(origin, code)?;
                // Update the storage.
                <TemplateInfo<T>>::insert(code_hash, TemplateDetails {
                    instantiation_fee,
                    owner: did,
                    frozen: false
                });
                <MetadataOfTemplate<T>>::insert(code_hash, meta_info);
                // Charge the protocol fee
                T::ProtocolFee::charge_fee(ProtocolOp::ContractsPutCode)
            })?;
        }

        // Simply forwards to the `call` function in the Contract module.
        #[weight = pallet_contracts::Call::<T>::call(dest.clone(), *value, *gas_limit, data.clone()).get_dispatch_info().weight]
        pub fn call(
            origin,
            dest: <T::Lookup as StaticLookup>::Source,
            #[compact] value: BalanceOf<T>,
            #[compact] gas_limit: Gas,
            data: Vec<u8>
        ) -> DispatchResultWithPostInfo {
            <pallet_contracts::Module<T>>::call(origin, dest, value, gas_limit, data)
        }

        /// Simply forwards to the `instantiate` function in the Contract module.
        ///
        /// # Additional functionality
        /// 1. Check whether instantiation of given code_hash is allowed or not.
        /// 2. Charge instantiation fee.
        ///
        /// # Errors
        /// InstantiationIsNotAllowed - It occurred when instantiation of the template is frozen.
        /// InsufficientMaxFee - Provided max_fee is less than required.
        #[weight = <T as Trait>::WeightInfo::instantiate().saturating_add(*gas_limit)]
        pub fn instantiate(
            origin,
            #[compact] endowment: BalanceOf<T>,
            #[compact] gas_limit: Gas,
            code_hash: CodeHash<T>,
            data: Vec<u8>,
            max_fee: BalanceOf<T>
        ) -> DispatchResultWithPostInfo {
            let sender = Identity::<T>::ensure_origin_call_permissions(origin.clone())?.sender;
            // Access the meta details of SE template
            let template_details = Self::get_template_details(code_hash);

            // Check whether instantiation is allowed or not.
            ensure!(!template_details.is_instantiation_frozen(), Error::<T>::InstantiationIsNotAllowed);

            // Check instantiation fee should be <= max_fee.
            let instantiation_fee = template_details.get_instantiation_fee();
            ensure!(instantiation_fee <= max_fee, Error::<T>::InsufficientMaxFee);

            let mut actual_weight = None;
            let mut contract_address = T::AccountId::default();

            with_transaction(|| {
                // Update the extension nonce.
                ExtensionNonce::mutate(|n| *n = *n + 1u64);
                // Generate the contract address. Generating here to avoid cloning of the vec.
                contract_address = T::DetermineContractAddress::contract_address_for(&code_hash, &data, &sender);
                // transmit the call to the base `pallet-contracts` module.
                actual_weight = <pallet_contracts::Module<T>>::instantiate(origin, endowment, gas_limit, code_hash, data).map(|post_info| post_info.actual_weight).map_err(|err| err.error)?;
                // Charge instantiation fee
                T::ProtocolFee::charge_extension_instantiation_fee((instantiation_fee.saturated_into::<u128>()).into(), Self::get_primary_key(&template_details.owner), T::NetworkShareInFee::get())
            })?;

            // Update the usage fee for the extension instance.
            <ExtensionInfo<T>>::insert(contract_address, Self::ext_details(&code_hash));

            // Update the actual weight of the extrinsic.
            Ok(actual_weight.map(|w| w.saturating_add(500_000_000)).into())
        }

        /// Allows a smart extension template owner to freeze the instantiation.
        ///
        /// # Arguments
        /// * origin - Only owner of the template is allowed to execute the dispatchable.
        /// * code_hash - Unique hash of the smart extension template.
        #[weight = <T as Trait>::WeightInfo::freeze_instantiation()]
        pub fn freeze_instantiation(origin, code_hash: CodeHash<T>) -> DispatchResult {
            // Ensure whether the extrinsic is signed & validate the `code_hash`.
            let (did, template_details) = Self::ensure_signed_and_template_exists(origin, code_hash)?;

            // If instantiation is already frozen then there is no point of changing the storage value.
            ensure!(!template_details.is_instantiation_frozen(), Error::<T>::InstantiationAlreadyFrozen);
            // Change the `frozen` variable to `true`.
            <TemplateInfo<T>>::mutate(&code_hash, |template_details| template_details.frozen = true);

            // Emit event.
            Self::deposit_event(RawEvent::InstantiationFreezed(did, code_hash));
            Ok(())
        }

        /// Allows a smart extension template owner to un freeze the instantiation.
        ///
        /// # Arguments
        /// * origin - Only owner of the template is allowed to execute the dispatchable.
        /// * code_hash - Unique hash of the smart extension template.
        #[weight = <T as Trait>::WeightInfo::unfreeze_instantiation()]
        pub fn unfreeze_instantiation(origin, code_hash: CodeHash<T>) -> DispatchResult {
            // Ensure whether the extrinsic is signed & validate the `code_hash`.
            let (did, template_details) = Self::ensure_signed_and_template_exists(origin, code_hash)?;

            // If instantiation is already un-frozen then there is no point of changing the storage value.
            ensure!(template_details.is_instantiation_frozen(), Error::<T>::InstantiationAlreadyUnFrozen);
            // Change the `frozen` variable to `false`.
            <TemplateInfo<T>>::mutate(&code_hash, |template_details| template_details.frozen = false);

            // Emit event.
            Self::deposit_event(RawEvent::InstantiationUnFreezed(did, code_hash));
            Ok(())
        }

        /// Transfer ownership of the template, Can only be called by the owner of template.
        /// `new_owner` should posses the valid CDD claim.
        ///
        /// # Arguments
        /// * origin Owner of the provided code_hash.
        /// * code_hash Unique identifer of the template.
        /// * new_owner Identity that will be the new owner of the provided code_hash.
        #[weight = <T as Trait>::WeightInfo::transfer_template_ownership()]
        pub fn transfer_template_ownership(origin, code_hash: CodeHash<T>, new_owner: IdentityId) -> DispatchResult {
            // Ensure whether the extrinsic is signed & validate the `code_hash`.
            let (did, _) = Self::ensure_signed_and_template_exists(origin, code_hash)?;

            // Ensuring the `new_owner` is CDD or not.
            ensure!(Identity::<T>::has_valid_cdd(new_owner), Error::<T>::NewOwnerIsNotCDD);

            // Change the `owner` variable value to the given did.
            <TemplateInfo<T>>::mutate(&code_hash, |template_details| template_details.owner = new_owner);

            // Emit event.
            Self::deposit_event(RawEvent::TemplateOwnershipTransferred(did, code_hash, new_owner));
            Ok(())
        }


        /// Change the usage fee & the instantiation fee of the smart extension template
        ///
        /// # Arguments
        /// * origin - Only owner of template is allowed to execute the dispatchable.
        /// * code_hash - Unique hash of the smart extension template.
        /// * new_instantiation_fee - New value of instantiation fee for the smart extension template.
        /// * new_usage_fee - New value of usage fee for the smart extension template.
        #[weight = <T as Trait>::WeightInfo::change_template_fees()]
        pub fn change_template_fees(origin, code_hash: CodeHash<T>, new_instantiation_fee: Option<BalanceOf<T>>, new_usage_fee: Option<BalanceOf<T>>) -> DispatchResult {
            // Ensure whether the extrinsic is signed & validate the `code_hash`.
            let (did, _) = Self::ensure_signed_and_template_exists(origin, code_hash)?;

            // Update the fees
            if let Some(usage_fee) = new_usage_fee {
                // Update the usage fee for a given code hash.
                let old_usage_fee = <MetadataOfTemplate<T>>::mutate(&code_hash, |metadata| mem::replace(&mut metadata.usage_fee, usage_fee));
                // Emit event with the old & new usage fee.
                Self::deposit_event(RawEvent::TemplateUsageFeeChanged(did, code_hash, old_usage_fee, usage_fee));
            }
            if let Some(instantiation_fee) = new_instantiation_fee {
                // Update the instantiation fee for a given code_hash.
                let old_instantiation_fee = <TemplateInfo<T>>::mutate(&code_hash, |template_details| mem::replace(&mut template_details.instantiation_fee, instantiation_fee));
                // Emit event with the old & new instantiation fee.
                Self::deposit_event(RawEvent::TemplateInstantiationFeeChanged(did, code_hash, old_instantiation_fee, instantiation_fee));
            }
            Ok(())
        }


        /// Change the template meta url.
        ///
        /// # Arguments
        /// * origin - Only owner of template is allowed to execute the dispatchable.
        /// * code_hash - Unique hash of the smart extension template.
        /// * new_url - New meta url that need to replace with old url.
        #[weight = <T as Trait>::WeightInfo::change_template_meta_url(new_url.as_ref().map_or(0, |u| u.0.len()) as u32 )]
        pub fn change_template_meta_url(origin, code_hash: CodeHash<T>, new_url: Option<MetaUrl>) -> DispatchResult {
            // Ensure whether the extrinsic is signed & validate the `code_hash`.
            let (did, _) = Self::ensure_signed_and_template_exists(origin, code_hash)?;
            // Update the usage fee for a given code hash.
            let old_url = <MetadataOfTemplate<T>>::mutate(&code_hash, |metadata| mem::replace(&mut metadata.url, new_url.clone()));
            // Emit event with old and new url.
            Self::deposit_event(RawEvent::TemplateMetaUrlChanged(did, code_hash, old_url, new_url));
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    // Internal function
    // Perform some basic sanity checks
    fn ensure_signed_and_template_exists(
        origin: T::Origin,
        code_hash: CodeHash<T>,
    ) -> Result<(IdentityId, TemplateDetails<BalanceOf<T>>), DispatchError> {
        // Ensure the transaction is signed and ensure `origin` has the required permission to
        // execute the dispatchable.
        let did = Identity::<T>::ensure_perms(origin)?;
        // Validate whether the template exists or not for a given code_hash.
        ensure!(
            <TemplateInfo<T>>::contains_key(code_hash),
            Error::<T>::TemplateNotExists
        );

        // Access the meta details of SE template
        let template_details = Self::get_template_details(code_hash);
        // Ensure sender's DID is the owner of the template.
        ensure!(
            did == template_details.owner,
            Error::<T>::UnAuthorizedOrigin
        );
        // Return the DID and the template details.
        Ok((did, template_details))
    }

    fn get_primary_key(id: &IdentityId) -> T::AccountId {
        Identity::<T>::did_records(id).primary_key
    }

    fn ext_details(code_hash: &CodeHash<T>) -> ExtensionAttributes<BalanceOf<T>> {
        let meta_info = Self::get_metadata_of(code_hash);
        ExtensionAttributes {
            usage_fee: meta_info.usage_fee,
            version: meta_info.version,
        }
    }
}
