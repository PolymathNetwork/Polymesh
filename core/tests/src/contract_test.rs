use frame_support::{
    assert_err, assert_ok, dispatch::DispatchResultWithPostInfo, weights::GetDispatchInfo,
    StorageMap,
};
use pallet_contracts::{ContractAddressFor, ContractInfoOf, Gas};
use sp_runtime::{traits::Hash, Perbill};

use crate::{
    ext_builder::MockProtocolBaseFees,
    storage::{
        account_from, make_account_with_uid, make_account_without_cdd, AccountId, TestStorage,
    },
    ExtBuilder,
};
use codec::Encode;
use hex_literal::hex;
use pallet_balances as balances;
use pallet_permissions as permissions;
use polymesh_common_utilities::{protocol_fee::ProtocolOp, traits::CddAndFeeDetails};
use polymesh_contracts::MetadataOfTemplate;
use polymesh_contracts::{Call as ContractsCall, NonceBasedAddressDeterminer};
use polymesh_primitives::{
    IdentityId, InvestorUid, SmartExtensionType, TemplateDetails, TemplateMetadata,
};
use test_client::AccountKeyring;

const GAS_LIMIT: Gas = 10_000_000_000;

type Balances = balances::Module<TestStorage>;
type System = frame_system::Module<TestStorage>;
type WrapperContracts = polymesh_contracts::Module<TestStorage>;
type Origin = <TestStorage as frame_system::Trait>::Origin;
type Contracts = pallet_contracts::Module<TestStorage>;
type WrapperContractsError = polymesh_contracts::Error<TestStorage>;
type ProtocolFeeError = pallet_protocol_fee::Error<TestStorage>;
type PermissionError = permissions::Error<TestStorage>;

/// Load a given wasm module represented by a .wat file and returns a wasm binary contents along
/// with it's hash.
///
/// The fixture files are located under the `fixtures/` directory.
pub fn compile_module<T>(fixture_name: &str) -> wat::Result<(Vec<u8>, <T::Hashing as Hash>::Output)>
where
    T: frame_system::Trait,
{
    let fixture_path = ["fixtures/", fixture_name, ".wat"].concat();
    let wasm_binary = wat::parse_file(fixture_path)?;
    let code_hash = T::Hashing::hash(&wasm_binary);
    Ok((wasm_binary, code_hash))
}

pub fn create_se_template<T>(
    template_creator: AccountId,
    template_creator_did: IdentityId,
    instantiation_fee: u128,
    code_hash: <T::Hashing as Hash>::Output,
    wasm: Vec<u8>,
) where
    T: frame_system::Trait<Hash = sp_core::H256>,
{
    let wasm_length_weight = 1426500000;

    // Set payer in context
    TestStorage::set_payer_context(Some(template_creator));

    // Create smart extension metadata
    let se_meta_data = TemplateMetadata {
        url: None,
        se_type: SmartExtensionType::TransferManager,
        usage_fee: 0,
        description: "This is a transfer manager type contract".into(),
        version: 5000,
    };

    // verify the weight value of the put_code extrinsic.
    let weight_of_extrinsic = ContractsCall::<TestStorage>::put_code(
        se_meta_data.clone(),
        instantiation_fee,
        wasm.clone(),
    )
    .get_dispatch_info()
    .weight;
    assert_eq!(wasm_length_weight + 50_000_000, weight_of_extrinsic);

    // Execute `put_code`
    assert_ok!(WrapperContracts::put_code(
        Origin::signed(template_creator),
        se_meta_data.clone(),
        instantiation_fee,
        wasm
    ));

    // Expected data provide by the runtime.
    let expected_template_metadata = TemplateDetails {
        instantiation_fee: instantiation_fee,
        owner: template_creator_did,
        frozen: false,
    };

    // Verify the storage
    assert_eq!(
        WrapperContracts::get_template_details(code_hash),
        expected_template_metadata
    );

    assert_eq!(WrapperContracts::get_metadata_of(code_hash), se_meta_data);

    // Set payer in context
    TestStorage::set_payer_context(None);
}

