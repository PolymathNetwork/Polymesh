use super::{
    storage::{make_account_with_balance, register_keyring_account, TestStorage},
    ExtBuilder,
};

use confidential_identity::{compute_cdd_id, compute_scope_id};
use pallet_asset::{self as asset, SecurityToken};
use pallet_compliance_manager as compliance_manager;
use pallet_confidential as confidential;
use pallet_identity as identity;
use polymesh_common_utilities::constants::ERC1400_TRANSFER_SUCCESS;
use polymesh_primitives::{
    asset::AssetType, AssetIdentifier, Claim, IdentityId, InvestorUid, InvestorZKProofData,
    PortfolioId, Scope, Ticker,
};

use core::convert::TryFrom;
use frame_support::{assert_err, assert_ok};
use test_client::AccountKeyring;

type Identity = identity::Module<TestStorage>;
type IdentityError = identity::Error<TestStorage>;
type Asset = asset::Module<TestStorage>;
type AssetError = asset::Error<TestStorage>;
type Confidential = confidential::Module<TestStorage>;
type ComplianceManager = compliance_manager::Module<TestStorage>;
type Origin = <TestStorage as frame_system::Trait>::Origin;

#[test]
fn range_proof() {
    ExtBuilder::default().build().execute_with(range_proof_we);
}

fn range_proof_we() {
    let alice = AccountKeyring::Alice.public();
    let prover = AccountKeyring::Bob.public();
    let verifier = AccountKeyring::Charlie.public();
    let alice_id = register_keyring_account(AccountKeyring::Alice).unwrap();
    let prover_id = register_keyring_account(AccountKeyring::Bob).unwrap();
    let verifier_id = register_keyring_account(AccountKeyring::Charlie).unwrap();

    // 1. Alice creates her security token.
    let token = SecurityToken {
        name: "ALI_ST".as_bytes().to_owned().into(),
        owner_did: alice_id,
        total_supply: 1_000_000,
        divisible: true,
        asset_type: AssetType::default(),
        ..Default::default()
    };
    let identifiers = vec![AssetIdentifier::isin(*b"US0378331005").unwrap()];
    let ticker = Ticker::try_from(token.name.as_slice()).unwrap();

    assert_ok!(Asset::create_asset(
        Origin::signed(alice),
        token.name.clone(),
        ticker,
        token.total_supply,
        true,
        token.asset_type.clone(),
        identifiers.clone(),
        None,
    ));

    // 2. X add a range proof
    let secret_value = 42;
    assert_ok!(Confidential::add_range_proof(
        Origin::signed(prover),
        alice_id,
        ticker.clone(),
        secret_value,
    ));

    assert_ok!(Confidential::add_verify_range_proof(
        Origin::signed(verifier),
        alice_id,
        prover_id,
        ticker.clone()
    ));

    assert_eq!(
        Confidential::range_proof_verification((alice_id, ticker), verifier_id),
        true
    );
}

#[test]
fn scope_claims() {
    ExtBuilder::default()
        .monied(true)
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(scope_claims_we);
}

