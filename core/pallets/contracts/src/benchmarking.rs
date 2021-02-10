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
use frame_support::storage::IterableStorageMap;
use frame_system::RawOrigin;
use pallet_contracts::PristineCode;
use parity_wasm::elements::FuncBody;
use polymesh_common_utilities::benchs::UserBuilder;
use polymesh_primitives::{MetaDescription, MetaUrl, SmartExtensionType, TemplateMetadata};
use sp_runtime::traits::Hash;

type BaseContracts<T> = pallet_contracts::Module<T>;

const MAX_URL_LENGTH: u32 = 100000;
const MAX_DESCRIPTION_LENGTH: u32 = 100000;

// Copied from - https://github.com/paritytech/substrate/blob/v2.0.0/frame/contracts/src/benchmarking.rs#L30
macro_rules! load_module {
    ($name:expr) => {{
        let code = include_bytes!(concat!("../fixtures/", $name, ".wat"));
        compile_module::<T>(code)
    }};
}

// Copied from - https://github.com/paritytech/substrate/blob/v2.0.0/frame/contracts/src/benchmarking.rs#L37
pub fn compile_module<T: Trait>(code: &[u8]) -> (Vec<u8>, <T::Hashing as Hash>::Output) {
    let code = sp_std::str::from_utf8(code).expect("Invalid utf8 in wat file.");
    let binary = wat::parse_str(code).expect("Failed to compile wat file.");
    let hash = T::Hashing::hash(&binary);
    (binary, hash)
}

// Copied from - https://github.com/paritytech/substrate/blob/v2.0.0/frame/contracts/src/benchmarking.rs#L54
fn contract_with_call_body<T: Trait>(body: FuncBody) -> (Vec<u8>, <T::Hashing as Hash>::Output) {
    use parity_wasm::elements::{Instruction::End, Instructions};
    let contract = parity_wasm::builder::ModuleBuilder::new()
        // deploy function (idx 0)
        .function()
        .signature()
        .with_params(vec![])
        .with_return_type(None)
        .build()
        .body()
        .with_instructions(Instructions::new(vec![End]))
        .build()
        .build()
        // call function (idx 1)
        .function()
        .signature()
        .with_params(vec![])
        .with_return_type(None)
        .build()
        .with_body(body)
        .build()
        .export()
        .field("deploy")
        .internal()
        .func(0)
        .build()
        .export()
        .field("call")
        .internal()
        .func(1)
        .build()
        .build();
    let bytes = contract.to_bytes().unwrap();
    let hash = T::Hashing::hash(&bytes);
    (bytes, hash)
}

// Copied from - https://github.com/paritytech/substrate/blob/v2.0.0/frame/contracts/src/benchmarking.rs#L77
fn expanded_contract<T: Trait>(target_bytes: u32) -> (Vec<u8>, <T::Hashing as Hash>::Output) {
    use parity_wasm::elements::{
        BlockType,
        Instruction::{self, End, I32Const, If, Return},
        Instructions,
    };
    // Base size of a contract is 47 bytes and each expansion adds 6 bytes.
    // We do one expansion less to account for the code section and function body
    // size fields inside the binary wasm module representation which are leb128 encoded
    // and therefore grow in size when the contract grows. We are not allowed to overshoot
    // because of the maximum code size that is enforced by `put_code`.
    let expansions = (target_bytes.saturating_sub(47) / 6).saturating_sub(1) as usize;
    const EXPANSION: [Instruction; 4] = [I32Const(0), If(BlockType::NoResult), Return, End];
    let instructions = Instructions::new(
        EXPANSION
            .iter()
            .cycle()
            .take(EXPANSION.len() * expansions)
            .cloned()
            .chain(sp_std::iter::once(End))
            .collect(),
    );
    contract_with_call_body::<T>(FuncBody::new(Vec::new(), instructions))
}

pub fn emulate_blueprint_in_storage<T: Trait>(
    instantiation_fee: u32,
    origin: RawOrigin<T::AccountId>,
    expanded: &'static str,
) -> Result<<T::Hashing as Hash>::Output, DispatchError> {
    let url = Some(MetaUrl::from(
        vec![b'U'; MAX_URL_LENGTH as usize].as_slice(),
    ));
    let description = MetaDescription::from(vec![b'D'; MAX_DESCRIPTION_LENGTH as usize].as_slice());
    let meta_info = TemplateMetadata {
        url,
        se_type: SmartExtensionType::TransferManager,
        usage_fee: 100u32.into(),
        description,
        version: 5000,
    };
    let (wasm_blob, code_hash) = if expanded == "" {
        expanded_contract::<T>(BaseContracts::<T>::current_schedule().max_code_size)
    } else {
        if expanded == "dummy" {
            load_module!("dummy")
        } else {
            load_module!("ptm")
        }
    };
    Module::<T>::put_code(
        origin.into(),
        meta_info,
        instantiation_fee.into(),
        wasm_blob,
    )?;
    Ok(code_hash)
}

