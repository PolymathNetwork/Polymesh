use crate::{
    contract_test::{compile_module, create_contract_instance, create_se_template},
    ext_builder::{ExtBuilder, MockProtocolBaseFees},
    pips_test::assert_balance,
    storage::{
        add_secondary_key, make_account_without_cdd, provide_scope_claim,
        provide_scope_claim_to_multiple_parties, register_keyring_account,
        register_keyring_account_without_cdd, root, AccountId, Checkpoint, TestStorage, User,
    },
};
use chrono::prelude::Utc;
use frame_support::{
    assert_err, assert_noop, assert_ok, dispatch::DispatchError, IterableStorageMap,
    StorageDoubleMap, StorageMap,
};
use hex_literal::hex;
use ink_primitives::hash as FunctionSelectorHasher;
use pallet_asset::checkpoint::ScheduleSpec;
use pallet_asset::ethereum;
use pallet_asset::{
    self as asset,
    checkpoint::{ScheduleId, StoredSchedule},
    AssetOwnershipRelation, ClassicTickerImport, ClassicTickerRegistration, ClassicTickers,
    ScopeIdOf, SecurityToken, TickerRegistration, TickerRegistrationConfig, Tickers,
};
use pallet_balances as balances;
use pallet_compliance_manager as compliance_manager;
use pallet_contracts::ContractAddressFor;
use pallet_identity as identity;
use pallet_statistics as statistics;
use polymesh_common_utilities::{
    constants::*, protocol_fee::ProtocolOp, traits::balances::Memo, traits::CddAndFeeDetails as _,
    SystematicIssuers,
};
use polymesh_contracts::NonceBasedAddressDeterminer;
use polymesh_primitives::{
    asset::{AssetType, FundingRoundName},
    calendar::{
        CalendarPeriod, CalendarUnit, CheckpointId, CheckpointSchedule, FixedOrVariableCalendarUnit,
    },
    AssetIdentifier, AuthorizationData, Document, DocumentId, IdentityId, InvestorUid, Moment,
    PortfolioId, Signatory, SmartExtension, SmartExtensionType, Ticker,
};
use rand::Rng;
use sp_io::hashing::keccak_256;
use sp_runtime::AnySignature;
use std::convert::{TryFrom, TryInto};
use test_client::AccountKeyring;

type Identity = identity::Module<TestStorage>;
type Balances = balances::Module<TestStorage>;
type Asset = asset::Module<TestStorage>;
type Timestamp = pallet_timestamp::Module<TestStorage>;
type ComplianceManager = compliance_manager::Module<TestStorage>;
type Portfolio = pallet_portfolio::Module<TestStorage>;
type AssetError = asset::Error<TestStorage>;
type OffChainSignature = AnySignature;
type Origin = <TestStorage as frame_system::Trait>::Origin;
type DidRecords = identity::DidRecords<TestStorage>;
type Statistics = statistics::Module<TestStorage>;
type AssetGenesis = asset::GenesisConfig<TestStorage>;
type System = frame_system::Module<TestStorage>;
type FeeError = pallet_protocol_fee::Error<TestStorage>;
type PortfolioError = pallet_portfolio::Error<TestStorage>;

fn setup_se_template<T>(
    creator: AccountId,
    creator_did: IdentityId,
    create_instance: bool,
) -> AccountId
where
    T: frame_system::Trait<Hash = sp_core::H256>,
{
    let (wasm, code_hash) = compile_module::<TestStorage>("flipper").unwrap();

    let input_data = hex!("0222FF18");

    if create_instance {
        // Create SE template.
        create_se_template::<TestStorage>(creator, creator_did, 0, code_hash, wasm);
    }

    // Create SE instance.
    assert_ok!(create_contract_instance::<TestStorage>(
        creator, code_hash, 0, false
    ));

    NonceBasedAddressDeterminer::<TestStorage>::contract_address_for(
        &code_hash,
        &input_data.to_vec(),
        &creator,
    )
}

#[test]
fn check_the_test_hex() {
    ExtBuilder::default().build().execute_with(|| {
        let selector: [u8; 4] = (FunctionSelectorHasher::keccak256("verify_transfer".as_bytes())
            [0..4])
            .try_into()
            .unwrap();
        println!("{:#X}", u32::from_be_bytes(selector));
        let data = hex!("D9386E41");
        println!("{:?}", data);
    });
}

#[test]
fn issuers_can_create_and_rename_tokens() {
    ExtBuilder::default().build().execute_with(|| {
        let owner_signed = Origin::signed(AccountKeyring::Dave.public());
        let owner_did = register_keyring_account(AccountKeyring::Dave).unwrap();
        let funding_round_name: FundingRoundName = b"round1".into();
        // Expected token entry
        let token = SecurityToken {
            name: vec![b'A'].into(),
            owner_did,
            total_supply: 1_000_000,
            divisible: true,
            asset_type: AssetType::default(),
            primary_issuance_agent: None,
            ..Default::default()
        };
        let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
        assert!(!<DidRecords>::contains_key(
            Identity::get_token_did(&ticker).unwrap()
        ));
        let identifiers = Vec::new();
        let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
        assert_err!(
            Asset::create_asset(
                owner_signed.clone(),
                token.name.clone(),
                ticker,
                1_000_000_000_000_000_000_000_000, // Total supply over the limit
                true,
                token.asset_type.clone(),
                identifiers.clone(),
                Some(funding_round_name.clone()),
            ),
            AssetError::TotalSupplyAboveLimit
        );

        // Issuance is successful
        assert_ok!(Asset::create_asset(
            owner_signed.clone(),
            token.name.clone(),
            ticker,
            token.total_supply,
            true,
            token.asset_type.clone(),
            identifiers.clone(),
            Some(funding_round_name.clone()),
        ));

        // Check the update investor count for the newly created asset
        assert_eq!(Statistics::investor_count(ticker), 1);

        // A correct entry is added
        assert_eq!(Asset::token_details(ticker), token);
        assert_eq!(
            Asset::asset_ownership_relation(token.owner_did, ticker),
            AssetOwnershipRelation::AssetOwned
        );
        assert!(<DidRecords>::contains_key(
            Identity::get_token_did(&ticker).unwrap()
        ));
        assert_eq!(Asset::funding_round(ticker), funding_round_name.clone());

        // Unauthorized identities cannot rename the token.
        let eve_signed = Origin::signed(AccountKeyring::Eve.public());
        let _eve_did = register_keyring_account(AccountKeyring::Eve).unwrap();
        assert_err!(
            Asset::rename_asset(eve_signed, ticker, vec![0xde, 0xad, 0xbe, 0xef].into()),
            AssetError::Unauthorized
        );
        // The token should remain unchanged in storage.
        assert_eq!(Asset::token_details(ticker), token);
        // Rename the token and check storage has been updated.
        let renamed_token = SecurityToken {
            name: vec![0x42].into(),
            owner_did: token.owner_did,
            total_supply: token.total_supply,
            divisible: token.divisible,
            asset_type: token.asset_type.clone(),
            primary_issuance_agent: None,
            ..Default::default()
        };
        assert_ok!(Asset::rename_asset(
            owner_signed.clone(),
            ticker,
            renamed_token.name.clone()
        ));
        assert_eq!(Asset::token_details(ticker), renamed_token);
        assert!(Asset::identifiers(ticker).is_empty());
    });
}

#[test]
fn valid_transfers_pass() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let now = Utc::now();
            Timestamp::set_timestamp(now.timestamp() as u64);

            let owner_signed = Origin::signed(AccountKeyring::Dave.public());
            let owner_did = register_keyring_account_without_cdd(AccountKeyring::Dave).unwrap();

            // Expected token entry
            let token = SecurityToken {
                name: vec![b'A'].into(),
                owner_did,
                total_supply: 1_000_000,
                divisible: true,
                asset_type: AssetType::default(),
                ..Default::default()
            };
            let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
            let alice_did = register_keyring_account_without_cdd(AccountKeyring::Alice).unwrap();
            let eve = AccountKeyring::Eve.public();

            // Provide scope claim to sender and receiver of the transaction.
            provide_scope_claim_to_multiple_parties(&[alice_did, owner_did], ticker, eve);

            // Issuance is successful
            assert_ok!(Asset::create_asset(
                owner_signed.clone(),
                token.name.clone(),
                ticker,
                token.total_supply,
                true,
                token.asset_type.clone(),
                vec![],
                None,
            ));

            assert_eq!(
                Asset::balance_of(&ticker, token.owner_did),
                token.total_supply
            );

            // Allow all transfers
            assert_ok!(ComplianceManager::add_compliance_requirement(
                owner_signed.clone(),
                ticker,
                vec![],
                vec![]
            ));

            // Should fail as sender matches receiver
            assert_noop!(
                Asset::base_transfer(
                    PortfolioId::default_portfolio(owner_did),
                    PortfolioId::default_portfolio(owner_did),
                    &ticker,
                    500
                ),
                AssetError::InvalidTransfer
            );

            assert_ok!(Asset::base_transfer(
                PortfolioId::default_portfolio(owner_did),
                PortfolioId::default_portfolio(alice_did),
                &ticker,
                500
            ));

            let balance_alice = <asset::BalanceOf<TestStorage>>::get(&ticker, &alice_did);
            let balance_owner = <asset::BalanceOf<TestStorage>>::get(&ticker, &owner_did);
            assert_eq!(balance_owner, 1_000_000 - 500);
            assert_eq!(balance_alice, 500);
        })
}

#[test]
fn issuers_can_redeem_tokens() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Alice.public()])
        .build()
        .execute_with(|| {
            let now = Utc::now();
            Timestamp::set_timestamp(now.timestamp() as u64);

            let owner_signed = Origin::signed(AccountKeyring::Dave.public());
            let owner_did = register_keyring_account_without_cdd(AccountKeyring::Dave).unwrap();
            let bob_signed = Origin::signed(AccountKeyring::Bob.public());
            let _bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();

            // Expected token entry
            let token = SecurityToken {
                name: vec![b'A'].into(),
                owner_did,
                total_supply: 1_000_000,
                divisible: true,
                asset_type: AssetType::default(),
                primary_issuance_agent: None,
            };
            let ticker = Ticker::try_from(token.name.as_slice()).unwrap();

            // Provide scope claim to sender and receiver of the transaction.
            provide_scope_claim_to_multiple_parties(
                &[owner_did],
                ticker,
                AccountKeyring::Alice.public(),
            );

            // Issuance is successful
            assert_ok!(Asset::create_asset(
                owner_signed.clone(),
                token.name.clone(),
                ticker,
                token.total_supply,
                true,
                token.asset_type.clone(),
                vec![],
                None,
            ));

            assert_eq!(Asset::balance_of(&ticker, owner_did), token.total_supply);

            assert_noop!(
                Asset::redeem(bob_signed.clone(), ticker, token.total_supply),
                AssetError::Unauthorized
            );

            assert_noop!(
                Asset::redeem(owner_signed.clone(), ticker, token.total_supply + 1),
                PortfolioError::InsufficientPortfolioBalance
            );

            assert_ok!(Asset::redeem(
                owner_signed.clone(),
                ticker,
                token.total_supply
            ));

            assert_eq!(Asset::balance_of(&ticker, owner_did), 0);
            assert_eq!(Asset::token_details(&ticker).total_supply, 0);

            assert_noop!(
                Asset::redeem(owner_signed.clone(), ticker, 1),
                PortfolioError::InsufficientPortfolioBalance
            );
        })
}

fn default_transfer(from: IdentityId, to: IdentityId, ticker: Ticker, val: u128) {
    assert_ok!(Asset::unsafe_transfer(
        PortfolioId::default_portfolio(from),
        PortfolioId::default_portfolio(to),
        &ticker,
        val
    ));
}