fn scope_claims_we() {
    let alice = AccountKeyring::Alice.public();
    let alice_id = register_keyring_account(AccountKeyring::Alice).unwrap();
    let investor = InvestorUid::from("inv_1");
    let inv_acc_1 = AccountKeyring::Bob.public();
    let (_, inv_did_1) = make_account_with_balance(inv_acc_1, investor, 1_000_000).unwrap();
    let inv_acc_2 = AccountKeyring::Charlie.public();
    let (_, inv_did_2) = make_account_with_balance(inv_acc_2, investor, 2_000_000).unwrap();
    let other_investor = InvestorUid::from("inv_2");
    let inv_acc_3 = AccountKeyring::Dave.public();
    let (_, inv_did_3) = make_account_with_balance(inv_acc_3, other_investor, 3_000_000).unwrap();

    // 1. Alice creates her ST and set up its compliance requirements.
    let st = SecurityToken {
        name: "ALI_ST".as_bytes().to_owned().into(),
        owner_did: alice_id,
        total_supply: 1_000_000,
        divisible: true,
        asset_type: AssetType::default(),
        ..Default::default()
    };
    let identifiers = vec![AssetIdentifier::isin(*b"US0378331005").unwrap()];
    let st_id = Ticker::try_from(st.name.as_slice()).unwrap();

    assert_ok!(Asset::create_asset(
        Origin::signed(alice),
        st.name.clone(),
        st_id,
        st.total_supply,
        true,
        st.asset_type.clone(),
        identifiers.clone(),
        None,
    ));

    // 2. Alice defines the asset compliance requirements.
    let st_scope = Scope::Identity(IdentityId::try_from(st_id.as_slice()).unwrap());
    assert_ok!(ComplianceManager::add_compliance_requirement(
        Origin::signed(alice),
        st_id,
        vec![],
        vec![]
    ));

    // 2. Investor adds its Confidential Scope claims.
    let scope_claim = InvestorZKProofData::make_scope_claim(&st_id.as_slice(), &investor);
    let scope_id = compute_scope_id(&scope_claim).compress().to_bytes().into();

    let inv_1_proof = InvestorZKProofData::new(&inv_did_1, &investor, &st_id);
    let cdd_claim_1 = InvestorZKProofData::make_cdd_claim(&inv_did_1, &investor);
    let cdd_id_1 = compute_cdd_id(&cdd_claim_1).compress().to_bytes().into();

    let conf_scope_claim_error = Claim::InvestorUniqueness(st_scope, scope_id, cdd_id_1);
    let conf_scope_claim_1 = Claim::InvestorUniqueness(st_id.into(), scope_id, cdd_id_1);

    assert_err!(
        Identity::add_investor_uniqueness_claim(
            Origin::signed(inv_acc_1),
            inv_did_1,
            conf_scope_claim_error.clone(),
            inv_1_proof.clone(),
            None
        ),
        IdentityError::InvalidScopeClaim
    );

    assert_ok!(Identity::add_investor_uniqueness_claim(
        Origin::signed(inv_acc_1),
        inv_did_1,
        conf_scope_claim_1.clone(),
        inv_1_proof.clone(),
        None
    ),);

    let inv_2_proof = InvestorZKProofData::new(&inv_did_2, &investor, &st_id);
    let cdd_claim_2 = InvestorZKProofData::make_cdd_claim(&inv_did_2, &investor);
    let cdd_id_2 = compute_cdd_id(&cdd_claim_2).compress().to_bytes().into();

    let conf_scope_claim_2 = Claim::InvestorUniqueness(st_id.into(), scope_id, cdd_id_2);
    assert_ok!(Identity::add_investor_uniqueness_claim(
        Origin::signed(inv_acc_2),
        inv_did_2,
        conf_scope_claim_2,
        inv_2_proof,
        None
    ));

    // 3. Transfer some tokens to Inv. 1 and 2.
    assert_eq!(Asset::balance_of(st_id, inv_did_1), 0);
    assert_ok!(Asset::unsafe_transfer(
        PortfolioId::default_portfolio(alice_id),
        PortfolioId::default_portfolio(inv_did_1),
        &st_id,
        10
    ));
    assert_eq!(Asset::balance_of(st_id, inv_did_1), 10);

    assert_eq!(Asset::balance_of(st_id, inv_did_2), 0);
    assert_ok!(Asset::unsafe_transfer(
        PortfolioId::default_portfolio(alice_id),
        PortfolioId::default_portfolio(inv_did_2),
        &st_id,
        20
    ));
    assert_eq!(Asset::balance_of(st_id, inv_did_2), 20);

    // 4. ERROR: Investor 2 cannot add a claim of the real investor.
    assert_err!(
        Identity::add_investor_uniqueness_claim(
            Origin::signed(inv_acc_3),
            inv_did_3,
            conf_scope_claim_1,
            inv_1_proof.clone(),
            None
        ),
        IdentityError::ConfidentialScopeClaimNotAllowed
    );

    // 5. ERROR: Replace the scope
    let st_2 = SecurityToken {
        name: "ALI2_ST".as_bytes().to_owned().into(),
        owner_did: alice_id,
        total_supply: 1_000_000,
        divisible: true,
        asset_type: AssetType::default(),
        ..Default::default()
    };
    let identifiers = vec![AssetIdentifier::isin(*b"US0378331005").unwrap()];
    let st2_id = Ticker::try_from(st_2.name.as_slice()).unwrap();

    assert_ok!(Asset::create_asset(
        Origin::signed(alice),
        st_2.name.clone(),
        st2_id,
        st_2.total_supply,
        true,
        st_2.asset_type.clone(),
        identifiers.clone(),
        None,
    ));

    let corrupted_scope_claim =
        InvestorZKProofData::make_scope_claim(&st2_id.as_slice(), &investor);
    let corrupted_scope_id = compute_scope_id(&corrupted_scope_claim)
        .compress()
        .to_bytes()
        .into();

    let conf_scope_claim_3 = Claim::InvestorUniqueness(st2_id.into(), corrupted_scope_id, cdd_id_1);
    assert_err!(
        Identity::add_investor_uniqueness_claim(
            Origin::signed(inv_acc_1),
            inv_did_1,
            conf_scope_claim_3.clone(),
            inv_1_proof.clone(),
            None
        ),
        IdentityError::InvalidScopeClaim
    );

    assert_ne!(
        Asset::_is_valid_transfer(
            &st2_id,
            PortfolioId::default_portfolio(alice_id),
            PortfolioId::default_portfolio(inv_did_1),
            10
        ),
        Ok(ERC1400_TRANSFER_SUCCESS)
    );
}