pub fn create_contract_instance<T>(
    instance_creator: AccountId,
    code_hash: <T::Hashing as Hash>::Output,
    max_fee: u128,
    fail: bool,
) -> DispatchResultWithPostInfo
where
    T: frame_system::Trait<Hash = sp_core::H256>,
{
    let input_data = hex!("0222FF18");
    // Set payer of the transaction
    TestStorage::set_payer_context(Some(instance_creator));

    // Access the extension nonce.
    let current_extension_nonce = WrapperContracts::extension_nonce();

    // create a instance
    let result = WrapperContracts::instantiate(
        Origin::signed(instance_creator),
        100,
        GAS_LIMIT,
        code_hash,
        input_data.to_vec(),
        max_fee,
    );

    if !fail {
        assert_eq!(
            WrapperContracts::extension_nonce(),
            current_extension_nonce + 1
        );
    }

    // Free up the context
    TestStorage::set_payer_context(None);
    result
}

fn get_wrong_code_hash<T>() -> <T::Hashing as Hash>::Output
where
    T: frame_system::Trait<Hash = sp_core::H256>,
{
    T::Hashing::hash(&b"abc".encode())
}

#[test]
fn check_put_code_functionality() {
    // Build wasm and get code_hash
    let (wasm, code_hash) = compile_module::<TestStorage>("flipper").unwrap();
    let protocol_fee = MockProtocolBaseFees(vec![(ProtocolOp::ContractsPutCode, 500)]);

    ExtBuilder::default()
        .network_fee_share(Perbill::from_percent(0))
        .set_protocol_base_fees(protocol_fee)
        .build()
        .execute_with(|| {
            let alice = AccountKeyring::Alice.public();
            // Create Alice account & the identity for her.
            let (_, alice_did) = make_account_without_cdd(alice).unwrap();

            // Get the balance of the Alice
            let alice_balance = System::account(alice).data.free;

            create_se_template::<TestStorage>(alice, alice_did, 0, code_hash, wasm);

            // Check the storage of the base pallet
            assert!(<pallet_contracts::PristineCode<TestStorage>>::get(code_hash).is_some());

            // Check for fee
            let fee_deducted = <pallet_protocol_fee::Module<TestStorage>>::compute_fee(&[
                ProtocolOp::ContractsPutCode,
            ]);

            // Check for protocol fee deduction
            let current_alice_balance = System::account(alice).data.free;
            assert_eq!(current_alice_balance, alice_balance - fee_deducted);

            // Balance of fee collector
            let balance_of_gainer = System::account(account_from(5000)).data.free;
            assert_eq!(balance_of_gainer, fee_deducted);

            // Free up the context.
            TestStorage::set_payer_context(None);
        });
}