#[test]
fn checkpoints_fuzz_test() {
    println!("Starting");
    for _ in 0..10 {
        // When fuzzing in local, feel free to bump this number to add more fuzz runs.
        ExtBuilder::default().build().execute_with(|| {
            let now = Utc::now();
            Timestamp::set_timestamp(now.timestamp() as u64);

            let owner_signed = Origin::signed(AccountKeyring::Dave.public());
            let owner_did = register_keyring_account(AccountKeyring::Dave).unwrap();

            // Expected token entry
            let token = SecurityToken {
                name: vec![b'A'].into(),
                owner_did,
                total_supply: 1_000_000,
                divisible: true,
                asset_type: AssetType::default(),
                ..Default::default()
            };
            let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
            let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();

            // Issuance is successful
            assert_ok!(Asset::create_asset(
                owner_signed.clone(),
                token.name.clone(),
                ticker,
                token.total_supply,
                true,
                token.asset_type.clone(),
                vec![],
                None,
            ));

            // Allow all transfers
            assert_ok!(ComplianceManager::add_compliance_requirement(
                owner_signed.clone(),
                ticker,
                vec![],
                vec![]
            ));
            let mut owner_balance: [u128; 100] = [1_000_000; 100];
            let mut bob_balance: [u128; 100] = [0; 100];
            let mut rng = rand::thread_rng();
            for j in 1..100 {
                let transfers = rng.gen_range(0, 10);
                owner_balance[j] = owner_balance[j - 1];
                bob_balance[j] = bob_balance[j - 1];
                for _k in 0..transfers {
                    if j == 1 {
                        owner_balance[0] -= 1;
                        bob_balance[0] += 1;
                    }
                    owner_balance[j] -= 1;
                    bob_balance[j] += 1;
                    default_transfer(owner_did, bob_did, ticker, 1);
                }
                assert_ok!(Checkpoint::create_checkpoint(owner_signed.clone(), ticker));
                let bal_at = |id, did| Asset::get_balance_at(ticker, did, CheckpointId(id));
                let check = |id, idx| {
                    assert_eq!(bal_at(id, owner_did), owner_balance[idx]);
                    assert_eq!(bal_at(id, bob_did), bob_balance[idx]);
                };
                let x: u64 = u64::try_from(j).unwrap();
                check(0, j);
                check(0, j);
                check(1, 1);
                check(x - 1, j - 1);
                check(x, j);
                check(x + 1, j);
                check(1000, j);
            }
        });
    }
}

#[test]
fn register_ticker() {
    ExtBuilder::default().build().execute_with(|| {
        let now = Utc::now();
        Timestamp::set_timestamp(now.timestamp() as u64);

        let owner_signed = Origin::signed(AccountKeyring::Dave.public());
        let owner_did = register_keyring_account(AccountKeyring::Dave).unwrap();

        let token = SecurityToken {
            name: vec![b'A'].into(),
            owner_did,
            total_supply: 1_000_000,
            divisible: true,
            asset_type: AssetType::default(),
            ..Default::default()
        };
        let identifiers = vec![AssetIdentifier::isin(*b"US0378331005").unwrap()];
        let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
        // Issuance is successful
        assert_ok!(Asset::create_asset(
            owner_signed.clone(),
            token.name.clone(),
            ticker,
            token.total_supply,
            true,
            token.asset_type.clone(),
            identifiers.clone(),
            None,
        ));

        assert_eq!(Asset::is_ticker_registry_valid(&ticker, owner_did), true);
        assert_eq!(Asset::is_ticker_available(&ticker), false);
        let stored_token = Asset::token_details(&ticker);
        assert_eq!(stored_token.asset_type, token.asset_type);
        assert_eq!(Asset::identifiers(ticker), identifiers);
        assert_err!(
            Asset::register_ticker(owner_signed.clone(), Ticker::try_from(&[b'A'][..]).unwrap()),
            AssetError::AssetAlreadyCreated
        );

        assert_err!(
            Asset::register_ticker(
                owner_signed.clone(),
                Ticker::try_from(&[b'A', b'A', b'A', b'A', b'A', b'A', b'A', b'A', b'A'][..])
                    .unwrap()
            ),
            AssetError::TickerTooLong
        );

        let ticker = Ticker::try_from(&[b'A', b'A'][..]).unwrap();

        assert_eq!(Asset::is_ticker_available(&ticker), true);

        assert_ok!(Asset::register_ticker(owner_signed.clone(), ticker));

        assert_eq!(
            Asset::asset_ownership_relation(owner_did, ticker),
            AssetOwnershipRelation::TickerOwned
        );

        let alice_signed = Origin::signed(AccountKeyring::Alice.public());
        let _ = register_keyring_account(AccountKeyring::Alice).unwrap();

        assert_err!(
            Asset::register_ticker(alice_signed.clone(), ticker),
            AssetError::TickerAlreadyRegistered
        );

        assert_eq!(Asset::is_ticker_registry_valid(&ticker, owner_did), true);
        assert_eq!(Asset::is_ticker_available(&ticker), false);

        Timestamp::set_timestamp(now.timestamp() as u64 + 10001);

        assert_eq!(Asset::is_ticker_registry_valid(&ticker, owner_did), false);
        assert_eq!(Asset::is_ticker_available(&ticker), true);

        for bs in &[
            [b'A', 31, b'B'].as_ref(),
            [127, b'A'].as_ref(),
            [b'A', 0, 0, 0, b'A'].as_ref(),
        ] {
            assert_noop!(
                Asset::register_ticker(owner_signed.clone(), Ticker::try_from(&bs[..]).unwrap()),
                AssetError::TickerNotAscii
            );
        }

        assert_ok!(Asset::register_ticker(
            owner_signed,
            Ticker::try_from(&[b' ', b'A', b'~'][..]).unwrap()
        ));
    })
}

#[test]
fn transfer_ticker() {
    ExtBuilder::default().build().execute_with(|| {
        let now = Utc::now();
        Timestamp::set_timestamp(now.timestamp() as u64);

        let owner_signed = Origin::signed(AccountKeyring::Dave.public());
        let owner_did = register_keyring_account(AccountKeyring::Dave).unwrap();
        let alice_signed = Origin::signed(AccountKeyring::Alice.public());
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let bob_signed = Origin::signed(AccountKeyring::Bob.public());
        let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();

        let ticker = Ticker::try_from(&[b'A', b'A'][..]).unwrap();

        assert_eq!(Asset::is_ticker_available(&ticker), true);
        assert_ok!(Asset::register_ticker(owner_signed.clone(), ticker));

        let auth_id_alice = Identity::add_auth(
            owner_did,
            Signatory::from(alice_did),
            AuthorizationData::TransferTicker(ticker),
            None,
        );

        let auth_id_bob = Identity::add_auth(
            owner_did,
            Signatory::from(bob_did),
            AuthorizationData::TransferTicker(ticker),
            None,
        );

        assert_eq!(Asset::is_ticker_registry_valid(&ticker, owner_did), true);
        assert_eq!(Asset::is_ticker_registry_valid(&ticker, alice_did), false);
        assert_eq!(Asset::is_ticker_available(&ticker), false);

        assert_err!(
            Asset::accept_ticker_transfer(alice_signed.clone(), auth_id_alice + 1),
            "Authorization does not exist"
        );

        assert_eq!(
            Asset::asset_ownership_relation(owner_did, ticker),
            AssetOwnershipRelation::TickerOwned
        );

        assert_ok!(Asset::accept_ticker_transfer(
            alice_signed.clone(),
            auth_id_alice
        ));

        assert_eq!(
            Asset::asset_ownership_relation(owner_did, ticker),
            AssetOwnershipRelation::NotOwned
        );
        assert_eq!(
            Asset::asset_ownership_relation(alice_did, ticker),
            AssetOwnershipRelation::TickerOwned
        );

        assert_eq!(
            Asset::asset_ownership_relation(alice_did, ticker),
            AssetOwnershipRelation::TickerOwned
        );

        assert_err!(
            Asset::accept_ticker_transfer(bob_signed.clone(), auth_id_bob),
            "Illegal use of Authorization"
        );

        let mut auth_id = Identity::add_auth(
            alice_did,
            Signatory::from(bob_did),
            AuthorizationData::TransferTicker(ticker),
            Some(now.timestamp() as u64 - 100),
        );

        assert_err!(
            Asset::accept_ticker_transfer(bob_signed.clone(), auth_id),
            "Authorization expired"
        );

        auth_id = Identity::add_auth(
            alice_did,
            Signatory::from(bob_did),
            AuthorizationData::Custom(ticker),
            Some(now.timestamp() as u64 + 100),
        );

        assert_err!(
            Asset::accept_ticker_transfer(bob_signed.clone(), auth_id),
            AssetError::NoTickerTransferAuth
        );

        auth_id = Identity::add_auth(
            alice_did,
            Signatory::from(bob_did),
            AuthorizationData::TransferTicker(ticker),
            Some(now.timestamp() as u64 + 100),
        );

        assert_ok!(Asset::accept_ticker_transfer(bob_signed.clone(), auth_id));

        assert_eq!(Asset::is_ticker_registry_valid(&ticker, owner_did), false);
        assert_eq!(Asset::is_ticker_registry_valid(&ticker, alice_did), false);
        assert_eq!(Asset::is_ticker_registry_valid(&ticker, bob_did), true);
        assert_eq!(Asset::is_ticker_available(&ticker), false);
    })
}

#[test]
fn controller_transfer() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let now = Utc::now();
            Timestamp::set_timestamp(now.timestamp() as u64);

            let owner_signed = Origin::signed(AccountKeyring::Dave.public());
            let owner_did = register_keyring_account_without_cdd(AccountKeyring::Dave).unwrap();

            // Expected token entry
            let token = SecurityToken {
                name: vec![b'A'].into(),
                owner_did,
                total_supply: 1_000_000,
                divisible: true,
                asset_type: AssetType::default(),
                ..Default::default()
            };
            let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
            let alice_did = register_keyring_account_without_cdd(AccountKeyring::Alice).unwrap();
            let eve = AccountKeyring::Eve.public();

            // Provide scope claim to sender and receiver of the transaction.
            provide_scope_claim_to_multiple_parties(&[alice_did, owner_did], ticker, eve);

            // Issuance is successful
            assert_ok!(Asset::create_asset(
                owner_signed.clone(),
                token.name.clone(),
                ticker,
                token.total_supply,
                true,
                token.asset_type.clone(),
                vec![],
                None,
            ));

            assert_eq!(
                Asset::balance_of(&ticker, token.owner_did),
                token.total_supply
            );

            // Allow all transfers
            assert_ok!(ComplianceManager::add_compliance_requirement(
                owner_signed.clone(),
                ticker,
                vec![],
                vec![]
            ));

            // Should fail as sender matches receiver
            assert_noop!(
                Asset::base_transfer(
                    PortfolioId::default_portfolio(owner_did),
                    PortfolioId::default_portfolio(owner_did),
                    &ticker,
                    500
                ),
                AssetError::InvalidTransfer
            );

            assert_ok!(Asset::base_transfer(
                PortfolioId::default_portfolio(owner_did),
                PortfolioId::default_portfolio(alice_did),
                &ticker,
                500
            ));

            let balance_alice = <asset::BalanceOf<TestStorage>>::get(&ticker, &alice_did);
            let balance_owner = <asset::BalanceOf<TestStorage>>::get(&ticker, &owner_did);
            assert_eq!(balance_owner, 1_000_000 - 500);
            assert_eq!(balance_alice, 500);

            assert_ok!(Asset::controller_transfer(
                owner_signed.clone(),
                ticker,
                100,
                PortfolioId::default_portfolio(alice_did),
            ));

            let new_balance_alice = <asset::BalanceOf<TestStorage>>::get(&ticker, &alice_did);
            let new_balance_owner = <asset::BalanceOf<TestStorage>>::get(&ticker, &owner_did);

            assert_eq!(new_balance_owner, balance_owner + 100);
            assert_eq!(new_balance_alice, balance_alice - 100);
        })
}

