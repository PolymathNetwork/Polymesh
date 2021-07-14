use super::{
    storage::{get_last_auth_id, TestStorage, User},
    ExtBuilder,
};
use frame_support::{assert_noop, assert_ok, StorageMap};
use polymesh_common_utilities::traits::transaction_payment::CddAndFeeDetails;
use polymesh_primitives::Signatory;
use polymesh_runtime_develop::{fee_details::CddHandler, runtime::Call};
use test_client::AccountKeyring;

type Relayer = pallet_relayer::Module<TestStorage>;
type Identity = pallet_identity::Module<TestStorage>;
type AccountKeyRefCount = pallet_identity::AccountKeyRefCount<TestStorage>;

type Error = pallet_relayer::Error<TestStorage>;

// Relayer Test Helper functions
// =======================================

macro_rules! assert_key_usage {
    ($user:expr, $usage:expr) => {
        assert_eq!(AccountKeyRefCount::get(&$user.acc()), $usage);
    };
}

#[test]
fn basic_relayer_paying_key_test() {
    ExtBuilder::default()
        .monied(true)
        .build()
        .execute_with(&do_basic_relayer_paying_key_test);
}
fn do_basic_relayer_paying_key_test() {
    let bob = User::new(AccountKeyring::Bob);
    let alice = User::new(AccountKeyring::Alice);
    let dave = User::new(AccountKeyring::Dave);

    // add authorization for using Alice as the paying key for bob
    assert_ok!(Relayer::set_paying_key(
        alice.origin(),
        bob.acc(),
        0u128.into()
    ));

    // No paying key is used yet.
    assert_key_usage!(alice, 0);
    assert_key_usage!(dave, 0);
    assert_key_usage!(bob, 0);

    // Bob accept's the paying key
    TestStorage::set_current_identity(&bob.did);
    let auth_id = get_last_auth_id(&Signatory::Account(bob.acc()));
    assert_ok!(Relayer::accept_paying_key(bob.origin(), auth_id));

    // check alice's key is be used.
    assert_key_usage!(alice, 1);
    // check bob's key is be used.
    assert_key_usage!(bob, 1);

    // Bob tries to increase his Polyx limit.  Not allowed
    TestStorage::set_current_identity(&bob.did);
    assert_noop!(
        Relayer::update_polyx_limit(bob.origin(), bob.acc(), 12345u128),
        Error::NotPayingKey
    );

    // Alice updates the Polyx limit for Bob.  Allowed
    TestStorage::set_current_identity(&alice.did);
    assert_ok!(Relayer::update_polyx_limit(
        alice.origin(),
        bob.acc(),
        10_000u128
    ));

    // Dave tries to remove the paying key from Bob's key.  Not allowed.
    TestStorage::set_current_identity(&dave.did);
    assert_noop!(
        Relayer::remove_paying_key(dave.origin(), bob.acc(), alice.acc(),),
        Error::NotAuthorizedForUserKey
    );

    // add authorization for using Dave as the paying key for bob
    TestStorage::set_current_identity(&dave.did);
    assert_ok!(Relayer::set_paying_key(
        dave.origin(),
        bob.acc(),
        0u128.into()
    ));

    // Bob tries to accept the new paying key, but he already has a paying key
    TestStorage::set_current_identity(&bob.did);
    let auth_id = get_last_auth_id(&Signatory::Account(bob.acc()));
    assert_noop!(
        Relayer::accept_paying_key(bob.origin(), auth_id),
        Error::AlreadyHasPayingKey
    );

    // Alice tries to remove the paying key from Bob's key.  Allowed.
    TestStorage::set_current_identity(&alice.did);
    assert_ok!(Relayer::remove_paying_key(
        alice.origin(),
        bob.acc(),
        alice.acc(),
    ));

    // check alice's key is not used any more.
    assert_key_usage!(alice, 0);
}

#[test]
fn relayer_accept_cdd_and_fees_test() {
    ExtBuilder::default()
        .monied(true)
        .build()
        .execute_with(&do_relayer_accept_cdd_and_fees_test);
}
fn do_relayer_accept_cdd_and_fees_test() {
    let alice = User::new(AccountKeyring::Alice);
    let bob = User::new(AccountKeyring::Bob);
    let bob_sign = Signatory::Account(bob.acc());

    // Alice creates authoration to subsidise for Bob.
    assert_ok!(Relayer::set_paying_key(
        alice.origin(),
        bob.acc(),
        0u128.into()
    ));
    let bob_auth_id = get_last_auth_id(&bob_sign);

    // Check that Bob can accept the subsidy with Alice paying for the transaction.
    assert_eq!(
        CddHandler::get_valid_payer(
            &Call::Relayer(pallet_relayer::Call::accept_paying_key(bob_auth_id)),
            &bob.acc()
        ),
        Ok(Some(alice.acc()))
    );
}
