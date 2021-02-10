use super::{
    assert_event_doesnt_exist, assert_event_exists, assert_last_event,
    pips_test::assert_balance,
    storage::{
        add_secondary_key, register_keyring_account_with_balance, Call, EventTest, Identity,
        Origin, Portfolio, System, TestStorage, User, Utility,
    },
    ExtBuilder,
};
use codec::Encode;
use frame_support::{assert_err, assert_ok, dispatch::DispatchError};
use frame_system::EventRecord;
use pallet_balances::Call as BalancesCall;
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
    let alice = User::new(AccountKeyring::Alice).balance(1_000);
    let bob_key = AccountKeyring::Bob.public();
    let bob_origin = Origin::signed(bob_key);
    let bob_signer = Signatory::Account(bob_key);
    let check_name = |name| {
        assert_eq!(Portfolio::portfolios(&alice.did, &PortfolioNumber(1)), name);
    };

    // Add Bob.
    add_secondary_key(alice.did, bob_signer);
    let low_risk_name: PortfolioName = b"low risk".into();
    assert_ok!(Portfolio::create_portfolio(
        bob_origin.clone(),
        low_risk_name.clone()
    ));
    assert_last_event!(EventTest::portfolio(
        pallet_portfolio::RawEvent::PortfolioCreated(_, _, _)
    ));
    check_name(low_risk_name.clone());

    // Set and check Bob's permissions.
    let bob_pallet_permissions = vec![
        PalletPermissions::new(b"Identity".into(), SubsetRestriction(None)),
        PalletPermissions::new(
            b"Portfolio".into(),
            SubsetRestriction::elems(vec![
                b"move_portfolio_funds".into(),
                b"rename_portfolio".into(),
            ]),
        ),
    ];
    let bob_permissions = Permissions {
        extrinsic: SubsetRestriction(Some(bob_pallet_permissions.into_iter().collect())),
        ..Permissions::default()
    };
    assert_ok!(Identity::set_permission_to_signer(
        alice.origin(),
        bob_signer,
        bob_permissions,
    ));
    let bob_secondary_key = &Identity::did_records(&alice.did).secondary_keys[0];
    let check_permission = |name: &[u8], t| {
        assert_eq!(
            t,
            bob_secondary_key.has_extrinsic_permission(&b"Portfolio".into(), &name.into())
        );
    };
    check_permission(b"rename_portfolio", true);
    check_permission(b"create_portfolio", false);

    // Call a disallowed extrinsic.
    let high_risk_name: PortfolioName = b"high risk".into();
    assert_err!(
        Portfolio::create_portfolio(bob_origin.clone(), high_risk_name.clone()),
        pallet_permissions::Error::<TestStorage>::UnauthorizedCaller
    );

    // Call one disallowed and one allowed extrinsic in a batch.
    let calls = vec![
        Call::Portfolio(PortfolioCall::create_portfolio(high_risk_name.clone())),
        Call::Portfolio(PortfolioCall::rename_portfolio(
            1u64.into(),
            high_risk_name.clone(),
        )),
    ];
    assert_ok!(Utility::batch(bob_origin.clone(), calls.clone()));
    assert_event_doesnt_exist!(EventTest::pallet_utility(Event::BatchCompleted));
    assert_event_exists!(EventTest::pallet_utility(Event::BatchInterrupted(0, _)));
    check_name(low_risk_name);

    // Call the same extrinsics optimistically.
    assert_ok!(Utility::batch_optimistic(bob_origin, calls));
    assert_event_exists!(
        EventTest::pallet_utility(Event::BatchOptimisticFailed(errors)),
        errors.len() == 1 && errors[0].0 == 0
    );
    check_name(high_risk_name);
}