#[test]
fn transfer_primary_issuance_agent() {
    ExtBuilder::default().build().execute_with(|| {
        let now = Utc::now();
        Timestamp::set_timestamp(now.timestamp() as u64);

        let owner_signed = Origin::signed(AccountKeyring::Alice.public());
        let owner_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let primary_issuance_signed = Origin::signed(AccountKeyring::Bob.public());
        let primary_issuance_agent = register_keyring_account(AccountKeyring::Bob).unwrap();

        let ticker = Ticker::try_from(&[b'A', b'A'][..]).unwrap();
        let token = SecurityToken {
            name: ticker.as_slice().into(),
            total_supply: 1_000_000,
            owner_did,
            divisible: true,
            asset_type: Default::default(),
            primary_issuance_agent: None,
        };

        assert_ok!(Asset::create_asset(
            owner_signed,
            token.name.clone(),
            ticker.clone(),
            token.total_supply,
            token.divisible,
            token.asset_type.clone(),
            Default::default(),
            Default::default(),
        ));

        assert!(!Asset::is_ticker_available(&ticker));
        assert_eq!(Asset::token_details(&ticker), token);

        let auth_id = Identity::add_auth(
            owner_did,
            Signatory::from(primary_issuance_agent),
            AuthorizationData::TransferPrimaryIssuanceAgent(ticker),
            Some(now.timestamp() as u64 - 100),
        );

        assert_err!(
            Asset::accept_primary_issuance_agent_transfer(primary_issuance_signed.clone(), auth_id),
            "Authorization expired"
        );
        assert_eq!(Asset::token_details(&ticker), token);

        let auth_id = Identity::add_auth(
            owner_did,
            Signatory::from(owner_did),
            AuthorizationData::TransferPrimaryIssuanceAgent(ticker),
            None,
        );

        assert_err!(
            Asset::accept_primary_issuance_agent_transfer(primary_issuance_signed.clone(), auth_id),
            "Authorization does not exist"
        );
        assert_eq!(Asset::token_details(&ticker), token);

        let auth_id = Identity::add_auth(
            primary_issuance_agent,
            Signatory::from(primary_issuance_agent),
            AuthorizationData::TransferPrimaryIssuanceAgent(ticker),
            None,
        );

        assert_err!(
            Asset::accept_primary_issuance_agent_transfer(primary_issuance_signed.clone(), auth_id),
            "Illegal use of Authorization"
        );
        assert_eq!(Asset::token_details(&ticker), token);

        let auth_id = Identity::add_auth(
            owner_did,
            Signatory::from(primary_issuance_agent),
            AuthorizationData::TransferPrimaryIssuanceAgent(ticker),
            None,
        );

        assert_ok!(Asset::accept_primary_issuance_agent_transfer(
            primary_issuance_signed.clone(),
            auth_id
        ));
        let mut new_token = token.clone();
        new_token.primary_issuance_agent = Some(primary_issuance_agent);
        assert_eq!(Asset::token_details(&ticker), new_token);
    })
}

#[test]
fn transfer_token_ownership() {
    ExtBuilder::default().build().execute_with(|| {
        let now = Utc::now();
        Timestamp::set_timestamp(now.timestamp() as u64);

        let owner_signed = Origin::signed(AccountKeyring::Dave.public());
        let owner_did = register_keyring_account(AccountKeyring::Dave).unwrap();
        let alice_signed = Origin::signed(AccountKeyring::Alice.public());
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let bob_signed = Origin::signed(AccountKeyring::Bob.public());
        let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();

        let token_name = vec![b'A', b'A'];
        let ticker = Ticker::try_from(token_name.as_slice()).unwrap();
        assert_ok!(Asset::create_asset(
            owner_signed.clone(),
            token_name.into(),
            ticker,
            1_000_000,
            true,
            AssetType::default(),
            vec![],
            None,
        ));

        let auth_id_alice = Identity::add_auth(
            owner_did,
            Signatory::from(alice_did),
            AuthorizationData::TransferAssetOwnership(ticker),
            None,
        );

        let auth_id_bob = Identity::add_auth(
            owner_did,
            Signatory::from(bob_did),
            AuthorizationData::TransferAssetOwnership(ticker),
            None,
        );

        assert_eq!(Asset::token_details(&ticker).owner_did, owner_did);

        assert_err!(
            Asset::accept_asset_ownership_transfer(alice_signed.clone(), auth_id_alice + 1),
            "Authorization does not exist"
        );

        assert_eq!(
            Asset::asset_ownership_relation(owner_did, ticker),
            AssetOwnershipRelation::AssetOwned
        );

        assert_ok!(Asset::accept_asset_ownership_transfer(
            alice_signed.clone(),
            auth_id_alice
        ));
        assert_eq!(Asset::token_details(&ticker).owner_did, alice_did);
        assert_eq!(
            Asset::asset_ownership_relation(owner_did, ticker),
            AssetOwnershipRelation::NotOwned
        );
        assert_eq!(
            Asset::asset_ownership_relation(alice_did, ticker),
            AssetOwnershipRelation::AssetOwned
        );

        assert_err!(
            Asset::accept_asset_ownership_transfer(bob_signed.clone(), auth_id_bob),
            "Illegal use of Authorization"
        );

        let mut auth_id = Identity::add_auth(
            alice_did,
            Signatory::from(bob_did),
            AuthorizationData::TransferAssetOwnership(ticker),
            Some(now.timestamp() as u64 - 100),
        );

        assert_err!(
            Asset::accept_asset_ownership_transfer(bob_signed.clone(), auth_id),
            "Authorization expired"
        );

        auth_id = Identity::add_auth(
            alice_did,
            Signatory::from(bob_did),
            AuthorizationData::Custom(ticker),
            Some(now.timestamp() as u64 + 100),
        );

        assert_err!(
            Asset::accept_asset_ownership_transfer(bob_signed.clone(), auth_id),
            AssetError::NotTickerOwnershipTransferAuth
        );

        auth_id = Identity::add_auth(
            alice_did,
            Signatory::from(bob_did),
            AuthorizationData::TransferAssetOwnership(Ticker::try_from(&[0x50][..]).unwrap()),
            Some(now.timestamp() as u64 + 100),
        );

        assert_err!(
            Asset::accept_asset_ownership_transfer(bob_signed.clone(), auth_id),
            AssetError::NoSuchAsset
        );

        auth_id = Identity::add_auth(
            alice_did,
            Signatory::from(bob_did),
            AuthorizationData::TransferAssetOwnership(ticker),
            Some(now.timestamp() as u64 + 100),
        );

        assert_ok!(Asset::accept_asset_ownership_transfer(
            bob_signed.clone(),
            auth_id
        ));
        assert_eq!(Asset::token_details(&ticker).owner_did, bob_did);
    })
}

#[test]
fn update_identifiers() {
    ExtBuilder::default().build().execute_with(|| {
        let owner_signed = Origin::signed(AccountKeyring::Dave.public());
        let owner_did = register_keyring_account(AccountKeyring::Dave).unwrap();

        // Expected token entry
        let token = SecurityToken {
            name: b"TEST".into(),
            owner_did,
            total_supply: 1_000_000,
            divisible: true,
            asset_type: AssetType::default(),
            primary_issuance_agent: None,
            ..Default::default()
        };
        let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
        assert!(!<DidRecords>::contains_key(
            Identity::get_token_did(&ticker).unwrap()
        ));
        let identifier_value1 = b"037833100";
        let identifiers = vec![AssetIdentifier::cusip(*identifier_value1).unwrap()];
        assert_ok!(Asset::create_asset(
            owner_signed.clone(),
            token.name.clone(),
            ticker,
            token.total_supply,
            true,
            token.asset_type.clone(),
            identifiers.clone(),
            None,
        ));

        // A correct entry was added
        assert_eq!(Asset::token_details(ticker), token);
        assert_eq!(Asset::identifiers(ticker), identifiers);
        let identifier_value2 = b"US0378331005";
        let updated_identifiers = vec![
            AssetIdentifier::cusip(*b"17275R102").unwrap(),
            AssetIdentifier::isin(*identifier_value2).unwrap(),
        ];
        assert_ok!(Asset::update_identifiers(
            owner_signed.clone(),
            ticker,
            updated_identifiers.clone(),
        ));
        assert_eq!(Asset::identifiers(ticker), updated_identifiers);
    });
}

#[test]
fn adding_removing_documents() {
    ExtBuilder::default().build().execute_with(|| {
        let owner_signed = Origin::signed(AccountKeyring::Dave.public());
        let owner_did = register_keyring_account(AccountKeyring::Dave).unwrap();

        let token = SecurityToken {
            name: vec![b'A'].into(),
            owner_did,
            total_supply: 1_000_000,
            divisible: true,
            asset_type: AssetType::default(),
            ..Default::default()
        };

        let ticker = Ticker::try_from(token.name.as_slice()).unwrap();

        assert!(!<DidRecords>::contains_key(
            Identity::get_token_did(&ticker).unwrap()
        ));

        let identifiers = Vec::new();
        let _ticker_did = Identity::get_token_did(&ticker).unwrap();

        // Issuance is successful
        assert_ok!(Asset::create_asset(
            owner_signed.clone(),
            token.name.clone(),
            ticker,
            token.total_supply,
            true,
            token.asset_type.clone(),
            identifiers.clone(),
            None,
        ));

        let documents = vec![
            Document {
                name: b"A".into(),
                uri: b"www.a.com".into(),
                content_hash: [1u8; 64][..].try_into().unwrap(),
                doc_type: Some(b"foo".into()),
                filing_date: Some(42),
            },
            Document {
                name: b"B".into(),
                uri: b"www.b.com".into(),
                content_hash: [2u8; 64][..].try_into().unwrap(),
                doc_type: None,
                filing_date: None,
            },
        ];

        assert_ok!(Asset::add_documents(
            owner_signed.clone(),
            documents.clone(),
            ticker
        ));

        assert_eq!(Asset::asset_documents(ticker, DocumentId(0)), documents[0]);
        assert_eq!(Asset::asset_documents(ticker, DocumentId(1)), documents[1]);

        assert_ok!(Asset::remove_documents(
            owner_signed.clone(),
            (0..=1).map(DocumentId).collect(),
            ticker
        ));

        assert_eq!(
            <asset::AssetDocuments>::iter_prefix_values(ticker).count(),
            0
        );
    });
}

#[test]
fn add_extension_successfully() {
    ExtBuilder::default()
        .set_max_tms_allowed(2)
        .build()
        .execute_with(|| {
            let dave = AccountKeyring::Dave.public();
            // Create did and singed version of dave account.
            let (owner_signed, dave_did) = make_account_without_cdd(dave).unwrap();

            // Expected token entry
            let token = SecurityToken {
                name: b"TEST".into(),
                total_supply: 1_000_000,
                divisible: true,
                asset_type: AssetType::default(),
                ..Default::default()
            };
            let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
            assert!(!<DidRecords>::contains_key(
                Identity::get_token_did(&ticker).unwrap()
            ));
            let identifier_value1 = b"037833100";
            let identifiers = vec![AssetIdentifier::cusip(*identifier_value1).unwrap()];
            assert_ok!(Asset::create_asset(
                owner_signed.clone(),
                token.name.clone(),
                ticker,
                token.total_supply,
                true,
                token.asset_type.clone(),
                identifiers.clone(),
                None,
            ));

            // Add smart extension
            let extension_name = b"PTM".into();
            let extension_id = setup_se_template::<TestStorage>(dave, dave_did, true);

            let extension_details = SmartExtension {
                extension_type: SmartExtensionType::TransferManager,
                extension_name,
                extension_id: extension_id.clone(),
                is_archive: false,
            };

            assert_ok!(Asset::add_extension(
                owner_signed.clone(),
                ticker,
                extension_details.clone(),
            ));

            // verify the data within the runtime
            assert_eq!(
                Asset::extension_details((ticker, extension_id)),
                extension_details
            );
            assert_eq!(
                (Asset::extensions((ticker, SmartExtensionType::TransferManager))).len(),
                1
            );
            assert_eq!(
                (Asset::extensions((ticker, SmartExtensionType::TransferManager)))[0],
                extension_id
            );
        });
}