#[test]
fn check_instantiation_functionality() {
    // Build wasm and get code_hash
    let (wasm, code_hash) = compile_module::<TestStorage>("flipper").unwrap();
    let protocol_fee = MockProtocolBaseFees(vec![(ProtocolOp::ContractsPutCode, 500)]);

    ExtBuilder::default()
        .network_fee_share(Perbill::from_percent(0))
        .set_protocol_base_fees(protocol_fee)
        .build()
        .execute_with(|| {
            let input_data = hex!("0222FF18");
            let extrinsic_wrapper_weight = 500_000_000;
            let instantiation_fee = 99999;

            let alice = AccountKeyring::Alice.public();
            // Create Alice account & the identity for her.
            let (_, alice_did) = make_account_without_cdd(alice).unwrap();

            create_se_template::<TestStorage>(alice, alice_did, instantiation_fee, code_hash, wasm);

            // Get the balance of the Alice
            let alice_balance = System::account(alice).data.free;

            // Bob will create a instance of it.
            let bob = AccountKeyring::Bob.public();
            // Create Alice account & the identity for her.
            let (_, _) = make_account_without_cdd(bob).unwrap();

            // Get the balance of the Bob
            let bob_balance = System::account(bob).data.free;

            // create instance of contract
            let result =
                create_contract_instance::<TestStorage>(bob, code_hash, instantiation_fee, false);

            assert_ok!(result);
            // Verify the actual weight of the extrinsic.
            assert!(result.unwrap().actual_weight.unwrap() > extrinsic_wrapper_weight);

            // Verify whether the instantiation fee deducted properly or not.
            // Alice balance should increased by `instantiation_fee` and Bob balance should be decreased by the same amount.
            let new_alice_balance = System::account(alice).data.free;
            let new_bob_balance = System::account(bob).data.free;

            assert_eq!(bob_balance - new_bob_balance, instantiation_fee + 100); // 100 for instantiation.
            assert_eq!(alice_balance + instantiation_fee, new_alice_balance);

            // Generate the contract address.
            let flipper_address_1 =
                NonceBasedAddressDeterminer::<TestStorage>::contract_address_for(
                    &code_hash,
                    &input_data.to_vec(),
                    &bob,
                );

            // Check whether the contract creation allowed or not with same constructor data.
            // It should be as contract creation is depend on the nonce of the account.

            let result =
                create_contract_instance::<TestStorage>(bob, code_hash, instantiation_fee, false);
            assert_ok!(result);

            // Generate the contract address.
            let flipper_address_2 =
                NonceBasedAddressDeterminer::<TestStorage>::contract_address_for(
                    &code_hash,
                    &input_data.to_vec(),
                    &bob,
                );

            // verify that contract address is different.
            assert!(flipper_address_1 != flipper_address_2);
        });
}

#[test]
fn allow_network_share_deduction() {
    // Build wasm and get code_hash
    let (wasm, code_hash) = compile_module::<TestStorage>("flipper").unwrap();
    let protocol_fee = MockProtocolBaseFees(vec![(ProtocolOp::ContractsPutCode, 500)]);

    ExtBuilder::default()
        .network_fee_share(Perbill::from_percent(25))
        .set_protocol_base_fees(protocol_fee)
        .build()
        .execute_with(|| {
            let instantiation_fee = 5000;
            let fee_collector = account_from(5000);
            let alice = AccountKeyring::Alice.public();
            // Create Alice account & the identity for her.
            let (_, alice_did) = make_account_without_cdd(alice).unwrap();

            // Bob will create a instance of it.
            let bob = AccountKeyring::Bob.public();
            // Create Alice account & the identity for her.
            make_account_without_cdd(bob).unwrap();

            // Create template of se
            create_se_template::<TestStorage>(alice, alice_did, instantiation_fee, code_hash, wasm);

            // Get the balance of Alice
            let alice_balance = System::account(alice).data.free;
            // Get Network fee collector balance
            let fee_collector_balance = System::account(fee_collector).data.free;

            // create instance of contract
            assert_ok!(create_contract_instance::<TestStorage>(
                bob,
                code_hash,
                instantiation_fee,
                false
            ));

            // check the fee division
            // 25 % of fee should be consumed by the network and 75% should be transferred to template owner.
            let new_alice_balance = System::account(alice).data.free;
            let new_fee_collector_balance = System::account(fee_collector).data.free;
            // 75% check
            assert_eq!(
                alice_balance.saturating_add(Perbill::from_percent(75) * instantiation_fee),
                new_alice_balance
            );
            // 25% check
            assert_eq!(
                fee_collector_balance.saturating_add(Perbill::from_percent(25) * instantiation_fee),
                new_fee_collector_balance
            );
        });
}

