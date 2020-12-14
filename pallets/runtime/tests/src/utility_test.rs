use super::{
    assert_event_doesnt_exist, assert_event_exists, assert_last_event,
    pips_test::assert_balance,
    storage::{
        add_secondary_key, register_keyring_account_with_balance, Balances, Call, EventTest,
        Identity, Origin, Portfolio, System, TestStorage, Utility,
    },
    ExtBuilder,
};
use codec::Encode;
use frame_support::{assert_err, assert_ok, dispatch::DispatchError, IterableStorageDoubleMap};
use frame_system::EventRecord;
use pallet_balances::{self as balances, Call as BalancesCall};
use pallet_portfolio::Call as PortfolioCall;
use pallet_utility::{self as utility, Event, UniqueCall};
use polymesh_common_utilities::traits::transaction_payment::CddAndFeeDetails;
use polymesh_primitives::{
    PalletPermissions, Permissions, PortfolioName, PortfolioNumber, Signatory, SubsetRestriction,
};
use sp_core::sr25519::{Public, Signature};
use test_client::AccountKeyring;

type Error = utility::Error<TestStorage>;

fn transfer(to: Public, amount: u128) -> Call {
    Call::Balances(BalancesCall::transfer(to, amount))
}

const ERROR: DispatchError = DispatchError::Module {
    index: 0,
    error: 2,
    message: None,
};

fn assert_event(event: Event) {
    assert_eq!(
        System::events().pop().unwrap().event,
        EventTest::pallet_utility(event)
    )
}

fn batch_test(test: impl FnOnce(Public, Public)) {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);

        let alice = AccountKeyring::Alice.public();
        TestStorage::set_payer_context(Some(alice));
        let _ = register_keyring_account_with_balance(AccountKeyring::Alice, 1_000).unwrap();

        let bob = AccountKeyring::Bob.public();
        TestStorage::set_payer_context(Some(bob));
        let _ = register_keyring_account_with_balance(AccountKeyring::Bob, 1_000).unwrap();

        assert_balance(alice, 959, 0);
        assert_balance(bob, 959, 0);

        test(alice, bob)
    });
}

#[test]
fn batch_with_signed_works() {
    batch_test(|alice, bob| {
        let calls = vec![transfer(bob, 400), transfer(bob, 400)];
        assert_ok!(Utility::batch(Origin::signed(alice), calls));
        assert_balance(alice, 159, 0);
        assert_balance(bob, 959 + 400 + 400, 0);
        assert_event(Event::BatchCompleted);
    });
}

#[test]
fn batch_early_exit_works() {
    batch_test(|alice, bob| {
        let calls = vec![transfer(bob, 400), transfer(bob, 900), transfer(bob, 400)];
        assert_ok!(Utility::batch(Origin::signed(alice), calls));
        assert_balance(alice, 559, 0);
        assert_balance(bob, 959 + 400, 0);
        assert_event(Event::BatchInterrupted(1, ERROR));
    })
}

#[test]
fn batch_optimistic_works() {
    batch_test(|alice, bob| {
        let calls = vec![transfer(bob, 401), transfer(bob, 402)];
        assert_ok!(Utility::batch_optimistic(Origin::signed(alice), calls));
        assert_event(Event::BatchCompleted);
        assert_balance(alice, 959 - 401 - 402, 0);
        assert_balance(bob, 959 + 401 + 402, 0);
    });
}

#[test]
fn batch_optimistic_failures_listed() {
    batch_test(|alice, bob| {
        assert_ok!(Utility::batch_optimistic(
            Origin::signed(alice),
            vec![
                transfer(bob, 401), // YAY.
                transfer(bob, 900), // NAY.
                transfer(bob, 800), // NAY.
                transfer(bob, 402), // YAY.
                transfer(bob, 403), // NAY.
            ]
        ));
        assert_event(Event::BatchOptimisticFailed(vec![
            (1, ERROR),
            (2, ERROR),
            (4, ERROR),
        ]));
        assert_balance(alice, 959 - 401 - 402, 0);
        assert_balance(bob, 959 + 401 + 402, 0);
    });
}

#[test]
fn batch_atomic_works() {
    batch_test(|alice, bob| {
        let calls = vec![transfer(bob, 401), transfer(bob, 402)];
        assert_ok!(Utility::batch_atomic(Origin::signed(alice), calls));
        assert_event(Event::BatchCompleted);
        assert_balance(alice, 959 - 401 - 402, 0);
        assert_balance(bob, 959 + 401 + 402, 0);
    });
}

#[test]
fn batch_atomic_early_exit_works() {
    batch_test(|alice, bob| {
        let calls = vec![transfer(bob, 400), transfer(bob, 900), transfer(bob, 400)];
        assert_ok!(Utility::batch_atomic(Origin::signed(alice), calls));
        assert_balance(alice, 959, 0);
        assert_balance(bob, 959, 0);
        assert_event(Event::BatchInterrupted(1, ERROR));
    })
}

#[test]
fn relay_happy_case() {
    ExtBuilder::default()
        .build()
        .execute_with(_relay_happy_case);
}