#[test]
fn add_same_extension_should_fail() {
    ExtBuilder::default()
        .set_max_tms_allowed(10)
        .build()
        .execute_with(|| {
            let dave = AccountKeyring::Dave.public();
            let (owner_signed, owner_did) = make_account_without_cdd(dave).unwrap();

            // Expected token entry
            let token = SecurityToken {
                name: b"TEST".into(),
                owner_did,
                total_supply: 1_000_000,
                divisible: true,
                asset_type: AssetType::default(),
                ..Default::default()
            };

            let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
            assert!(!<DidRecords>::contains_key(
                Identity::get_token_did(&ticker).unwrap()
            ));
            let identifier_value1 = b"037833100";
            let identifiers = vec![AssetIdentifier::cusip(*identifier_value1).unwrap()];
            assert_ok!(Asset::create_asset(
                owner_signed.clone(),
                token.name.clone(),
                ticker,
                token.total_supply,
                true,
                token.asset_type.clone(),
                identifiers.clone(),
                None,
            ));

            // Add smart extension
            let extension_name = b"PTM".into();
            let extension_id = setup_se_template::<TestStorage>(dave, owner_did, true);

            let extension_details = SmartExtension {
                extension_type: SmartExtensionType::TransferManager,
                extension_name,
                extension_id: extension_id.clone(),
                is_archive: false,
            };

            assert_ok!(Asset::add_extension(
                owner_signed.clone(),
                ticker,
                extension_details.clone()
            ));

            // verify the data within the runtime
            assert_eq!(
                Asset::extension_details((ticker, extension_id)),
                extension_details.clone()
            );
            assert_eq!(
                (Asset::extensions((ticker, SmartExtensionType::TransferManager))).len(),
                1
            );
            assert_eq!(
                (Asset::extensions((ticker, SmartExtensionType::TransferManager)))[0],
                extension_id
            );

            assert_err!(
                Asset::add_extension(owner_signed.clone(), ticker, extension_details),
                AssetError::ExtensionAlreadyPresent
            );
        });
}

#[test]
fn should_successfully_archive_extension() {
    ExtBuilder::default()
        .set_max_tms_allowed(10)
        .build()
        .execute_with(|| {
            let dave = AccountKeyring::Dave.public();
            let (owner_signed, owner_did) = make_account_without_cdd(dave).unwrap();

            // Expected token entry
            let token = SecurityToken {
                name: b"TEST".into(),
                owner_did,
                total_supply: 1_000_000,
                divisible: true,
                asset_type: AssetType::default(),
                ..Default::default()
            };

            let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
            assert!(!<DidRecords>::contains_key(
                Identity::get_token_did(&ticker).unwrap()
            ));
            let identifier_value1 = b"037833100";
            let identifiers = vec![AssetIdentifier::cusip(*identifier_value1).unwrap()];
            assert_ok!(Asset::create_asset(
                owner_signed.clone(),
                token.name.clone(),
                ticker,
                token.total_supply,
                true,
                token.asset_type.clone(),
                identifiers.clone(),
                None,
            ));
            // Add smart extension
            let extension_name = b"STO".into();
            let extension_id = setup_se_template::<TestStorage>(dave, owner_did, true);

            let extension_details = SmartExtension {
                extension_type: SmartExtensionType::Offerings,
                extension_name,
                extension_id: extension_id.clone(),
                is_archive: false,
            };

            assert_ok!(Asset::add_extension(
                owner_signed.clone(),
                ticker,
                extension_details.clone()
            ));

            // verify the data within the runtime
            assert_eq!(
                Asset::extension_details((ticker, extension_id)),
                extension_details
            );
            assert_eq!(
                (Asset::extensions((ticker, SmartExtensionType::Offerings))).len(),
                1
            );
            assert_eq!(
                (Asset::extensions((ticker, SmartExtensionType::Offerings)))[0],
                extension_id
            );

            assert_ok!(Asset::archive_extension(
                owner_signed.clone(),
                ticker,
                extension_id
            ));

            assert_eq!(
                (Asset::extension_details((ticker, extension_id))).is_archive,
                true
            );
        });
}

#[test]
fn should_fail_to_archive_an_already_archived_extension() {
    ExtBuilder::default()
        .set_max_tms_allowed(10)
        .build()
        .execute_with(|| {
            let dave = AccountKeyring::Dave.public();
            let (owner_signed, owner_did) = make_account_without_cdd(dave).unwrap();

            // Expected token entry
            let token = SecurityToken {
                name: b"TEST".into(),
                owner_did,
                total_supply: 1_000_000,
                divisible: true,
                asset_type: AssetType::default(),
                ..Default::default()
            };

            let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
            assert!(!<DidRecords>::contains_key(
                Identity::get_token_did(&ticker).unwrap()
            ));
            let identifier_value1 = b"037833100";
            let identifiers = vec![AssetIdentifier::cusip(*identifier_value1).unwrap()];
            assert_ok!(Asset::create_asset(
                owner_signed.clone(),
                token.name.clone(),
                ticker,
                token.total_supply,
                true,
                token.asset_type.clone(),
                identifiers.clone(),
                None,
            ));
            // Add smart extension
            let extension_name = b"STO".into();
            let extension_id = setup_se_template::<TestStorage>(dave, owner_did, true);

            let extension_details = SmartExtension {
                extension_type: SmartExtensionType::Offerings,
                extension_name,
                extension_id: extension_id.clone(),
                is_archive: false,
            };

            assert_ok!(Asset::add_extension(
                owner_signed.clone(),
                ticker,
                extension_details.clone()
            ));

            // verify the data within the runtime
            assert_eq!(
                Asset::extension_details((ticker, extension_id)),
                extension_details
            );
            assert_eq!(
                (Asset::extensions((ticker, SmartExtensionType::Offerings))).len(),
                1
            );
            assert_eq!(
                (Asset::extensions((ticker, SmartExtensionType::Offerings)))[0],
                extension_id
            );

            assert_ok!(Asset::archive_extension(
                owner_signed.clone(),
                ticker,
                extension_id
            ));

            assert_eq!(
                (Asset::extension_details((ticker, extension_id))).is_archive,
                true
            );

            assert_err!(
                Asset::archive_extension(owner_signed.clone(), ticker, extension_id),
                AssetError::AlreadyArchived
            );
        });
}

#[test]
fn should_fail_to_archive_a_non_existent_extension() {
    ExtBuilder::default()
        .set_max_tms_allowed(10)
        .build()
        .execute_with(|| {
            let owner_signed = Origin::signed(AccountKeyring::Dave.public());
            let owner_did = register_keyring_account(AccountKeyring::Dave).unwrap();

            // Expected token entry
            let token = SecurityToken {
                name: b"TEST".into(),
                owner_did,
                total_supply: 1_000_000,
                divisible: true,
                asset_type: AssetType::default(),
                ..Default::default()
            };

            let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
            assert!(!<DidRecords>::contains_key(
                Identity::get_token_did(&ticker).unwrap()
            ));
            let identifier_value1 = b"037833100";
            let identifiers = vec![AssetIdentifier::cusip(*identifier_value1).unwrap()];
            assert_ok!(Asset::create_asset(
                owner_signed.clone(),
                token.name.clone(),
                ticker,
                token.total_supply,
                true,
                token.asset_type.clone(),
                identifiers.clone(),
                None,
            ));
            // Add smart extension
            let extension_id = AccountKeyring::Bob.public();

            assert_err!(
                Asset::archive_extension(owner_signed.clone(), ticker, extension_id),
                AssetError::NoSuchSmartExtension
            );
        });
}

#[test]
fn should_successfuly_unarchive_an_extension() {
    ExtBuilder::default()
        .set_max_tms_allowed(10)
        .build()
        .execute_with(|| {
            let dave = AccountKeyring::Dave.public();
            let (owner_signed, owner_did) = make_account_without_cdd(dave).unwrap();

            // Expected token entry
            let token = SecurityToken {
                name: b"TEST".into(),
                owner_did,
                total_supply: 1_000_000,
                divisible: true,
                asset_type: AssetType::default(),
                ..Default::default()
            };

            let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
            assert!(!<DidRecords>::contains_key(
                Identity::get_token_did(&ticker).unwrap()
            ));
            let identifier_value1 = b"037833100";
            let identifiers = vec![AssetIdentifier::cusip(*identifier_value1).unwrap()];
            assert_ok!(Asset::create_asset(
                owner_signed.clone(),
                token.name.clone(),
                ticker,
                token.total_supply,
                true,
                token.asset_type.clone(),
                identifiers.clone(),
                None,
            ));
            // Add smart extension
            let extension_name = b"STO".into();
            let extension_id = setup_se_template::<TestStorage>(dave, owner_did, true);

            let extension_details = SmartExtension {
                extension_type: SmartExtensionType::Offerings,
                extension_name,
                extension_id: extension_id.clone(),
                is_archive: false,
            };

            assert_ok!(Asset::add_extension(
                owner_signed.clone(),
                ticker,
                extension_details.clone()
            ));

            // verify the data within the runtime
            assert_eq!(
                Asset::extension_details((ticker, extension_id)),
                extension_details
            );
            assert_eq!(
                (Asset::extensions((ticker, SmartExtensionType::Offerings))).len(),
                1
            );
            assert_eq!(
                (Asset::extensions((ticker, SmartExtensionType::Offerings)))[0],
                extension_id
            );

            assert_ok!(Asset::archive_extension(
                owner_signed.clone(),
                ticker,
                extension_id
            ));

            assert_eq!(
                (Asset::extension_details((ticker, extension_id))).is_archive,
                true
            );

            assert_ok!(Asset::unarchive_extension(
                owner_signed.clone(),
                ticker,
                extension_id
            ));
            assert_eq!(
                (Asset::extension_details((ticker, extension_id))).is_archive,
                false
            );
        });
}

