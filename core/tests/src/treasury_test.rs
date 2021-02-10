use super::{
    storage::{register_keyring_account, TestStorage},
    ExtBuilder,
};

use frame_support::{assert_err, assert_ok};
use pallet_balances as balances;
use pallet_identity as identity;
use pallet_treasury::{self as treasury, TreasuryTrait};
use polymesh_common_utilities::Context;
use polymesh_primitives::Beneficiary;
use sp_runtime::DispatchError;
use test_client::AccountKeyring;

pub type Balances = balances::Module<TestStorage>;
pub type Treasury = treasury::Module<TestStorage>;
type Identity = identity::Module<TestStorage>;
type Origin = <TestStorage as frame_system::Trait>::Origin;

#[test]
fn reimbursement_and_disbursement() {
    ExtBuilder::default()
        .balance_factor(10)
        .build()
        .execute_with(reimbursement_and_disbursement_we);
}

fn reimbursement_and_disbursement_we() {
    let root = Origin::from(frame_system::RawOrigin::Root);
    let alice = register_keyring_account(AccountKeyring::Alice).unwrap();
    let alice_acc = Origin::signed(AccountKeyring::Alice.public());
    let alice_pub = AccountKeyring::Alice.public();
    let bob_pub = AccountKeyring::Bob.public();
    let bob = register_keyring_account(AccountKeyring::Bob).unwrap();

    let total_issuance = Balances::total_issuance();

    // Verify reimburstement.
    assert_eq!(Treasury::balance(), 0);
    assert_ok!(Treasury::reimbursement(alice_acc.clone(), 1_000));
    assert_eq!(Treasury::balance(), 1_000);
    assert_eq!(total_issuance, Balances::total_issuance());

    // Disbursement: Only root can do that.
    let beneficiaries = vec![
        Beneficiary {
            id: alice,
            amount: 100,
        },
        Beneficiary {
            id: bob,
            amount: 500,
        },
    ];

    // Providing a random DID to Root, In an ideal world root will have a valid DID
    let before_alice_balance = Balances::free_balance(&alice_pub);
    let before_bob_balance = Balances::free_balance(&bob_pub);
    assert_ok!(Treasury::disbursement(root.clone(), beneficiaries));
    Context::set_current_identity::<Identity>(None);
    assert_eq!(Treasury::balance(), 400);
    assert_eq!(
        Balances::free_balance(&alice_pub),
        before_alice_balance + 100
    );
    assert_eq!(Balances::free_balance(&bob_pub), before_bob_balance + 500);
    assert_eq!(total_issuance, Balances::total_issuance());

    // Alice cannot make a disbursement to herself.
    assert_err!(
        Treasury::disbursement(
            alice_acc,
            vec![Beneficiary {
                id: alice,
                amount: 500
            }]
        ),
        DispatchError::BadOrigin
    );
    assert_eq!(total_issuance, Balances::total_issuance());
}