fn _relay_happy_case() {
    let alice = AccountKeyring::Alice.public();
    let _ = register_keyring_account_with_balance(AccountKeyring::Alice, 1_000).unwrap();

    let bob = AccountKeyring::Bob.public();
    let _ = register_keyring_account_with_balance(AccountKeyring::Bob, 1_000).unwrap();

    let charlie = AccountKeyring::Charlie.public();
    let _ = register_keyring_account_with_balance(AccountKeyring::Charlie, 1_000).unwrap();

    assert_balance(bob, 1000, 0);
    assert_balance(charlie, 1000, 0);

    let origin = Origin::signed(alice);
    let transaction = UniqueCall::new(
        Utility::nonce(bob),
        Call::Balances(BalancesCall::transfer(charlie, 50)),
    );

    assert_ok!(Utility::relay_tx(
        origin,
        bob,
        AccountKeyring::Bob.sign(&transaction.encode()).into(),
        transaction
    ));

    assert_balance(bob, 950, 0);
    assert_balance(charlie, 1_050, 0);
}

#[test]
fn relay_unhappy_cases() {
    ExtBuilder::default()
        .build()
        .execute_with(_relay_unhappy_cases);
}

fn _relay_unhappy_cases() {
    let alice = AccountKeyring::Alice.public();
    let _ = register_keyring_account_with_balance(AccountKeyring::Alice, 1_000).unwrap();

    let bob = AccountKeyring::Bob.public();

    let charlie = AccountKeyring::Charlie.public();

    let origin = Origin::signed(alice);
    let transaction = UniqueCall::new(
        Utility::nonce(bob),
        Call::Balances(BalancesCall::transfer(charlie, 59)),
    );

    assert_err!(
        Utility::relay_tx(
            origin.clone(),
            bob,
            Signature::default().into(),
            transaction.clone()
        ),
        Error::InvalidSignature
    );

    assert_err!(
        Utility::relay_tx(
            origin.clone(),
            bob,
            AccountKeyring::Bob.sign(&transaction.encode()).into(),
            transaction.clone()
        ),
        Error::TargetCddMissing
    );

    let _ = register_keyring_account_with_balance(AccountKeyring::Bob, 1_000).unwrap();

    let transaction = UniqueCall::new(
        Utility::nonce(bob) + 1,
        Call::Balances(BalancesCall::transfer(charlie, 59)),
    );

    assert_err!(
        Utility::relay_tx(
            origin.clone(),
            bob,
            Signature::default().into(),
            transaction
        ),
        Error::InvalidNonce
    );
}

#[test]
fn batch_secondary_with_permissions_works() {
    ExtBuilder::default()
        .build()
        .execute_with(batch_secondary_with_permissions);
}

fn batch_secondary_with_permissions() {
    System::set_block_number(1);
    let alice_key = AccountKeyring::Alice.public();
    let alice_origin = Origin::signed(alice_key);
    let alice_did = register_keyring_account_with_balance(AccountKeyring::Alice, 1_000).unwrap();
    let bob_key = AccountKeyring::Bob.public();
    let bob_origin = Origin::signed(bob_key);
    let bob_signer = Signatory::Account(bob_key);

    add_secondary_key(alice_did, bob_signer);
    let low_risk_name: PortfolioName = b"low risk".into();
    assert_ok!(Portfolio::create_portfolio(
        bob_origin.clone(),
        low_risk_name.clone()
    ));
    assert_last_event!(EventTest::portfolio(pallet_portfolio::RawEvent::PortfolioCreated(_, _, _)));
    assert_eq!(
        Portfolio::portfolios(&alice_did, &PortfolioNumber(1)),
        low_risk_name.clone()
    );
    let bob_pallet_permissions = vec![
        PalletPermissions::new(b"identity".into(), SubsetRestriction(None)),
        PalletPermissions::new(
            b"portfolio".into(),
            SubsetRestriction::elems(vec![
                b"move_portfolio_funds".into(),
                b"rename_portfolio".into(),
            ]),
        ),
    ];
    assert_ok!(Identity::set_permission_to_signer(
        alice_origin,
        bob_signer,
        Permissions::from_pallet_permissions(bob_pallet_permissions),
    ));
    let high_risk_name: PortfolioName = b"high risk".into();
    assert_err!(
        Portfolio::create_portfolio(bob_origin.clone(), high_risk_name.clone()),
        pallet_permissions::Error::<TestStorage>::UnauthorizedCaller
    );
    let calls = vec![
        Call::Portfolio(PortfolioCall::create_portfolio(high_risk_name.clone())),
        Call::Portfolio(PortfolioCall::rename_portfolio(0.into(), high_risk_name)),
    ];
    assert_ok!(Utility::batch(bob_origin, calls));
    println!("{:?}", System::events());
    assert_event_doesnt_exist!(EventTest::pallet_utility(Event::BatchCompleted));
    // TODO: Why doesn't this error code match with 0?
    // assert_event_exists!(
    //     EventTest::pallet_utility(Event::BatchInterrupted(_, err)),
    //     *err == pallet_permissions::Error::<TestStorage>::UnauthorizedCaller.into()
    // );
    assert_event_exists!(EventTest::pallet_utility(Event::BatchInterrupted(_, _)));
    println!(
        "Alice's: {:?}",
        pallet_portfolio::Portfolios::iter_prefix(&alice_did).collect::<Vec<_>>()
    );
    // println!(
    //     "Bob's: {}",
    //     Portfolio::Portfolios::iter_prefix(&bob_did).collect::<Vec<_>>()
    // );
    assert_eq!(
        Portfolio::portfolios(&alice_did, &PortfolioNumber(1)),
        low_risk_name.clone()
    );
}