#[test]
fn should_fail_to_unarchive_an_already_unarchived_extension() {
    ExtBuilder::default()
        .set_max_tms_allowed(10)
        .build()
        .execute_with(|| {
            let dave = AccountKeyring::Dave.public();
            let (owner_signed, owner_did) = make_account_without_cdd(dave).unwrap();

            // Expected token entry
            let token = SecurityToken {
                name: b"TEST".into(),
                owner_did,
                total_supply: 1_000_000,
                divisible: true,
                asset_type: AssetType::default(),
                ..Default::default()
            };

            let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
            assert!(!<DidRecords>::contains_key(
                Identity::get_token_did(&ticker).unwrap()
            ));
            let identifier_value1 = b"037833100";
            let identifiers = vec![AssetIdentifier::cusip(*identifier_value1).unwrap()];
            assert_ok!(Asset::create_asset(
                owner_signed.clone(),
                token.name.clone(),
                ticker,
                token.total_supply,
                true,
                token.asset_type.clone(),
                identifiers.clone(),
                None,
            ));
            // Add smart extension
            let extension_name = b"STO".into();
            let extension_id = setup_se_template::<TestStorage>(dave, owner_did, true);

            let extension_details = SmartExtension {
                extension_type: SmartExtensionType::Offerings,
                extension_name,
                extension_id: extension_id.clone(),
                is_archive: false,
            };

            assert_ok!(Asset::add_extension(
                owner_signed.clone(),
                ticker,
                extension_details.clone(),
            ));

            // verify the data within the runtime
            assert_eq!(
                Asset::extension_details((ticker, extension_id)),
                extension_details
            );
            assert_eq!(
                (Asset::extensions((ticker, SmartExtensionType::Offerings))).len(),
                1
            );
            assert_eq!(
                (Asset::extensions((ticker, SmartExtensionType::Offerings)))[0],
                extension_id
            );

            assert_ok!(Asset::archive_extension(
                owner_signed.clone(),
                ticker,
                extension_id
            ));

            assert_eq!(
                (Asset::extension_details((ticker, extension_id))).is_archive,
                true
            );

            assert_ok!(Asset::unarchive_extension(
                owner_signed.clone(),
                ticker,
                extension_id
            ));
            assert_eq!(
                (Asset::extension_details((ticker, extension_id))).is_archive,
                false
            );

            assert_err!(
                Asset::unarchive_extension(owner_signed.clone(), ticker, extension_id),
                AssetError::AlreadyUnArchived
            );
        });
}

#[test]
fn freeze_unfreeze_asset() {
    ExtBuilder::default().build().execute_with(|| {
        let now = Utc::now();
        Timestamp::set_timestamp(now.timestamp() as u64);
        let alice_signed = Origin::signed(AccountKeyring::Alice.public());
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let bob_signed = Origin::signed(AccountKeyring::Bob.public());
        let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
        let token_name = b"COOL";
        let ticker = Ticker::try_from(&token_name[..]).unwrap();
        assert_ok!(Asset::create_asset(
            alice_signed.clone(),
            token_name.into(),
            ticker,
            1_000_000,
            true,
            AssetType::default(),
            vec![],
            None,
        ));

        // Allow all transfers.
        assert_ok!(ComplianceManager::add_compliance_requirement(
            alice_signed.clone(),
            ticker,
            vec![],
            vec![]
        ));
        assert_err!(
            Asset::freeze(bob_signed.clone(), ticker),
            AssetError::Unauthorized
        );
        assert_err!(
            Asset::unfreeze(alice_signed.clone(), ticker),
            AssetError::NotFrozen
        );
        assert_ok!(Asset::freeze(alice_signed.clone(), ticker));
        assert_err!(
            Asset::freeze(alice_signed.clone(), ticker),
            AssetError::AlreadyFrozen
        );

        // Attempt to transfer token ownership.
        let auth_id = Identity::add_auth(
            alice_did,
            Signatory::from(bob_did),
            AuthorizationData::TransferAssetOwnership(ticker),
            None,
        );

        assert_ok!(Asset::accept_asset_ownership_transfer(
            bob_signed.clone(),
            auth_id
        ));

        assert_ok!(Asset::unfreeze(bob_signed.clone(), ticker));
        assert_err!(
            Asset::unfreeze(bob_signed.clone(), ticker),
            AssetError::NotFrozen
        );
    });
}
#[test]
fn frozen_secondary_keys_create_asset() {
    ExtBuilder::default()
        .build()
        .execute_with(frozen_secondary_keys_create_asset_we);
}

fn frozen_secondary_keys_create_asset_we() {
    // 0. Create identities.
    let alice = AccountKeyring::Alice.public();
    let alice_id = register_keyring_account(AccountKeyring::Alice).unwrap();
    let _charlie_id = register_keyring_account(AccountKeyring::Charlie).unwrap();
    let bob = AccountKeyring::Bob.public();

    // 1. Add Bob as signatory to Alice ID.
    let bob_signatory = Signatory::Account(AccountKeyring::Bob.public());
    add_secondary_key(alice_id, bob_signatory);
    assert_ok!(Balances::transfer_with_memo(
        Origin::signed(alice),
        bob,
        1_000,
        Some(Memo::from("Bob funding"))
    ));

    // 2. Bob can create token
    let token_1 = SecurityToken {
        name: vec![b'A'].into(),
        owner_did: alice_id,
        total_supply: 1_000_000,
        divisible: true,
        asset_type: AssetType::default(),
        primary_issuance_agent: None,
        ..Default::default()
    };
    let ticker_1 = Ticker::try_from(token_1.name.as_slice()).unwrap();
    assert_ok!(Asset::create_asset(
        Origin::signed(bob),
        token_1.name.clone(),
        ticker_1,
        token_1.total_supply,
        true,
        token_1.asset_type.clone(),
        vec![],
        None,
    ));
    assert_eq!(Asset::token_details(ticker_1), token_1);

    // 3. Alice freezes her secondary keys.
    assert_ok!(Identity::freeze_secondary_keys(Origin::signed(alice)));

    // 4. Bob cannot create a token.
    let token_2 = SecurityToken {
        name: vec![b'A'].into(),
        owner_did: alice_id,
        total_supply: 1_000_000,
        divisible: true,
        asset_type: AssetType::default(),
        ..Default::default()
    };
    let _ticker_2 = Ticker::try_from(token_2.name.as_slice()).unwrap();
    // commenting this because `default_identity` feature is not allowing to access None identity.
    // let create_token_result = Asset::create_asset(
    //     Origin::signed(bob),
    //     token_2.name.clone(),
    //     ticker_2,
    //     token_2.total_supply,
    //     true,
    //     token_2.asset_type.clone(),
    //     vec![],
    //     None,
    // );
    // assert_err!(
    //     create_token_result,
    //     DispatchError::Other("Current identity is none and key is not linked to any identity")
    // );
}

#[test]
fn test_can_transfer_rpc() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .monied(true)
        .balance_factor(1)
        .build()
        .execute_with(|| {
            let alice_signed = Origin::signed(AccountKeyring::Alice.public());
            let alice_did = register_keyring_account_without_cdd(AccountKeyring::Alice).unwrap();
            let _bob_signed = Origin::signed(AccountKeyring::Bob.public());
            let bob_did = register_keyring_account_without_cdd(AccountKeyring::Bob).unwrap();

            let eve = AccountKeyring::Eve.public();

            let token_name = b"COOL";
            let ticker = Ticker::try_from(&token_name[..]).unwrap();
            assert_ok!(Asset::create_asset(
                alice_signed.clone(),
                token_name.into(),
                ticker,
                1_000 * currency::ONE_UNIT,
                false, // Asset divisibility is false
                AssetType::default(),
                vec![],
                None,
            ));

            // check the balance of the alice Identity
            assert_eq!(
                Asset::balance_of(&ticker, alice_did),
                1_000 * currency::ONE_UNIT
            );

            // Provide scope claim for sender and receiver.
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker, eve);

            let unsafe_can_transfer_result = |from_did, to_did, amount| {
                Asset::unsafe_can_transfer(
                    None,
                    PortfolioId::default_portfolio(from_did),
                    None,
                    PortfolioId::default_portfolio(to_did),
                    &ticker,
                    amount, // It only passed when it should be the multiple of currency::ONE_UNIT
                )
                .unwrap()
            };

            // case 1: When passed invalid granularity
            assert_eq!(
                unsafe_can_transfer_result(alice_did, bob_did, 100),
                INVALID_GRANULARITY
            );

            // Case 2: when from_did balance is 0
            assert_eq!(
                unsafe_can_transfer_result(bob_did, alice_did, 100 * currency::ONE_UNIT),
                ERC1400_INSUFFICIENT_BALANCE
            );

            // Comment below test case, These will be un-commented when we improved the DID check.

            // // Case 4: When sender doesn't posses a valid cdd
            // // 4.1: Create Identity who doesn't posses cdd
            // let (from_without_cdd_signed, from_without_cdd_did) = make_account_with_uid(AccountKeyring::Ferdie.public()).unwrap();
            // // Execute can_transfer
            // assert_eq!(
            //     Asset::unsafe_can_transfer(
            //         AccountKeyring::Ferdie.public(),
            //         ticker,
            //         Some(from_without_cdd_did),
            //         Some(bob_did),
            //         5 * currency::ONE_UNIT
            //     ).unwrap(),
            //     INVALID_SENDER_DID
            // );

            // // Case 5: When receiver doesn't posses a valid cdd
            // assert_eq!(
            //     Asset::unsafe_can_transfer(
            //         AccountKeyring::Alice.public(),
            //         ticker,
            //         Some(alice_did),
            //         Some(from_without_cdd_did),
            //         5 * currency::ONE_UNIT
            //     ).unwrap(),
            //     INVALID_RECEIVER_DID
            // );

            // Case 6: When Asset transfer is frozen
            // 6.1: pause the transfer
            assert_ok!(Asset::freeze(alice_signed.clone(), ticker));
            assert_eq!(
                unsafe_can_transfer_result(alice_did, bob_did, 20 * currency::ONE_UNIT),
                ERC1400_TRANSFERS_HALTED
            );
            assert_ok!(Asset::unfreeze(alice_signed.clone(), ticker));

            // Case 7: when transaction get success by the compliance_manager
            // Allow all transfers.
            assert_ok!(ComplianceManager::add_compliance_requirement(
                alice_signed.clone(),
                ticker,
                vec![],
                vec![]
            ));

            assert_eq!(
                unsafe_can_transfer_result(alice_did, bob_did, 20 * currency::ONE_UNIT),
                ERC1400_TRANSFER_SUCCESS
            );
        })
}

#[test]
fn can_set_primary_issuance_agent() {
    ExtBuilder::default()
        .build()
        .execute_with(can_set_primary_issuance_agent_we);
}

fn can_set_primary_issuance_agent_we() {
    let alice = AccountKeyring::Alice.public();
    let alice_id = register_keyring_account(AccountKeyring::Alice).unwrap();
    let bob = AccountKeyring::Bob.public();
    let bob_id = register_keyring_account(AccountKeyring::Bob).unwrap();
    assert_ok!(Balances::transfer_with_memo(
        Origin::signed(alice),
        bob,
        1_000,
        Some(Memo::from("Bob funding"))
    ));
    let mut token = SecurityToken {
        name: vec![b'A'].into(),
        owner_did: alice_id,
        total_supply: 1_000_000,
        divisible: true,
        asset_type: AssetType::default(),
        primary_issuance_agent: Some(bob_id),
        ..Default::default()
    };
    let ticker = Ticker::try_from(token.name.as_slice()).unwrap();

    assert_ok!(Asset::create_asset(
        Origin::signed(alice),
        token.name.clone(),
        ticker,
        token.total_supply,
        true,
        token.asset_type.clone(),
        vec![],
        None,
    ));
    let auth_id = Identity::add_auth(
        token.owner_did,
        Signatory::from(bob_id),
        AuthorizationData::TransferPrimaryIssuanceAgent(ticker),
        None,
    );
    assert_ok!(Asset::accept_primary_issuance_agent_transfer(
        Origin::signed(bob),
        auth_id
    ));
    assert_eq!(Asset::token_details(ticker), token);
    assert_ok!(Asset::remove_primary_issuance_agent(
        Origin::signed(alice),
        ticker
    ));
    token.primary_issuance_agent = None;
    assert_eq!(Asset::token_details(ticker), token);
}