benchmarks! {
    _{}

    put_code {
        // Catalyst for the code size length
        let l in 1 .. BaseContracts::<T>::current_schedule().max_code_size;
        // Catalyst for the MetaUrl length.
        let u in 1 .. MAX_URL_LENGTH;
        // Catalyst for the MetaDescription length.
        let d in 1 .. MAX_DESCRIPTION_LENGTH;

        let url = Some(MetaUrl::from(vec![b'U'; u as usize].as_slice()));
        let description = MetaDescription::from(vec![b'D'; d as usize].as_slice());
        let meta_info = TemplateMetadata {
            url,
            se_type: SmartExtensionType::TransferManager,
            usage_fee: 100u32.into(),
            description,
            version: 5000
        };
        let (wasm_blob, code_hash) = expanded_contract::<T>(l);
        let user = UserBuilder::<T>::default().generate_did().build("creator");
    }: _(user.origin, meta_info, 1000u32.into(), wasm_blob)
    verify {
        ensure!(matches!(Module::<T>::get_metadata_of(code_hash), meta_info), "Contracts_putCode: Meta info set incorrect");
        ensure!(PristineCode::<T>::get(code_hash).is_some(), "Contracts_putCode: Base contract doesn't get updated with given code hash");
    }

    // No catalyst.
    instantiate {
        let data = vec![0u8; 128];
        let max_fee = 100;
        let creator = UserBuilder::<T>::default().generate_did().build("creator");
        let code_hash = emulate_blueprint_in_storage::<T>(max_fee, creator.origin, "dummy")?;
        let deployer = UserBuilder::<T>::default().generate_did().build("deployer");
    }: _(deployer.origin, 1_000_000u32.into(), Weight::max_value(), code_hash, data, max_fee.into())
    verify {
        let (key, value) = ExtensionInfo::<T>::iter().next().unwrap();
        let attributes = Module::<T>::ext_details(&code_hash);
        ensure!(matches!(value, attributes), "Contracts_instantiate: Storage doesn't set correctly");
    }

    // No catalyst.
    freeze_instantiation {
        let creator = UserBuilder::<T>::default().generate_did().build("creator");
        let code_hash = emulate_blueprint_in_storage::<T>(100, creator.origin.clone(), "dummy")?;
    }: _(creator.origin, code_hash)
    verify {
        ensure!(Module::<T>::get_template_details(code_hash).is_instantiation_frozen(), "Contracts_freeze_instantiation: Failed to freeze instantiation");
    }

    // No catalyst.
    unfreeze_instantiation {
        let creator = UserBuilder::<T>::default().generate_did().build("creator");
        let code_hash = emulate_blueprint_in_storage::<T>(100, creator.origin.clone(), "dummy")?;
        Module::<T>::freeze_instantiation(creator.origin.clone().into(), code_hash)?;
    }: _(creator.origin, code_hash)
    verify {
        ensure!(!Module::<T>::get_template_details(code_hash).is_instantiation_frozen(), "Contracts_unfreeze_instantiation: Failed to unfreeze instantiation");
    }

    // No catalyst.
    transfer_template_ownership {
        let creator = UserBuilder::<T>::default().generate_did().build("creator");
        let code_hash = emulate_blueprint_in_storage::<T>(100, creator.origin.clone(), "dummy")?;
        let new_owner = UserBuilder::<T>::default().generate_did().build("newOwner");
    }: _(creator.origin, code_hash, new_owner.did())
    verify {
        ensure!(Module::<T>::get_template_details(code_hash).owner == new_owner.did(), "Contracts_transfer_template_ownership: Failed to transfer ownership");
    }

    // No catalyst.
    change_template_fees {
        let creator = UserBuilder::<T>::default().generate_did().build("creator");
        let code_hash = emulate_blueprint_in_storage::<T>(100, creator.origin.clone(), "")?;
    }: _(creator.origin, code_hash, Some(500u32.into()), Some(650u32.into()))
    verify {
        ensure!(Module::<T>::get_template_details(code_hash).get_instantiation_fee() == 500u32.into(), "Contracts_change_template_fees: Failed to change the instantiation fees");
        ensure!(Module::<T>::get_metadata_of(code_hash).usage_fee == 650u32.into(), "Contracts_change_template_fees: Failed to change the usage fees");
    }

    change_template_meta_url {
        // Catalyst for the MetaUrl length.
        let u in 1 .. MAX_URL_LENGTH;
        let url = Some(MetaUrl::from(vec![b'U'; u as usize].as_slice()));
        let creator = UserBuilder::<T>::default().generate_did().build("creator");
        let code_hash = emulate_blueprint_in_storage::<T>(100, creator.origin.clone(), "")?;
    }: _(creator.origin, code_hash, url.clone())
    verify {
        ensure!(Module::<T>::get_metadata_of(code_hash).url == url, "Contracts_change_template_meta_url: Failed to change the url of template");
    }

    // No catalyst.
    update_schedule {
        let schedule = Schedule {
            version: 1,
            .. Default::default()
        };
    }: _(RawOrigin::Root, schedule)
}