#[test]
fn check_behavior_when_instantiation_fee_changes() {
    // Build wasm and get code_hash
    let (wasm, code_hash) = compile_module::<TestStorage>("flipper").unwrap();

    ExtBuilder::default()
        .network_fee_share(Perbill::from_percent(30))
        .build()
        .execute_with(|| {
            let instantiation_fee = 5000;
            let fee_collector = account_from(5000);
            let alice = AccountKeyring::Alice.public();
            // Create Alice account & the identity for her.
            let (_, alice_did) = make_account_without_cdd(alice).unwrap();

            // Bob will create a instance of it.
            let bob = AccountKeyring::Bob.public();
            // Create Alice account & the identity for her.
            make_account_without_cdd(bob).unwrap();

            // Create template of se
            create_se_template::<TestStorage>(alice, alice_did, instantiation_fee, code_hash, wasm);

            let new_instantiation_fee = 8000;

            // Change instantiation fee of the template
            // Should fail because provide hash doesn't exists
            assert_err!(
                WrapperContracts::change_template_fees(
                    Origin::signed(alice),
                    get_wrong_code_hash::<TestStorage>(),
                    Some(new_instantiation_fee),
                    None,
                ),
                WrapperContractsError::TemplateNotExists
            );

            // Should fail as sender is not the template owner
            assert_err!(
                WrapperContracts::change_template_fees(
                    Origin::signed(AccountKeyring::Bob.public()),
                    code_hash,
                    Some(new_instantiation_fee),
                    None,
                ),
                WrapperContractsError::UnAuthorizedOrigin
            );

            let old_template_fee =
                WrapperContracts::get_template_details(code_hash).instantiation_fee;

            // No change when None is passed.
            assert_ok!(WrapperContracts::change_template_fees(
                Origin::signed(alice),
                code_hash,
                None,
                None,
            ));

            assert_eq!(
                WrapperContracts::get_template_details(code_hash).instantiation_fee,
                old_template_fee
            );

            // Should success fully change the instantiation fee
            assert_ok!(WrapperContracts::change_template_fees(
                Origin::signed(alice),
                code_hash,
                Some(new_instantiation_fee),
                None,
            ));

            // Verify the storage changes
            assert_eq!(
                WrapperContracts::get_template_details(code_hash).instantiation_fee,
                new_instantiation_fee
            );

            // Get the balance of Alice
            let alice_balance = System::account(alice).data.free;
            // Get Network fee collector balance
            let fee_collector_balance = System::account(fee_collector).data.free;

            // create instance of contract
            assert_ok!(create_contract_instance::<TestStorage>(
                bob,
                code_hash,
                new_instantiation_fee,
                false
            ));

            // check the fee division
            // 30 % of fee should be consumed by the network and 70% should be transferred to template owner.
            let new_alice_balance = System::account(alice).data.free;
            let new_fee_collector_balance = System::account(fee_collector).data.free;
            // 70% check
            assert_eq!(
                alice_balance.saturating_add(Perbill::from_percent(70) * new_instantiation_fee),
                new_alice_balance
            );
            // 30% check
            assert_eq!(
                fee_collector_balance
                    .saturating_add(Perbill::from_percent(30) * new_instantiation_fee),
                new_fee_collector_balance
            );
        });
}