#[test]
fn check_functionality_of_remove_extension() {
    ExtBuilder::default()
        .set_max_tms_allowed(5)
        .build()
        .execute_with(|| {
            let alice = AccountKeyring::Alice.public();
            let (alice_signed, alice_did) = make_account_without_cdd(alice).unwrap();

            let token = SecurityToken {
                name: vec![b'A'].into(),
                owner_did: alice_did,
                total_supply: 1_000_000_000,
                divisible: true,
                asset_type: AssetType::default(),
                primary_issuance_agent: None,
                ..Default::default()
            };
            let ticker = Ticker::try_from(token.name.as_slice()).unwrap();

            assert_ok!(Asset::create_asset(
                alice_signed.clone(),
                token.name.clone(),
                ticker,
                token.total_supply,
                true,
                token.asset_type.clone(),
                vec![],
                None,
            ));

            let extension_id = setup_se_template::<TestStorage>(alice, alice_did, true);

            // Add Tms
            assert_ok!(Asset::add_extension(
                alice_signed.clone(),
                ticker,
                SmartExtension {
                    extension_type: SmartExtensionType::TransferManager,
                    extension_name: b"ABC".into(),
                    extension_id: extension_id,
                    is_archive: false
                }
            ));

            // verify storage
            assert_eq!(
                Asset::extensions((ticker, SmartExtensionType::TransferManager)),
                vec![extension_id]
            );
            // Remove the extension
            assert_ok!(Asset::remove_smart_extension(
                alice_signed.clone(),
                ticker,
                extension_id
            ));

            // verify storage
            assert_eq!(
                Asset::extensions((ticker, SmartExtensionType::TransferManager)),
                vec![]
            );

            // Removing the same extension gives the error.
            assert_err!(
                Asset::remove_smart_extension(alice_signed.clone(), ticker, extension_id),
                AssetError::NoSuchSmartExtension
            );
        });
}

// Classic token tests:

fn ticker(name: &str) -> Ticker {
    name.as_bytes().try_into().unwrap()
}

fn default_classic() -> ClassicTickerImport {
    ClassicTickerImport {
        eth_owner: <_>::default(),
        ticker: <_>::default(),
        is_created: false,
        is_contract: false,
    }
}

fn default_reg_config() -> TickerRegistrationConfig<u64> {
    TickerRegistrationConfig {
        max_ticker_length: 8,
        registration_length: Some(10000),
    }
}

fn alice_secret_key() -> secp256k1::SecretKey {
    secp256k1::SecretKey::parse(&keccak_256(b"Alice")).unwrap()
}

fn bob_secret_key() -> secp256k1::SecretKey {
    secp256k1::SecretKey::parse(&keccak_256(b"Bob")).unwrap()
}

fn sorted<K: Ord + Clone, V>(iter: impl IntoIterator<Item = (K, V)>) -> Vec<(K, V)> {
    let mut vec: Vec<_> = iter.into_iter().collect();
    vec.sort_by_key(|x| x.0.clone());
    vec
}

fn with_asset_genesis(genesis: AssetGenesis) -> ExtBuilder {
    ExtBuilder::default().adjust(Box::new(move |storage| {
        genesis.assimilate_storage(storage).unwrap();
    }))
}

fn test_asset_genesis(genesis: AssetGenesis) {
    with_asset_genesis(genesis).build().execute_with(|| {});
}

#[test]
#[should_panic = "lowercase ticker"]
fn classic_ticker_genesis_lowercase() {
    test_asset_genesis(AssetGenesis {
        classic_migration_tickers: vec![ClassicTickerImport {
            ticker: ticker("lower"),
            ..default_classic()
        }],
        ..<_>::default()
    });
}

#[test]
#[should_panic = "TickerTooLong"]
fn classic_ticker_genesis_too_long() {
    test_asset_genesis(AssetGenesis {
        classic_migration_tconfig: TickerRegistrationConfig {
            max_ticker_length: 3,
            registration_length: None,
        },
        classic_migration_tickers: vec![ClassicTickerImport {
            ticker: ticker("ACME"),
            ..default_classic()
        }],
        ..<_>::default()
    });
}

#[test]
#[should_panic = "TickerAlreadyRegistered"]
fn classic_ticker_genesis_already_registered_sys_did() {
    let import = ClassicTickerImport {
        ticker: ticker("ACME"),
        ..default_classic()
    };
    test_asset_genesis(AssetGenesis {
        classic_migration_tconfig: default_reg_config(),
        classic_migration_tickers: vec![import.clone(), import],
        ..<_>::default()
    });
}

#[test]
#[should_panic = "TickerAlreadyRegistered"]
fn classic_ticker_genesis_already_registered_other_did() {
    let import_a = ClassicTickerImport {
        ticker: ticker("ACME"),
        ..default_classic()
    };
    let import_b = ClassicTickerImport {
        is_contract: true,
        ..import_a.clone()
    };
    test_asset_genesis(AssetGenesis {
        classic_migration_contract_did: 1.into(),
        classic_migration_tconfig: default_reg_config(),
        classic_migration_tickers: vec![import_a, import_b],
        ..<_>::default()
    });
}

#[test]
fn classic_ticker_genesis_works() {
    let alice_eth = ethereum::EthereumAddress(*b"0x012345678987654321");
    let bob_eth = ethereum::EthereumAddress(*b"0x212345678987654321");
    let charlie_eth = ethereum::EthereumAddress(*b"0x512345678987654321");

    // Define actual on-genesis asset config.
    let classic_migration_tickers = vec![
        ClassicTickerImport {
            eth_owner: alice_eth,
            ticker: ticker("ALPHA"),
            is_created: false,
            is_contract: false,
        },
        ClassicTickerImport {
            eth_owner: alice_eth,
            ticker: ticker("BETA"),
            is_created: true,
            is_contract: false,
        },
        ClassicTickerImport {
            eth_owner: bob_eth,
            ticker: ticker("GAMMA"),
            is_created: false,
            is_contract: true,
        },
        ClassicTickerImport {
            eth_owner: charlie_eth,
            ticker: ticker("DELTA"),
            is_created: true,
            is_contract: true,
        },
    ];
    let contract_did = IdentityId::from(1337);
    let registration_length = Some(42);
    let standard_config = default_reg_config();
    let genesis = AssetGenesis {
        classic_migration_tickers,
        ticker_registration_config: standard_config.clone(),
        classic_migration_contract_did: contract_did,
        classic_migration_tconfig: TickerRegistrationConfig {
            registration_length,
            ..standard_config
        },
        reserved_country_currency_codes: vec![],
        versions: vec![],
    };

    // Define expected ticker data after genesis.
    let reg = |owner, expiry| TickerRegistration { expiry, owner };
    let cm_did = SystematicIssuers::ClassicMigration.as_id();
    let mut tickers = vec![
        (ticker("ALPHA"), reg(cm_did, registration_length)),
        (ticker("BETA"), reg(cm_did, registration_length)),
        (ticker("GAMMA"), reg(contract_did, registration_length)),
        (ticker("DELTA"), reg(contract_did, registration_length)),
    ];

    // Define expected classic ticker data after genesis.
    let classic_reg = |eth_owner, is_created| ClassicTickerRegistration {
        eth_owner,
        is_created,
    };
    let classic_tickers = vec![
        (ticker("ALPHA"), classic_reg(alice_eth, false)),
        (ticker("BETA"), classic_reg(alice_eth, true)),
        (ticker("GAMMA"), classic_reg(bob_eth, false)),
        (ticker("DELTA"), classic_reg(charlie_eth, true)),
    ];

    with_asset_genesis(genesis).build().execute_with(move || {
        // Dave enters the room.
        let rt_signer = Origin::signed(AccountKeyring::Dave.public());
        let rt_did = register_keyring_account(AccountKeyring::Dave).unwrap();

        // Ensure we have cm_did != contract_did != rt_did.
        assert_ne!(cm_did, contract_did);
        assert_ne!(rt_did, cm_did);
        assert_ne!(rt_did, contract_did);

        // Add another ticker to contrast expiry and DID and expect it.
        let expiry = standard_config.registration_length;
        assert_ok!(Asset::register_ticker(rt_signer, ticker("EPSILON")));
        tickers.push((ticker("EPSILON"), reg(rt_did, expiry)));

        // Ensure actual permutes expected.
        assert_eq!(sorted(Tickers::<TestStorage>::iter()), sorted(tickers));
        assert_eq!(sorted(ClassicTickers::iter()), sorted(classic_tickers));
    });
}

#[test]
fn classic_ticker_register_works() {
    let mut config = default_reg_config();
    with_asset_genesis(AssetGenesis {
        ticker_registration_config: config.clone(),
        classic_migration_tconfig: config.clone(),
        // Let there be one classic ticker reserved already, to cause a conflict.
        classic_migration_tickers: vec![ClassicTickerImport {
            ticker: ticker("ACME"),
            ..default_classic()
        }],
        ..<_>::default()
    })
    .build()
    .execute_with(move || {
        let alice_eth = ethereum::EthereumAddress(*b"0x012345678987654321");
        let mut classic = ClassicTickerImport {
            eth_owner: alice_eth,
            ticker: ticker("ACME"),
            is_created: true,
            is_contract: false,
        };

        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let alice_signer = Origin::signed(AccountKeyring::Alice.public());

        // Only root can call.
        assert_noop!(
            Asset::reserve_classic_ticker(alice_signer.clone(), classic, alice_did, config.clone()),
            DispatchError::BadOrigin
        );

        // ACME was already registered.
        assert_noop!(
            Asset::reserve_classic_ticker(root(), classic, alice_did, config.clone()),
            AssetError::TickerAlreadyRegistered
        );

        // Create BETA as an asset and fail to register it.
        classic.ticker = ticker("BETA");
        assert_ok!(Asset::create_asset(
            alice_signer,
            b"".into(),
            classic.ticker,
            0,
            false,
            <_>::default(),
            vec![],
            None,
        ));
        assert_noop!(
            Asset::reserve_classic_ticker(root(), classic, alice_did, config.clone()),
            AssetError::AssetAlreadyCreated
        );

        // Set max-length to 1 and check that it's enforced.
        // Also set and expect the expiry length.
        config.max_ticker_length = 1;
        classic.ticker = ticker("AB");
        assert_noop!(
            Asset::reserve_classic_ticker(root(), classic, alice_did, config.clone()),
            AssetError::TickerTooLong
        );
        classic.ticker = ticker("A");
        config.registration_length = Some(1337);
        let expiry = Some(Timestamp::get() + 1337);
        let assert = |classic, owner, is_created| {
            assert_ok!(Asset::reserve_classic_ticker(
                root(),
                classic,
                alice_did,
                config.clone()
            ));
            assert_eq!(
                Asset::ticker_registration(classic.ticker),
                TickerRegistration { owner, expiry },
            );
            assert_eq!(
                Asset::classic_ticker_registration(classic.ticker).unwrap(),
                ClassicTickerRegistration {
                    is_created,
                    eth_owner: alice_eth,
                },
            );
        };
        assert(classic, SystematicIssuers::ClassicMigration.as_id(), true);

        // Also test the contract situation.
        classic.ticker = ticker("B");
        classic.is_contract = true;
        classic.is_created = false;
        assert(classic, alice_did, false);
    });
}

#[test]
fn classic_ticker_no_such_classic_ticker() {
    with_asset_genesis(AssetGenesis {
        classic_migration_tconfig: default_reg_config(),
        // There is a classic ticker, but not the one we're claiming.
        classic_migration_tickers: vec![ClassicTickerImport {
            ticker: ticker("ACME"),
            ..default_classic()
        }],
        ..<_>::default()
    })
    .build()
    .execute_with(|| {
        assert_noop!(
            Asset::claim_classic_ticker(root(), ticker("EMCA"), ethereum::EcdsaSignature([0; 65])),
            AssetError::NoSuchClassicTicker
        );
    });
}

