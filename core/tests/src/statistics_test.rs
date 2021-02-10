use super::{
    storage::{make_account, make_account_with_scope, register_keyring_account, TestStorage},
    ExtBuilder,
};
use frame_support::{assert_noop, assert_ok};
use pallet_asset::{self as asset, SecurityToken};
use pallet_compliance_manager as compliance_manager;
use pallet_statistics::{self as statistics, TransferManager};
use polymesh_primitives::{asset::AssetType, IdentityId, PortfolioId, Ticker};
use sp_arithmetic::Permill;
use sp_core::sr25519::Public;
use sp_std::convert::TryFrom;
use test_client::AccountKeyring;

type Origin = <TestStorage as frame_system::Trait>::Origin;
type Asset = asset::Module<TestStorage>;
type Statistic = statistics::Module<TestStorage>;
type ComplianceManager = compliance_manager::Module<TestStorage>;
type Error = statistics::Error<TestStorage>;
type AssetError = asset::Error<TestStorage>;

fn create_token(token_name: &[u8], ticker: Ticker, keyring: Public) {
    assert_ok!(Asset::create_asset(
        Origin::signed(keyring),
        token_name.into(),
        ticker,
        1_000_000,
        true,
        AssetType::default(),
        vec![],
        None,
    ));
    assert_ok!(ComplianceManager::add_compliance_requirement(
        Origin::signed(keyring),
        ticker,
        vec![],
        vec![]
    ));
}

#[track_caller]
fn do_valid_transfer(ticker: Ticker, from: IdentityId, to: IdentityId, amount: u128) {
    assert_ok!(Asset::base_transfer(
        PortfolioId::default_portfolio(from),
        PortfolioId::default_portfolio(to),
        &ticker,
        amount
    ));
}

#[track_caller]
fn ensure_invalid_transfer(ticker: Ticker, from: IdentityId, to: IdentityId, amount: u128) {
    assert_noop!(
        Asset::base_transfer(
            PortfolioId::default_portfolio(from),
            PortfolioId::default_portfolio(to),
            &ticker,
            amount
        ),
        AssetError::InvalidTransfer
    );
}

#[test]
fn investor_count() {
    ExtBuilder::default()
        .build()
        .execute_with(|| investor_count_with_ext);
}

fn investor_count_with_ext() {
    let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
    let alice_signed = Origin::signed(AccountKeyring::Alice.public());
    let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
    let charlie_did = register_keyring_account(AccountKeyring::Charlie).unwrap();

    // 1. Alice create an asset.
    let token = SecurityToken {
        name: vec![0x01].into(),
        owner_did: alice_did,
        total_supply: 1_000_000,
        divisible: true,
        ..Default::default()
    };

    let identifiers = Vec::new();
    let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
    assert_ok!(Asset::create_asset(
        alice_signed.clone(),
        token.name.clone(),
        ticker,
        1_000_000, // Total supply over the limit
        true,
        token.asset_type.clone(),
        identifiers.clone(),
        None,
    ));

    let ticker = Ticker::try_from(token.name.as_slice()).unwrap();
    assert_ok!(ComplianceManager::add_compliance_requirement(
        alice_signed.clone(),
        ticker,
        vec![],
        vec![]
    ));

    let unsafe_transfer = |from, to, value| {
        assert_ok!(Asset::unsafe_transfer(
            PortfolioId::default_portfolio(from),
            PortfolioId::default_portfolio(to),
            &ticker,
            value,
        ));
    };

    // Alice sends some tokens to Bob. Token has only one investor.
    unsafe_transfer(alice_did, bob_did, 500);
    assert_eq!(Statistic::investor_count(&ticker), 2);

    // Alice sends some tokens to Charlie. Token has now two investors.
    unsafe_transfer(alice_did, charlie_did, 5000);
    assert_eq!(Statistic::investor_count(&ticker), 3);

    // Bob sends all his tokens to Charlie, so now we have one investor again.
    unsafe_transfer(bob_did, charlie_did, 500);
    assert_eq!(Statistic::investor_count(&ticker), 2);
}

#[test]
fn should_add_tm() {
    ExtBuilder::default().build().execute_with(|| {
        let (token_owner_signed, _token_owner_did) =
            make_account(AccountKeyring::Alice.public()).unwrap();

        let token_name = b"ACME";
        let ticker = Ticker::try_from(&token_name[..]).unwrap();
        create_token(token_name, ticker, AccountKeyring::Alice.public());

        let tms = (0..3u64)
            .map(TransferManager::CountTransferManager)
            .collect::<Vec<_>>();

        let add_tm = |tm| Statistic::add_transfer_manager(token_owner_signed.clone(), ticker, tm);
        assert_ok!(add_tm(tms[0].clone()));
        assert_eq!(Statistic::transfer_managers(ticker), [tms[0].clone()]);

        assert_noop!(add_tm(tms[0].clone()), Error::DuplicateTransferManager);

        for tm in &tms[1..3] {
            assert_ok!(add_tm(tm.clone()));
        }

        assert_eq!(Statistic::transfer_managers(ticker), tms);

        assert_noop!(
            add_tm(TransferManager::CountTransferManager(1000000)),
            Error::TransferManagersLimitReached
        );
    });
}