#[test]
fn check_freeze_unfreeze_functionality() {
    // Build wasm and get code_hash
    let (wasm, code_hash) = compile_module::<TestStorage>("flipper").unwrap();

    ExtBuilder::default()
        .network_fee_share(Perbill::from_percent(30))
        .build()
        .execute_with(|| {
            let instantiation_fee = 5000;
            let alice = AccountKeyring::Alice.public();
            // Create Alice account & the identity for her.
            let (alice_signed, alice_did) = make_account_without_cdd(alice).unwrap();

            // Bob will create a instance of it.
            let bob = AccountKeyring::Bob.public();
            // Create Alice account & the identity for her.
            make_account_without_cdd(bob).unwrap();

            // Create template of se
            create_se_template::<TestStorage>(alice, alice_did, instantiation_fee, code_hash, wasm);

            // Check whether freeze functionality is working or not
            // successfully freeze the instantiation of the SE template
            assert_ok!(WrapperContracts::freeze_instantiation(
                alice_signed.clone(),
                code_hash
            ));

            // Verify the storage
            assert!(WrapperContracts::get_template_details(code_hash).frozen);

            // Should fail when trying to freeze the template again
            assert_err!(
                WrapperContracts::freeze_instantiation(alice_signed.clone(), code_hash),
                WrapperContractsError::InstantiationAlreadyFrozen
            );

            // Instantiation should fail
            assert_err!(
                create_contract_instance::<TestStorage>(bob, code_hash, instantiation_fee, true),
                WrapperContractsError::InstantiationIsNotAllowed
            );

            // check unfreeze functionality

            // successfully unfreeze the instantiation of the SE template
            assert_ok!(WrapperContracts::unfreeze_instantiation(
                alice_signed.clone(),
                code_hash
            ));

            // Verify the storage
            assert!(!WrapperContracts::get_template_details(code_hash).frozen);

            // Should fail when trying to unfreeze the template again
            assert_err!(
                WrapperContracts::unfreeze_instantiation(alice_signed, code_hash),
                WrapperContractsError::InstantiationAlreadyUnFrozen
            );

            // Instantiation should fail if we max_fee is less than the instantiation fee.
            assert_err!(
                create_contract_instance::<TestStorage>(bob, code_hash, 500, true),
                WrapperContractsError::InsufficientMaxFee
            );

            // Instantiation should passed
            assert_ok!(create_contract_instance::<TestStorage>(
                bob,
                code_hash,
                instantiation_fee,
                false
            ));
        });
}

#[test]
fn validate_transfer_template_ownership_functionality() {
    // Build wasm and get code_hash
    let (wasm, code_hash) = compile_module::<TestStorage>("flipper").unwrap();

    ExtBuilder::default()
        .network_fee_share(Perbill::from_percent(30))
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let instantiation_fee = 5000;
            let alice = AccountKeyring::Alice.public();
            // Create Alice account & the identity for her.
            let (alice_signed, alice_did) = make_account_without_cdd(alice).unwrap();

            // Bob will create a instance of it.
            let bob = AccountKeyring::Bob.public();
            // Create Bob account & the identity for her.
            let bob_uid = InvestorUid::from("bob_take_1");
            let (_, bob_did) = make_account_with_uid(bob, bob_uid).unwrap();

            // Create template of se
            create_se_template::<TestStorage>(alice, alice_did, instantiation_fee, code_hash, wasm);

            // Call the transfer ownership functionality
            // Should fail because provided identityId doesn't has the CDD
            assert_err!(
                WrapperContracts::transfer_template_ownership(
                    alice_signed.clone(),
                    code_hash,
                    IdentityId::from(2)
                ),
                WrapperContractsError::NewOwnerIsNotCDD
            );

            // Not a valid sender.
            assert_err!(
                WrapperContracts::transfer_template_ownership(
                    Origin::signed(account_from(45)),
                    code_hash,
                    bob_did
                ),
                PermissionError::UnauthorizedCaller
            );

            // Successfully transfer ownership to the other DID.
            assert_ok!(WrapperContracts::transfer_template_ownership(
                alice_signed,
                code_hash,
                bob_did
            ));

            assert_eq!(
                WrapperContracts::get_template_details(code_hash).owner,
                bob_did
            );
        });
}