#[test]
fn classic_ticker_registered_by_other() {
    let ticker = ticker("ACME");
    with_asset_genesis(AssetGenesis {
        classic_migration_tconfig: default_reg_config(),
        // There is a classic ticker, but its not owned by sys DID.
        classic_migration_tickers: vec![ClassicTickerImport {
            ticker,
            is_contract: true,
            ..default_classic()
        }],
        ..<_>::default()
    })
    .build()
    .execute_with(|| {
        assert_noop!(
            Asset::claim_classic_ticker(root(), ticker, ethereum::EcdsaSignature([0; 65])),
            AssetError::TickerAlreadyRegistered
        );
    });
}

#[test]
fn classic_ticker_expired_thus_available() {
    let ticker = ticker("ACME");
    with_asset_genesis(AssetGenesis {
        classic_migration_tconfig: TickerRegistrationConfig {
            registration_length: Some(0),
            ..default_reg_config()
        },
        classic_migration_tickers: vec![ClassicTickerImport {
            ticker,
            ..default_classic()
        }],
        ..<_>::default()
    })
    .build()
    .execute_with(|| {
        let rt_signer = Origin::signed(AccountKeyring::Dave.public());
        Timestamp::set_timestamp(1);
        assert_noop!(
            Asset::claim_classic_ticker(rt_signer, ticker, ethereum::EcdsaSignature([0; 65])),
            AssetError::TickerRegistrationExpired
        );
    });
}

#[test]
fn classic_ticker_garbage_signature() {
    let ticker = ticker("ACME");
    with_asset_genesis(AssetGenesis {
        classic_migration_tconfig: default_reg_config(),
        classic_migration_tickers: vec![ClassicTickerImport {
            ticker,
            ..default_classic()
        }],
        ..<_>::default()
    })
    .build()
    .execute_with(|| {
        let rt_signer = Origin::signed(AccountKeyring::Dave.public());
        assert_noop!(
            Asset::claim_classic_ticker(rt_signer, ticker, ethereum::EcdsaSignature([0; 65])),
            pallet_permissions::Error::<TestStorage>::UnauthorizedCaller
        );
    });
}

#[test]
fn classic_ticker_not_owner() {
    let ticker = ticker("ACME");
    with_asset_genesis(AssetGenesis {
        classic_migration_tconfig: default_reg_config(),
        classic_migration_tickers: vec![ClassicTickerImport {
            ticker,
            eth_owner: ethereum::address(&alice_secret_key()),
            ..default_classic()
        }],
        ..<_>::default()
    })
    .build()
    .execute_with(|| {
        let signer = Origin::signed(AccountKeyring::Bob.public());
        let did = register_keyring_account(AccountKeyring::Bob).unwrap();
        let eth_sig = ethereum::eth_msg(did, b"classic_claim", &bob_secret_key());
        assert_noop!(
            Asset::claim_classic_ticker(signer, ticker, eth_sig),
            AssetError::NotAnOwner
        );
    });
}

#[test]
fn classic_ticker_claim_works() {
    let eth_owner = ethereum::address(&alice_secret_key());

    // Define all the classic ticker imports.
    let import = |name, is_created| ClassicTickerImport {
        eth_owner,
        ticker: ticker(name),
        is_created,
        is_contract: false,
    };
    let tickers = vec![
        import("ALPHA", false),
        import("BETA", true),
        import("GAMMA", true),
        import("DELTA", true),
        import("EPSILON", true),
        import("ZETA", true),
    ];

    // Complete the genesis definition.
    let expire_after = 42;
    let genesis = AssetGenesis {
        classic_migration_tickers: tickers.clone(),
        ticker_registration_config: default_reg_config(),
        classic_migration_tconfig: TickerRegistrationConfig {
            registration_length: Some(expire_after),
            ..default_reg_config()
        },
        classic_migration_contract_did: 0.into(),
        reserved_country_currency_codes: vec![],
        versions: vec![],
    };

    // Define the fees and initial balance.
    let init_bal = 150;
    let fee = 50;
    let fees = MockProtocolBaseFees(vec![(ProtocolOp::AssetCreateAsset, fee)]);

    let ext = with_asset_genesis(genesis).set_protocol_base_fees(fees);
    ext.build().execute_with(move || {
        System::set_block_number(1);

        let focus_user = |key: AccountKeyring, bal| {
            let acc = key.public();
            let did = crate::register_keyring_account_with_balance(key, bal).unwrap();
            TestStorage::set_payer_context(Some(acc));
            (acc, did)
        };

        // Initialize Alice and let them claim classic tickers successfully.
        let (alice_acc, alice_did) = focus_user(AccountKeyring::Alice, init_bal);
        let eth_sig = ethereum::eth_msg(alice_did, b"classic_claim", &alice_secret_key());
        for ClassicTickerImport { ticker, .. } in tickers {
            let signer = Origin::signed(alice_acc);
            assert_ok!(Asset::claim_classic_ticker(signer, ticker, eth_sig.clone()));
            assert_eq!(alice_did, Tickers::<TestStorage>::get(ticker).owner);
            assert!(matches!(
                &*System::events(),
                [
                    ..,
                    frame_system::EventRecord {
                        event: super::storage::EventTest::asset(
                            pallet_asset::RawEvent::ClassicTickerClaimed(..)
                        ),
                        ..
                    }
                ]
            ));
        }

        // Create `ALPHA` asset; this will cost.
        let create = |acc, name: &str, bal_after| {
            let asset = name.try_into().unwrap();
            let ticker = ticker(name);
            let signer = Origin::signed(acc);
            let ret =
                Asset::create_asset(signer, asset, ticker, 1, true, <_>::default(), vec![], None);
            assert_balance(acc, bal_after, 0);
            ret
        };
        assert_ok!(create(alice_acc, "ALPHA", init_bal - fee));

        // Create `BETA`; fee is waived as `is_created: true`.
        assert_ok!(create(alice_acc, "BETA", init_bal - fee));

        // Fast forward, thereby expiring `GAMMA` for which `is_created: true` holds.
        // Thus, fee isn't waived and is charged.
        Timestamp::set_timestamp(expire_after + 1);
        assert_ok!(create(alice_acc, "GAMMA", init_bal - fee - fee));

        // Now `DELTA` has expired as well. Bob registers it, so its not classic anymore and fee is charged.
        let (bob_acc, _) = focus_user(AccountKeyring::Bob, 0);
        assert!(ClassicTickers::get(&ticker("DELTA")).is_some());
        assert_ok!(Asset::register_ticker(
            Origin::signed(bob_acc),
            ticker("DELTA")
        ));
        assert_eq!(ClassicTickers::get(&ticker("DELTA")), None);
        assert_noop!(
            create(bob_acc, "DELTA", 0),
            FeeError::InsufficientAccountBalance
        );

        // Repeat for `EPSILON`, but directly `create_asset` instead.
        let (charlie_acc, charlie_did) = focus_user(AccountKeyring::Charlie, 2 * fee);
        assert!(ClassicTickers::get(&ticker("EPSILON")).is_some());
        assert_ok!(create(charlie_acc, "EPSILON", 1 * fee));
        assert_eq!(ClassicTickers::get(&ticker("EPSILON")), None);

        // Travel back in time to unexpire `ZETA`,
        // transfer it to Charlie, and ensure its not classic anymore.
        Timestamp::set_timestamp(0);
        let zeta = ticker("ZETA");
        assert!(ClassicTickers::get(&zeta).is_some());
        let auth_id_alice = Identity::add_auth(
            alice_did,
            Signatory::from(charlie_did),
            AuthorizationData::TransferTicker(zeta),
            None,
        );
        assert_ok!(Asset::accept_ticker_transfer(
            Origin::signed(charlie_acc),
            auth_id_alice
        ));
        assert_eq!(ClassicTickers::get(&zeta), None);
        assert_ok!(create(charlie_acc, "ZETA", 0 * fee));
        assert_eq!(ClassicTickers::get(&zeta), None);
    });
}

fn generate_uid(entity_name: String) -> InvestorUid {
    InvestorUid::from(format!("uid_{}", entity_name).as_bytes())
}

// Test for the validating the code for unique investors and aggregation of balances.
#[test]
fn check_unique_investor_count() {
    ExtBuilder::default()
        .set_max_tms_allowed(5)
        .cdd_providers(vec![AccountKeyring::Charlie.public()])
        .build()
        .execute_with(|| {
            // cdd provider.
            let cdd_provider = AccountKeyring::Charlie.public();

            let alice = AccountKeyring::Alice.public();
            let (alice_signed, alice_did) = make_account_without_cdd(alice).unwrap();

            // Bob entity as investor in given ticker.
            let bob_1 = AccountKeyring::Bob.public();
            let (_, bob_1_did) = make_account_without_cdd(bob_1).unwrap();

            // Eve also comes under the `Bob` entity.
            let bob_2 = AccountKeyring::Eve.public();
            let (_, bob_2_did) = make_account_without_cdd(bob_2).unwrap();

            let total_supply = 1_000_000_000;

            let token = SecurityToken {
                name: vec![b'A'].into(),
                owner_did: alice_did,
                total_supply: total_supply,
                divisible: true,
                asset_type: AssetType::default(),
                primary_issuance_agent: None,
                ..Default::default()
            };
            let ticker = Ticker::try_from(token.name.as_slice()).unwrap();

            assert_ok!(Asset::create_asset(
                alice_signed.clone(),
                token.name.clone(),
                ticker,
                token.total_supply,
                true,
                token.asset_type.clone(),
                vec![],
                None,
            ));

            // Verify the asset creation
            assert_eq!(Asset::token_details(&ticker), token);

            // Verify the balance of the alice and the investor count for the asset.
            assert_eq!(Asset::balance_of(&ticker, alice_did), total_supply); // It should be equal to total supply.
                                                                             // Alice act as the unique investor but not on the basis of ScopeId as alice doesn't posses the claim yet.
            assert_eq!(Statistics::investor_count(&ticker), 1);
            assert!(!ScopeIdOf::contains_key(&ticker, alice_did));

            // 1. Transfer some funds to bob_1_did.

            // 1a). Add empty compliance requirement.
            assert_ok!(ComplianceManager::add_compliance_requirement(
                alice_signed.clone(),
                ticker,
                vec![],
                vec![]
            ));

            // 1b). Should fail when transferring funds to bob_1_did because it doesn't posses scope_claim.
            // portfolio Id -
            let sender_portfolio = PortfolioId::default_portfolio(alice_did);
            let receiver_portfolio = PortfolioId::default_portfolio(bob_1_did);
            assert_err!(
                Asset::base_transfer(sender_portfolio, receiver_portfolio, &ticker, 1000),
                AssetError::InvalidTransfer
            );

            // 1c). Provide the valid scope claim.
            // Create Investor unique Id, In an ideal scenario it will be generated from the PUIS system.
            let bob_uid = generate_uid("BOB_ENTITY".to_string());
            provide_scope_claim(bob_1_did, ticker, bob_uid, cdd_provider);

            let alice_uid = generate_uid("ALICE_ENTITY".to_string());
            provide_scope_claim(alice_did, ticker, alice_uid, cdd_provider);
            let alice_scope_id = Asset::scope_id_of(&ticker, &alice_did);

            // 1d). Validate the storage changes.
            let bob_scope_id = Asset::scope_id_of(&ticker, &bob_1_did);
            assert_eq!(Asset::aggregate_balance_of(&ticker, bob_scope_id), 0);
            assert_eq!(Asset::balance_of_at_scope(bob_scope_id, bob_1_did), 0);

            assert_eq!(
                Asset::aggregate_balance_of(&ticker, alice_scope_id),
                total_supply
            );
            assert_eq!(
                Asset::balance_of_at_scope(alice_scope_id, alice_did),
                total_supply
            );

            // 1e). successfully transfer funds.
            assert_ok!(Asset::base_transfer(
                sender_portfolio,
                receiver_portfolio,
                &ticker,
                1000
            ));

            // validate the storage changes for Bob.
            assert_eq!(Asset::aggregate_balance_of(&ticker, &bob_scope_id), 1000);
            assert_eq!(Asset::balance_of_at_scope(&bob_scope_id, &bob_1_did), 1000);
            assert_eq!(Asset::balance_of(&ticker, &bob_1_did), 1000);
            assert_eq!(Statistics::investor_count(&ticker), 2);

            // validate the storage changes for Alice.
            assert_eq!(
                Asset::aggregate_balance_of(&ticker, &alice_scope_id),
                total_supply - 1000
            );
            assert_eq!(
                Asset::balance_of_at_scope(&alice_scope_id, &alice_did),
                total_supply - 1000
            );
            assert_eq!(Asset::balance_of(&ticker, &alice_did), total_supply - 1000);
            assert_eq!(Statistics::investor_count(&ticker), 2);

            // Provide scope claim to bob_2_did
            provide_scope_claim(bob_2_did, ticker, bob_uid, cdd_provider);

            // 1f). successfully transfer funds.
            assert_ok!(Asset::base_transfer(
                sender_portfolio,
                PortfolioId::default_portfolio(bob_2_did),
                &ticker,
                1000
            ));

            // validate the storage changes for Bob.
            assert_eq!(Asset::aggregate_balance_of(&ticker, &bob_scope_id), 2000);
            assert_eq!(Asset::balance_of_at_scope(&bob_scope_id, &bob_2_did), 1000);
            assert_eq!(Asset::balance_of(&ticker, &bob_2_did), 1000);
            assert_eq!(Statistics::investor_count(&ticker), 2);
        });
}