#[test]
fn should_remove_tm() {
    ExtBuilder::default().build().execute_with(|| {
        let (token_owner_signed, _token_owner_did) =
            make_account(AccountKeyring::Alice.public()).unwrap();

        let token_name = b"ACME";
        let ticker = Ticker::try_from(&token_name[..]).unwrap();
        create_token(token_name, ticker, AccountKeyring::Alice.public());

        let mut tms = Vec::new();

        for i in 0..3u64 {
            tms.push(TransferManager::CountTransferManager(i));
            assert_ok!(Statistic::add_transfer_manager(
                token_owner_signed.clone(),
                ticker,
                tms[i as usize].clone()
            ));
            assert_eq!(Statistic::transfer_managers(ticker), tms);
        }

        for _ in 0..3u64 {
            let tm = tms.pop().unwrap();
            assert_ok!(Statistic::remove_transfer_manager(
                token_owner_signed.clone(),
                ticker,
                tm
            ));
            assert_eq!(Statistic::transfer_managers(ticker), tms);
        }
    });
}

#[test]
fn should_add_remove_exempted_entities() {
    ExtBuilder::default().build().execute_with(|| {
        let (token_owner_signed, token_owner_did) =
            make_account(AccountKeyring::Alice.public()).unwrap();

        let token_name = b"ACME";
        let ticker = Ticker::try_from(&token_name[..]).unwrap();
        create_token(token_name, ticker, AccountKeyring::Alice.public());

        let tm = TransferManager::CountTransferManager(1000000);
        let assert_exemption = |boolean| {
            assert_eq!(
                Statistic::entity_exempt((ticker, tm.clone()), token_owner_did),
                boolean
            )
        };
        assert_exemption(false);
        assert_ok!(Statistic::add_exempted_entities(
            token_owner_signed.clone(),
            ticker,
            tm.clone(),
            vec![token_owner_did]
        ));
        assert_exemption(true);
        assert_ok!(Statistic::remove_exempted_entities(
            token_owner_signed.clone(),
            ticker,
            tm.clone(),
            vec![token_owner_did]
        ));
        assert_exemption(false);
    });
}

#[test]
fn should_verify_tms() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let token_name = b"ACME";
            let ticker = Ticker::try_from(&token_name[..]).unwrap();
            let setup_account = |keyring: AccountKeyring| {
                make_account_with_scope(keyring.public(), ticker, AccountKeyring::Eve.public())
                    .unwrap()
            };
            let (alice_signed, alice_did, alice_scope) = setup_account(AccountKeyring::Alice);
            let (_, bob_did, _) = setup_account(AccountKeyring::Bob);
            let (_, char_did, char_scope) = setup_account(AccountKeyring::Charlie);
            let (_, dave_did, dave_scope) = setup_account(AccountKeyring::Dave);
            create_token(token_name, ticker, AccountKeyring::Alice.public());
            assert_eq!(Statistic::investor_count(&ticker), 1);

            // No TM attached, transfer should be valid
            do_valid_transfer(ticker, alice_did, bob_did, 10);
            assert_eq!(Statistic::investor_count(&ticker), 2);
            let add_tm = |tm| Statistic::add_transfer_manager(alice_signed.clone(), ticker, tm);
            let add_ctm = |limit| {
                add_tm(TransferManager::CountTransferManager(limit)).expect("Failed to add CTM")
            };
            // Count TM attached with max investors limit reached already
            add_ctm(2);
            // Transfer that increases the investor count beyond the limit should fail
            ensure_invalid_transfer(ticker, bob_did, char_did, 5);
            // Transfer that keeps the investor count the same should succeed
            do_valid_transfer(ticker, bob_did, alice_did, 1);

            let add_exemption = |tm, scopes| {
                assert_ok!(Statistic::add_exempted_entities(
                    alice_signed.clone(),
                    ticker,
                    tm,
                    scopes
                ));
            };

            // Add some folks to the exemption list
            add_exemption(
                TransferManager::CountTransferManager(2),
                vec![alice_scope, dave_scope],
            );
            // Transfer should fail when receiver is exempted but not sender for CTM
            ensure_invalid_transfer(ticker, bob_did, dave_did, 5);
            // Transfer should succeed since sender is exempted
            do_valid_transfer(ticker, alice_did, char_did, 5);

            // Bump CTM to 10
            assert_ok!(Statistic::remove_transfer_manager(
                alice_signed.clone(),
                ticker,
                TransferManager::CountTransferManager(2)
            ));
            add_ctm(10);

            let ptm25 = TransferManager::PercentageTransferManager(
                Permill::from_rational_approximation(1u32, 4u32),
            );
            // Add ptm with max ownership limit of 50%
            assert_ok!(add_tm(ptm25.clone()));
            // Transfer should fail when receiver is breaching the limit
            ensure_invalid_transfer(ticker, alice_did, dave_did, 250_001);
            // Transfer should succeed when under limit
            do_valid_transfer(ticker, alice_did, dave_did, 250_000);
            // Transfer should fail when receiver is breaching the limit
            ensure_invalid_transfer(ticker, alice_did, dave_did, 1);

            // Add charlie to exemption list
            add_exemption(ptm25, vec![char_scope]);
            // Transfer should succeed since receiver is exempted
            do_valid_transfer(ticker, alice_did, char_did, 250_001);

            // Advanced scenario where charlie is limited at 30% but others at 25%
            assert_ok!(add_tm(TransferManager::PercentageTransferManager(
                Permill::from_rational_approximation(3u32, 10u32)
            )));
            // Transfer should fail when dave is breaching the default limit
            ensure_invalid_transfer(ticker, alice_did, dave_did, 1);
            // Transfer should fail when charlie is breaching the advanced limit
            ensure_invalid_transfer(ticker, alice_did, char_did, 50_000);
            // Transfer should succeed when charlie under advanced limit
            do_valid_transfer(ticker, alice_did, char_did, 25_000);
        });
}