#[test]
fn check_transaction_rollback_functionality_for_put_code() {
    // Build wasm and get code_hash
    let (wasm, code_hash) = compile_module::<TestStorage>("flipper").unwrap();
    let protocol_fee = MockProtocolBaseFees(vec![(ProtocolOp::ContractsPutCode, 900000000)]);

    ExtBuilder::default()
        .network_fee_share(Perbill::from_percent(30))
        .set_protocol_base_fees(protocol_fee)
        .build()
        .execute_with(|| {
            let instantiation_fee = 5000;
            let alice = AccountKeyring::Alice.public();
            // Create Alice account & the identity for her.
            let (alice_signed, _) = make_account_without_cdd(alice).unwrap();

            // Set payer in context
            TestStorage::set_payer_context(Some(alice));

            // Create smart extension metadata
            let se_meta_data = TemplateMetadata {
                url: None,
                se_type: SmartExtensionType::TransferManager,
                usage_fee: 0,
                description: "This is a transfer manager type contract".into(),
                version: 5000,
            };

            // Execute `put_code`
            assert_err!(
                WrapperContracts::put_code(
                    alice_signed,
                    se_meta_data.clone(),
                    instantiation_fee,
                    wasm
                ),
                ProtocolFeeError::InsufficientAccountBalance
            );

            // Verify that storage doesn't change.
            assert!(!MetadataOfTemplate::<TestStorage>::contains_key(code_hash));
            assert!(<pallet_contracts::PristineCode<TestStorage>>::get(code_hash).is_none())
        });
}

#[test]
fn check_transaction_rollback_functionality_for_instantiation() {
    // Build wasm and get code_hash
    let (wasm, code_hash) = compile_module::<TestStorage>("flipper").unwrap();
    let protocol_fee = MockProtocolBaseFees(vec![(ProtocolOp::ContractsPutCode, 500)]);

    ExtBuilder::default()
        .network_fee_share(Perbill::from_percent(30))
        .set_protocol_base_fees(protocol_fee)
        .build()
        .execute_with(|| {
            let input_data = hex!("0222FF18");
            let instantiation_fee = 10000000000;
            let alice = AccountKeyring::Alice.public();
            // Create Alice account & the identity for her.
            let (_, alice_did) = make_account_without_cdd(alice).unwrap();

            // Bob will create a instance of it.
            let bob = AccountKeyring::Bob.public();
            // Create Alice account & the identity for her.
            make_account_without_cdd(bob).unwrap();

            // Create template of se
            create_se_template::<TestStorage>(alice, alice_did, instantiation_fee, code_hash, wasm);

            // create instance of contract
            assert_err!(
                create_contract_instance::<TestStorage>(bob, code_hash, instantiation_fee, true),
                ProtocolFeeError::InsufficientAccountBalance
            );

            // Generate the contract address.
            let flipper_address_1 =
                NonceBasedAddressDeterminer::<TestStorage>::contract_address_for(
                    &code_hash,
                    &input_data.to_vec(),
                    &bob,
                );

            assert!(!ContractInfoOf::<TestStorage>::contains_key(
                flipper_address_1
            ));
        });
}

#[test]
fn check_meta_url_functionality() {
    // Build wasm and get code_hash
    let (wasm, code_hash) = compile_module::<TestStorage>("flipper").unwrap();
    let protocol_fee = MockProtocolBaseFees(vec![(ProtocolOp::ContractsPutCode, 500)]);

    ExtBuilder::default()
        .network_fee_share(Perbill::from_percent(30))
        .set_protocol_base_fees(protocol_fee)
        .build()
        .execute_with(|| {
            let instantiation_fee = 10000000000;
            let alice = AccountKeyring::Alice.public();
            // Create Alice account & the identity for her.
            let (alice_signed, alice_did) = make_account_without_cdd(alice).unwrap();

            // Bob will create a instance of it.
            let bob = AccountKeyring::Bob.public();
            // Create Alice account & the identity for her.
            make_account_without_cdd(bob).unwrap();

            // Create template of se
            create_se_template::<TestStorage>(alice, alice_did, instantiation_fee, code_hash, wasm);

            // Change the meta url.

            assert_ok!(WrapperContracts::change_template_meta_url(
                alice_signed,
                code_hash,
                Some("http://www.google.com".into())
            ));
        });
}