#[test]
fn next_checkpoint_is_updated() {
    ExtBuilder::default()
        .build()
        .execute_with(next_checkpoint_is_updated_we);
}

fn next_checkpoint_is_updated_we() {
    // 14 November 2023, 22:13 UTC (millisecs)
    let start: u64 = 1_700_000_000_000;
    // A period of 42 hours.
    let period = CalendarPeriod {
        unit: CalendarUnit::Hour,
        amount: 42,
    };
    // The increment in seconds.
    let period_secs = match period.to_recurring().unwrap().as_fixed_or_variable() {
        FixedOrVariableCalendarUnit::Fixed(secs) => secs,
        _ => panic!("period should be fixed"),
    };
    let period_ms = period_secs * 1000;
    Timestamp::set_timestamp(start);
    assert_eq!(start, <TestStorage as asset::Trait>::UnixTime::now());
    let alice_signed = Origin::signed(AccountKeyring::Alice.public());
    let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
    let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
    let token_name = b"NXT";
    let ticker = Ticker::try_from(&token_name[..]).unwrap();
    let total_supply = 1_000_000;
    assert_ok!(Asset::create_asset(
        alice_signed.clone(),
        token_name.into(),
        ticker,
        total_supply,
        true,
        AssetType::default(),
        vec![],
        None,
    ));
    assert_eq!(Checkpoint::schedules(ticker), Vec::new());
    let schedule = ScheduleSpec {
        start: Some(start),
        period,
        remaining: 0,
    };
    assert_ok!(Checkpoint::set_schedules_max_complexity(
        root(),
        period.complexity()
    ));
    assert_ok!(Checkpoint::create_schedule(alice_signed, ticker, schedule));
    let id = CheckpointId(1);
    assert_eq!(id, Checkpoint::checkpoint_id_sequence(&ticker));
    assert_eq!(start, Checkpoint::timestamps(ticker, id));
    assert_eq!(total_supply, Checkpoint::total_supply_at(ticker, id));
    assert_eq!(total_supply, Asset::get_balance_at(ticker, alice_did, id));
    assert_eq!(0, Asset::get_balance_at(ticker, bob_did, id));
    let checkpoint2 = start + period_ms;
    assert_eq!(vec![Some(checkpoint2)], next_checkpoints(ticker, start),);
    assert_eq!(vec![checkpoint2], checkpoint_ats(ticker));

    let transfer = |at| {
        Timestamp::set_timestamp(at);
        default_transfer(alice_did, bob_did, ticker, total_supply / 2);
    };

    // Make a transaction before the next timestamp.
    transfer(checkpoint2 - 1000);
    // Make another transaction at the checkpoint.
    // The updates are applied after the checkpoint is recorded.
    // After this transfer Alice's balance is 0.
    transfer(checkpoint2);
    // The balance after checkpoint 2.
    assert_eq!(0, Asset::balance_of(&ticker, alice_did));
    // Balances at checkpoint 2.
    let id = CheckpointId(2);
    assert_eq!(vec![start + 2 * period_ms], checkpoint_ats(ticker));
    assert_eq!(id, Checkpoint::checkpoint_id_sequence(&ticker));
    assert_eq!(start + period_ms, Checkpoint::timestamps(ticker, id));
    assert_eq!(
        total_supply / 2,
        Asset::get_balance_at(ticker, alice_did, id)
    );
    assert_eq!(total_supply / 2, Asset::get_balance_at(ticker, bob_did, id));
}

#[test]
fn non_recurring_schedule_works() {
    ExtBuilder::default()
        .build()
        .execute_with(non_recurring_schedule_works_we);
}

fn non_recurring_schedule_works_we() {
    // 14 November 2023, 22:13 UTC (millisecs)
    let start: u64 = 1_700_000_000_000;
    // Non-recuring schedule.
    let period = CalendarPeriod::default();
    Timestamp::set_timestamp(start);
    assert_eq!(start, <TestStorage as asset::Trait>::UnixTime::now());
    let alice_signed = Origin::signed(AccountKeyring::Alice.public());
    let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
    let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
    let token_name = b"NXT";
    let ticker = Ticker::try_from(&token_name[..]).unwrap();
    let total_supply = 1_000_000;
    assert_ok!(Asset::create_asset(
        alice_signed.clone(),
        token_name.into(),
        ticker,
        total_supply,
        true,
        AssetType::default(),
        vec![],
        None,
    ));
    assert_eq!(Checkpoint::schedules(ticker), Vec::new());
    let schedule = ScheduleSpec {
        start: Some(start),
        period,
        remaining: 0,
    };
    assert_ok!(Checkpoint::set_schedules_max_complexity(
        root(),
        period.complexity()
    ));
    assert_ok!(Checkpoint::create_schedule(alice_signed, ticker, schedule));
    let id = CheckpointId(1);
    assert_eq!(id, Checkpoint::checkpoint_id_sequence(&ticker));
    assert_eq!(start, Checkpoint::timestamps(ticker, id));
    assert_eq!(total_supply, Checkpoint::total_supply_at(ticker, id));
    assert_eq!(total_supply, Asset::get_balance_at(ticker, alice_did, id));
    assert_eq!(0, Asset::get_balance_at(ticker, bob_did, id));
    // The schedule will not recur.
    assert_eq!(Checkpoint::schedules(ticker), Vec::new());
}

fn checkpoint_ats(ticker: Ticker) -> Vec<u64> {
    Checkpoint::schedules(ticker)
        .into_iter()
        .map(|s| s.at)
        .collect()
}

fn next_checkpoints(ticker: Ticker, start: u64) -> Vec<Option<u64>> {
    Checkpoint::schedules(ticker)
        .into_iter()
        .map(|s| s.schedule.next_checkpoint(start))
        .collect()
}

#[test]
fn schedule_remaining_works() {
    ExtBuilder::default().build().execute_with(|| {
        let start = 1_000;
        Timestamp::set_timestamp(start);

        let alice = User::new(AccountKeyring::Alice);
        let bob = User::new(AccountKeyring::Bob);

        // Create the asset.
        let token_name = b"NXT";
        let ticker = Ticker::try_from(&token_name[..]).unwrap();
        assert_ok!(Asset::create_asset(
            alice.origin(),
            token_name.into(),
            ticker,
            1_000_000,
            true,
            AssetType::default(),
            vec![],
            None,
        ));

        let transfer = |at: Moment| {
            Timestamp::set_timestamp(at * 1_000);
            default_transfer(alice.did, bob.did, ticker, 1);
        };
        let collect_ts = |sh_id| {
            Checkpoint::schedule_points(ticker, sh_id)
                .into_iter()
                .map(|cp| Checkpoint::timestamps(ticker, cp))
                .collect::<Vec<_>>()
        };

        // No schedules yet.
        assert_eq!(Checkpoint::schedules(ticker), vec![]);

        // For simplicity, we use 1s = 1_000ms periods.
        let period = CalendarPeriod {
            unit: CalendarUnit::Second,
            amount: 1,
        };

        // Allow such a schedule to be added on-chain. Otherwise, we'll have errors.
        assert_ok!(Checkpoint::set_schedules_max_complexity(
            root(),
            period.complexity()
        ));

        // Create a schedule with one remaining and where `start == now`.
        let mut spec = ScheduleSpec {
            start: Some(start),
            period,
            remaining: 1,
        };
        let schedule = CheckpointSchedule { start, period };
        assert_ok!(Checkpoint::create_schedule(alice.origin(), ticker, spec));

        // We had `remaining == 1` and `start == now`,
        // so since a CP was created, hence `remaining => 0`,
        // the schedule was immediately evicted.
        assert_eq!(Checkpoint::schedules(ticker), vec![]);
        assert_eq!(collect_ts(ScheduleId(1)), vec![start]);

        // This time, we set `remaining == 5`, but we still have `start == now`,
        // thus one CP is immediately created, so `remaining => 4`.
        spec.remaining = 5;
        let id2 = ScheduleId(2);
        let assert_ts = |ticks| {
            assert_eq!(
                collect_ts(id2),
                (1..=ticks).map(|x| x * 1_000).collect::<Vec<_>>()
            );
        };
        let assert_sh = |at: Moment, remaining| {
            assert_eq!(
                Checkpoint::schedules(ticker),
                vec![StoredSchedule {
                    id: id2,
                    schedule,
                    at: 1_000 * at,
                    remaining,
                }]
            );
        };
        assert_ok!(Checkpoint::create_schedule(alice.origin(), ticker, spec));
        assert_sh(2, 4);
        assert_ts(1);

        // Transfer and move through the 2nd to 4th recurrences.
        for i in 2..5 {
            transfer(i);
            assert_sh(i + 1, spec.remaining - i as u32);
            assert_ts(i);
        }

        // Transfer and move to the 5th (last) recurrence.
        // We've to the point where there are no ticks left.
        transfer(5);
        assert_eq!(Checkpoint::schedules(ticker), vec![]);
        assert_ts(5);
    });
}

#[test]
fn mesh_1531_ts_collission_regression_test() {
    ExtBuilder::default().build().execute_with(|| {
        // Create the assets.
        let owner = User::new(AccountKeyring::Alice);
        let asset = |name: &[u8]| {
            let ticker = Ticker::try_from(&name[..]).unwrap();
            assert_ok!(Asset::create_asset(
                owner.origin(),
                name.into(),
                ticker,
                1_000_000,
                true,
                AssetType::default(),
                vec![],
                None,
            ));
            ticker
        };
        let alpha = asset(b"ALPHA");
        let beta = asset(b"BETA");

        // First CP is made at 1s.
        let cp = CheckpointId(1);
        Timestamp::set_timestamp(1_000);
        assert_ok!(Checkpoint::create_checkpoint(owner.origin(), alpha));
        assert_eq!(Checkpoint::timestamps(alpha, cp), 1_000);

        // Second CP is for beta, using same ID.
        Timestamp::set_timestamp(2_000);
        assert_ok!(Checkpoint::create_checkpoint(owner.origin(), beta));
        assert_eq!(Checkpoint::timestamps(alpha, cp), 1_000);
        assert_eq!(Checkpoint::timestamps(beta, cp), 2_000);
    });
}
