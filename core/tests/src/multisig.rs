use super::{
    storage::{get_last_auth_id, register_keyring_account, Call, TestStorage},
    ExtBuilder,
};
use frame_support::{assert_err, assert_ok, traits::OnInitialize};
use pallet_balances as balances;
use pallet_identity as identity;
use pallet_multisig as multisig;
use polymesh_common_utilities::Context;
use polymesh_primitives::{PalletPermissions, Permissions, SecondaryKey, Signatory};
use sp_core::sr25519::Public;
use test_client::AccountKeyring;

type Balances = balances::Module<TestStorage>;
type Identity = identity::Module<TestStorage>;
type MultiSig = multisig::Module<TestStorage>;
type Timestamp = pallet_timestamp::Module<TestStorage>;
type Origin = <TestStorage as frame_system::Trait>::Origin;
type Error = multisig::Error<TestStorage>;
type System = frame_system::Module<TestStorage>;
type Scheduler = pallet_scheduler::Module<TestStorage>;

pub fn next_block() {
    let block_number = System::block_number() + 1;
    System::set_block_number(block_number);
    let _ = Scheduler::on_initialize(block_number);
}

#[test]
fn create_multisig() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
        let alice = Origin::signed(AccountKeyring::Alice.public());

        let musig_address = MultiSig::get_next_multisig_address(AccountKeyring::Alice.public());

        assert_ok!(MultiSig::create_multisig(
            alice.clone(),
            vec![Signatory::from(alice_did), Signatory::from(bob_did)],
            1,
        ));

        assert_eq!(MultiSig::ms_signs_required(musig_address), 1);

        assert_err!(
            MultiSig::create_multisig(alice.clone(), vec![], 10,),
            Error::NoSigners
        );

        assert_err!(
            MultiSig::create_multisig(
                alice.clone(),
                vec![Signatory::from(alice_did), Signatory::from(bob_did)],
                0,
            ),
            Error::RequiredSignaturesOutOfBounds
        );

        assert_err!(
            MultiSig::create_multisig(
                alice.clone(),
                vec![Signatory::from(alice_did), Signatory::from(bob_did)],
                10,
            ),
            Error::RequiredSignaturesOutOfBounds
        );
    });
}

#[test]
fn join_multisig() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let alice = Origin::signed(AccountKeyring::Alice.public());
        let bob = Origin::signed(AccountKeyring::Bob.public());
        let bob_signer = Signatory::Account(AccountKeyring::Bob.public());

        let musig_address = MultiSig::get_next_multisig_address(AccountKeyring::Alice.public());

        assert_ok!(MultiSig::create_multisig(
            alice.clone(),
            vec![Signatory::from(alice_did), bob_signer],
            1,
        ));

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), Signatory::from(alice_did)),
            false
        );

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            false
        );

        let alice_auth_id = get_last_auth_id(&Signatory::from(alice_did));
        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_ok!(MultiSig::accept_multisig_signer_as_identity(
            alice.clone(),
            alice_auth_id
        ));

        let bob_auth_id = get_last_auth_id(&bob_signer);
        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_ok!(MultiSig::accept_multisig_signer_as_key(
            bob.clone(),
            bob_auth_id
        ));

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), Signatory::from(alice_did)),
            true
        );
        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            true
        );

        Context::set_current_identity::<Identity>(None);
        assert_ok!(MultiSig::create_multisig(
            alice.clone(),
            vec![Signatory::from(alice_did), bob_signer],
            1,
        ));

        let bob_auth_id2 = get_last_auth_id(&bob_signer);
        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_err!(
            MultiSig::accept_multisig_signer_as_key(bob.clone(), bob_auth_id2),
            Error::SignerAlreadyLinked
        );
    });
}

#[test]
fn change_multisig_sigs_required() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let alice = Origin::signed(AccountKeyring::Alice.public());
        let bob = Origin::signed(AccountKeyring::Bob.public());
        let bob_signer = Signatory::Account(AccountKeyring::Bob.public());

        let musig_address = MultiSig::get_next_multisig_address(AccountKeyring::Alice.public());

        assert_ok!(MultiSig::create_multisig(
            alice.clone(),
            vec![Signatory::from(alice_did), bob_signer],
            2,
        ));

        let alice_auth_id = get_last_auth_id(&Signatory::from(alice_did));
        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_ok!(MultiSig::accept_multisig_signer_as_identity(
            alice.clone(),
            alice_auth_id
        ));

        let bob_auth_id = get_last_auth_id(&bob_signer);

        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_ok!(MultiSig::accept_multisig_signer_as_key(
            bob.clone(),
            bob_auth_id
        ));

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), Signatory::from(alice_did)),
            true
        );

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            true
        );

        let call = Box::new(Call::MultiSig(multisig::Call::change_sigs_required(1)));

        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_ok!(MultiSig::create_proposal_as_key(
            bob.clone(),
            musig_address.clone(),
            call,
            None,
            false
        ));

        assert_eq!(MultiSig::ms_signs_required(musig_address.clone()), 2);

        let proposal = (musig_address.clone(), 0);
        let proposal_details = MultiSig::proposal_detail(&proposal);
        assert_eq!(
            proposal_details.status,
            multisig::ProposalStatus::ActiveOrExpired
        );

        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_ok!(MultiSig::approve_as_identity(
            alice.clone(),
            musig_address.clone(),
            0
        ));
        next_block();
        assert_eq!(MultiSig::ms_signs_required(musig_address), 1);
    });
}

#[test]
fn create_or_approve_change_multisig_sigs_required() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let alice = Origin::signed(AccountKeyring::Alice.public());
        let bob = Origin::signed(AccountKeyring::Bob.public());
        let bob_signer = Signatory::Account(AccountKeyring::Bob.public());
        let musig_address = MultiSig::get_next_multisig_address(AccountKeyring::Alice.public());
        assert_ok!(MultiSig::create_multisig(
            alice.clone(),
            vec![Signatory::from(alice_did), bob_signer],
            2,
        ));
        let alice_auth_id = get_last_auth_id(&Signatory::from(alice_did));

        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_ok!(MultiSig::accept_multisig_signer_as_identity(
            alice.clone(),
            alice_auth_id
        ));
        let bob_auth_id = get_last_auth_id(&bob_signer);
        assert_ok!(MultiSig::accept_multisig_signer_as_key(
            bob.clone(),
            bob_auth_id
        ));
        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), Signatory::from(alice_did)),
            true
        );
        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            true
        );
        let call = Box::new(Call::MultiSig(multisig::Call::change_sigs_required(1)));
        assert_ok!(MultiSig::create_or_approve_proposal_as_key(
            bob.clone(),
            musig_address.clone(),
            call.clone(),
            None,
            false
        ));
        next_block();
        assert_eq!(MultiSig::ms_signs_required(musig_address.clone()), 2);
        assert_ok!(MultiSig::create_or_approve_proposal_as_identity(
            alice.clone(),
            musig_address.clone(),
            call,
            None,
            false
        ));
        next_block();
        assert_eq!(MultiSig::ms_signs_required(musig_address), 1);
    });
}

#[test]
fn remove_multisig_signer() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let alice = Origin::signed(AccountKeyring::Alice.public());
        let alice_signer = Signatory::from(alice_did);
        let bob = Origin::signed(AccountKeyring::Bob.public());
        let bob_signer = Signatory::Account(AccountKeyring::Bob.public());

        let musig_address = MultiSig::get_next_multisig_address(AccountKeyring::Alice.public());

        assert_ok!(MultiSig::create_multisig(
            alice.clone(),
            vec![alice_signer, bob_signer],
            1,
        ));

        assert_eq!(MultiSig::number_of_signers(musig_address.clone()), 0);

        let alice_auth_id = get_last_auth_id(&alice_signer);

        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_ok!(MultiSig::accept_multisig_signer_as_identity(
            alice.clone(),
            alice_auth_id
        ));

        assert_eq!(MultiSig::number_of_signers(musig_address.clone()), 1);

        let bob_auth_id = get_last_auth_id(&bob_signer);

        assert_ok!(MultiSig::accept_multisig_signer_as_key(
            bob.clone(),
            bob_auth_id
        ));

        assert_eq!(MultiSig::number_of_signers(musig_address.clone()), 2);

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), alice_signer),
            true
        );

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            true
        );

        // No direct identity for Bob as he is only a signer
        assert_eq!(Identity::get_identity(&AccountKeyring::Bob.public()), None);
        // No identity as multisig has not been set as a secondary / primary key
        assert_eq!(Identity::get_identity(&musig_address), None);

        let call = Box::new(Call::MultiSig(multisig::Call::remove_multisig_signer(
            bob_signer,
        )));

        assert_ok!(MultiSig::create_proposal_as_identity(
            alice.clone(),
            musig_address.clone(),
            call,
            None,
            false
        ));

        next_block();

        assert_eq!(MultiSig::number_of_signers(musig_address.clone()), 1);

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), alice_signer),
            true
        );

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            false
        );

        assert_eq!(Identity::get_identity(&AccountKeyring::Bob.public()), None);

        Context::set_current_identity::<Identity>(None);

        let remove_alice = Box::new(Call::MultiSig(multisig::Call::remove_multisig_signer(
            alice_signer,
        )));

        assert_ok!(MultiSig::create_proposal_as_identity(
            alice.clone(),
            musig_address.clone(),
            remove_alice,
            None,
            false
        ));

        next_block();

        // Alice not removed since that would've broken the multi sig.
        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), alice_signer),
            true
        );
    });
}

#[test]
fn add_multisig_signer() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let alice = Origin::signed(AccountKeyring::Alice.public());
        let bob = Origin::signed(AccountKeyring::Bob.public());
        let bob_signer = Signatory::Account(AccountKeyring::Bob.public());
        let charlie = Origin::signed(AccountKeyring::Charlie.public());
        let charlie_signer = Signatory::Account(AccountKeyring::Charlie.public());

        let musig_address = MultiSig::get_next_multisig_address(AccountKeyring::Alice.public());

        assert_ok!(MultiSig::create_multisig(
            alice.clone(),
            vec![Signatory::from(alice_did)],
            1,
        ));

        let alice_auth_id = get_last_auth_id(&Signatory::from(alice_did));

        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_ok!(MultiSig::accept_multisig_signer_as_identity(
            alice.clone(),
            alice_auth_id
        ));

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), Signatory::from(alice_did)),
            true
        );
        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            false
        );

        let call = Box::new(Call::MultiSig(multisig::Call::add_multisig_signer(
            bob_signer,
        )));

        assert_ok!(MultiSig::create_proposal_as_identity(
            alice.clone(),
            musig_address.clone(),
            call,
            None,
            false
        ));

        next_block();

        let call2 = Box::new(Call::MultiSig(multisig::Call::add_multisig_signer(
            charlie_signer,
        )));

        assert_ok!(MultiSig::create_proposal_as_identity(
            alice.clone(),
            musig_address.clone(),
            call2,
            None,
            false
        ));

        next_block();

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), Signatory::from(alice_did)),
            true
        );

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            false
        );

        let bob_auth_id = get_last_auth_id(&bob_signer);
        let charlie_auth_id = get_last_auth_id(&charlie_signer);

        let root = Origin::from(frame_system::RawOrigin::Root);

        assert_ok!(MultiSig::make_multisig_primary(
            alice.clone(),
            musig_address.clone(),
            None
        ));

        assert_ok!(MultiSig::accept_multisig_signer_as_key(
            charlie.clone(),
            charlie_auth_id
        ));

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), charlie_signer),
            true
        );
        assert!(Identity::change_cdd_requirement_for_mk_rotation(root.clone(), true).is_ok());

        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_err!(
            MultiSig::accept_multisig_signer_as_key(bob.clone(), bob_auth_id),
            Error::ChangeNotAllowed
        );
        assert!(Identity::change_cdd_requirement_for_mk_rotation(root.clone(), false).is_ok());

        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_ok!(MultiSig::accept_multisig_signer_as_key(
            bob.clone(),
            bob_auth_id
        ));

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            true
        );
    });
}

#[test]
fn make_multisig_primary() {
    ExtBuilder::default().monied(true).build().execute_with(|| {
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let alice = Origin::signed(AccountKeyring::Alice.public());
        let bob = Origin::signed(AccountKeyring::Bob.public());
        let _bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();

        let musig_address = MultiSig::get_next_multisig_address(AccountKeyring::Alice.public());

        assert_ok!(MultiSig::create_multisig(
            alice.clone(),
            vec![Signatory::from(alice_did)],
            1,
        ));

        assert_eq!(
            Identity::did_records(alice_did).primary_key,
            AccountKeyring::Alice.public()
        );

        assert_err!(
            MultiSig::make_multisig_primary(bob.clone(), musig_address.clone(), None),
            Error::IdentityNotCreator
        );

        assert_ok!(MultiSig::make_multisig_primary(
            alice.clone(),
            musig_address.clone(),
            None
        ));

        assert_eq!(Identity::did_records(alice_did).primary_key, musig_address);
    });
}

#[test]
fn make_multisig_signer() {
    ExtBuilder::default().monied(true).build().execute_with(|| {
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let alice = Origin::signed(AccountKeyring::Alice.public());
        let _bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
        let bob = Origin::signed(AccountKeyring::Bob.public());

        let musig_address = MultiSig::get_next_multisig_address(AccountKeyring::Alice.public());

        assert_ok!(MultiSig::create_multisig(
            alice.clone(),
            vec![Signatory::from(alice_did)],
            1,
        ));
        let permissions =
            Permissions::from_pallet_permissions(vec![PalletPermissions::entire_pallet(
                b"multisig".as_ref().into(),
            )]);
        // The desired secondary key record.
        let musig_secondary = SecondaryKey::new(Signatory::Account(musig_address), permissions);
        let secondary_keys = Identity::did_records(alice_did).secondary_keys;
        assert!(secondary_keys
            .iter()
            .find(|si| **si == musig_secondary)
            .is_none());

        assert_err!(
            MultiSig::make_multisig_signer(bob.clone(), musig_address.clone()),
            Error::IdentityNotCreator
        );

        assert_ok!(MultiSig::make_multisig_signer(
            alice.clone(),
            musig_address.clone(),
        ));

        let secondary_keys2 = Identity::did_records(alice_did).secondary_keys;
        assert!(secondary_keys2
            .iter()
            .find(|si| **si == musig_secondary)
            .is_some());
    });
}

#[test]
fn remove_multisig_signers_via_creator() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let alice = Origin::signed(AccountKeyring::Alice.public());
        let alice_signer = Signatory::from(alice_did);
        let bob = Origin::signed(AccountKeyring::Bob.public());
        let bob_signer = Signatory::Account(AccountKeyring::Bob.public());

        let musig_address = MultiSig::get_next_multisig_address(AccountKeyring::Alice.public());

        assert_ok!(MultiSig::create_multisig(
            alice.clone(),
            vec![alice_signer, bob_signer],
            1,
        ));

        assert_eq!(MultiSig::number_of_signers(musig_address.clone()), 0);

        let alice_auth_id = get_last_auth_id(&alice_signer);

        assert_ok!(MultiSig::accept_multisig_signer_as_identity(
            alice.clone(),
            alice_auth_id
        ));

        assert_eq!(MultiSig::number_of_signers(musig_address.clone()), 1);

        let bob_auth_id = get_last_auth_id(&bob_signer);

        assert_ok!(MultiSig::accept_multisig_signer_as_key(
            bob.clone(),
            bob_auth_id
        ));

        assert_eq!(MultiSig::number_of_signers(musig_address.clone()), 2);

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), alice_signer),
            true
        );

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            true
        );

        assert_err!(
            MultiSig::remove_multisig_signers_via_creator(
                bob.clone(),
                musig_address.clone(),
                vec![bob_signer]
            ),
            Error::IdentityNotCreator
        );

        assert_ok!(MultiSig::remove_multisig_signers_via_creator(
            alice.clone(),
            musig_address.clone(),
            vec![bob_signer]
        ));

        assert_eq!(MultiSig::number_of_signers(musig_address.clone()), 1);

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), alice_signer),
            true
        );

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            false
        );

        assert_err!(
            MultiSig::remove_multisig_signers_via_creator(
                alice.clone(),
                musig_address.clone(),
                vec![alice_signer]
            ),
            Error::NotEnoughSigners
        );

        // Alice not removed since that would've broken the multi sig.
        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), alice_signer),
            true
        );
    });
}

#[test]
fn add_multisig_signers_via_creator() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let alice = Origin::signed(AccountKeyring::Alice.public());
        let bob = Origin::signed(AccountKeyring::Bob.public());
        let bob_signer = Signatory::Account(AccountKeyring::Bob.public());

        let musig_address = MultiSig::get_next_multisig_address(AccountKeyring::Alice.public());

        assert_ok!(MultiSig::create_multisig(
            alice.clone(),
            vec![Signatory::from(alice_did)],
            1,
        ));

        let alice_auth_id = get_last_auth_id(&Signatory::from(alice_did));

        assert_ok!(MultiSig::accept_multisig_signer_as_identity(
            alice.clone(),
            alice_auth_id
        ));

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), Signatory::from(alice_did)),
            true
        );
        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            false
        );

        assert_err!(
            MultiSig::add_multisig_signers_via_creator(
                bob.clone(),
                musig_address.clone(),
                vec![bob_signer]
            ),
            Error::IdentityNotCreator
        );

        assert_ok!(MultiSig::add_multisig_signers_via_creator(
            alice.clone(),
            musig_address.clone(),
            vec![bob_signer]
        ));

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), Signatory::from(alice_did)),
            true
        );

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            false
        );

        let bob_auth_id = get_last_auth_id(&bob_signer);

        assert_ok!(MultiSig::accept_multisig_signer_as_key(
            bob.clone(),
            bob_auth_id
        ));

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            true
        );
    });
}

#[test]
fn check_for_approval_closure() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let eve_did = register_keyring_account(AccountKeyring::Eve).unwrap();
        let alice = Origin::signed(AccountKeyring::Alice.public());
        let eve = Origin::signed(AccountKeyring::Eve.public());
        let bob_signer = Signatory::Account(AccountKeyring::Bob.public());

        let musig_address = MultiSig::get_next_multisig_address(AccountKeyring::Alice.public());

        assert_ok!(MultiSig::create_multisig(
            alice.clone(),
            vec![Signatory::from(alice_did), Signatory::from(eve_did)],
            1,
        ));

        let alice_auth_id = get_last_auth_id(&Signatory::from(alice_did));

        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_ok!(MultiSig::accept_multisig_signer_as_identity(
            alice.clone(),
            alice_auth_id
        ));

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), Signatory::from(alice_did)),
            true
        );

        let eve_auth_id = get_last_auth_id(&Signatory::from(eve_did));

        Context::set_current_identity::<Identity>(Some(eve_did));
        assert_ok!(MultiSig::accept_multisig_signer_as_identity(
            eve.clone(),
            eve_auth_id
        ));

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), Signatory::from(eve_did)),
            true
        );

        assert_eq!(
            MultiSig::ms_signers(musig_address.clone(), bob_signer),
            false
        );

        let call = Box::new(Call::MultiSig(multisig::Call::add_multisig_signer(
            bob_signer,
        )));
        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_ok!(MultiSig::create_proposal_as_identity(
            alice.clone(),
            musig_address.clone(),
            call.clone(),
            None,
            false
        ));
        next_block();
        let proposal_id = MultiSig::proposal_ids(musig_address.clone(), call).unwrap();
        let bob_auth_id = get_last_auth_id(&bob_signer);
        let multi_purpose_nonce = Identity::multi_purpose_nonce();

        Context::set_current_identity::<Identity>(Some(eve_did));
        assert_ok!(MultiSig::approve_as_identity(
            eve.clone(),
            musig_address.clone(),
            proposal_id
        ));
        next_block();
        let after_extra_approval_auth_id = get_last_auth_id(&bob_signer);
        let after_extra_approval_multi_purpose_nonce = Identity::multi_purpose_nonce();
        // To validate that no new auth is created
        assert_eq!(bob_auth_id, after_extra_approval_auth_id);
        assert_eq!(
            multi_purpose_nonce,
            after_extra_approval_multi_purpose_nonce
        );
        assert_eq!(
            MultiSig::proposal_detail(&(musig_address.clone(), proposal_id)).approvals,
            2
        );
    });
}

#[test]
fn reject_proposals() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let alice = Origin::signed(AccountKeyring::Alice.public());

        let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
        let bob = Origin::signed(AccountKeyring::Bob.public());

        let charlie_did = register_keyring_account(AccountKeyring::Charlie).unwrap();
        let charlie = Origin::signed(AccountKeyring::Charlie.public());

        let dave_did = register_keyring_account(AccountKeyring::Dave).unwrap();
        let dave = Origin::signed(AccountKeyring::Dave.public());

        let eve_key = AccountKeyring::Eve.public();
        let eve = Origin::signed(AccountKeyring::Eve.public());

        let musig_address = MultiSig::get_next_multisig_address(AccountKeyring::Alice.public());

        setup_multisig(
            alice.clone(),
            3,
            vec![
                Signatory::from(alice_did),
                Signatory::from(bob_did),
                Signatory::from(charlie_did),
                Signatory::from(dave_did),
                Signatory::Account(eve_key),
            ],
        );

        let call1 = Box::new(Call::MultiSig(multisig::Call::change_sigs_required(4)));
        let call2 = Box::new(Call::MultiSig(multisig::Call::change_sigs_required(5)));
        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_ok!(MultiSig::create_proposal_as_identity(
            alice.clone(),
            musig_address.clone(),
            call1.clone(),
            None,
            false
        ));
        assert_ok!(MultiSig::create_proposal_as_identity(
            alice.clone(),
            musig_address.clone(),
            call2.clone(),
            None,
            true
        ));

        let proposal_id1 = MultiSig::proposal_ids(musig_address.clone(), call1).unwrap();
        let proposal_id2 = MultiSig::proposal_ids(musig_address.clone(), call2).unwrap();

        // Proposal with auto close disabled can be voted on even after rejection.
        Context::set_current_identity::<Identity>(Some(bob_did));
        assert_ok!(MultiSig::reject_as_identity(
            bob.clone(),
            musig_address.clone(),
            proposal_id1
        ));
        Context::set_current_identity::<Identity>(Some(charlie_did));
        assert_ok!(MultiSig::reject_as_identity(
            charlie.clone(),
            musig_address.clone(),
            proposal_id1
        ));
        assert_ok!(MultiSig::reject_as_key(
            eve.clone(),
            musig_address.clone(),
            proposal_id1
        ));
        Context::set_current_identity::<Identity>(Some(dave_did));
        assert_ok!(MultiSig::approve_as_identity(
            dave.clone(),
            musig_address.clone(),
            proposal_id1
        ));
        let proposal_details1 = MultiSig::proposal_detail(&(musig_address.clone(), proposal_id1));
        assert_eq!(proposal_details1.approvals, 2);
        assert_eq!(proposal_details1.rejections, 3);
        assert_eq!(
            proposal_details1.status,
            multisig::ProposalStatus::ActiveOrExpired
        );
        assert_eq!(proposal_details1.auto_close, false);

        // Proposal with auto close enabled can not be voted on after rejection.
        Context::set_current_identity::<Identity>(Some(bob_did));
        assert_ok!(MultiSig::reject_as_identity(
            bob.clone(),
            musig_address.clone(),
            proposal_id2
        ));
        Context::set_current_identity::<Identity>(Some(charlie_did));
        assert_ok!(MultiSig::reject_as_identity(
            charlie.clone(),
            musig_address.clone(),
            proposal_id2
        ));
        assert_ok!(MultiSig::reject_as_key(
            eve.clone(),
            musig_address.clone(),
            proposal_id2
        ));
        Context::set_current_identity::<Identity>(Some(dave_did));
        assert_err!(
            MultiSig::approve_as_identity(dave.clone(), musig_address.clone(), proposal_id2),
            Error::ProposalAlreadyRejected
        );

        let proposal_details2 = MultiSig::proposal_detail(&(musig_address.clone(), proposal_id2));
        next_block();
        assert_eq!(proposal_details2.approvals, 1);
        assert_eq!(proposal_details2.rejections, 3);
        assert_eq!(proposal_details2.status, multisig::ProposalStatus::Rejected);
        assert_eq!(proposal_details2.auto_close, true);
    });
}

fn expired_proposals() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let alice = Origin::signed(AccountKeyring::Alice.public());

        let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
        let bob = Origin::signed(AccountKeyring::Bob.public());

        let charlie_did = register_keyring_account(AccountKeyring::Charlie).unwrap();
        let charlie = Origin::signed(AccountKeyring::Charlie.public());

        let musig_address = MultiSig::get_next_multisig_address(AccountKeyring::Alice.public());

        setup_multisig(
            alice.clone(),
            3,
            vec![
                Signatory::from(alice_did),
                Signatory::from(bob_did),
                Signatory::from(charlie_did),
            ],
        );

        let expires_at = 100u64;
        let call = Box::new(Call::MultiSig(multisig::Call::change_sigs_required(2)));

        Context::set_current_identity::<Identity>(Some(alice_did));
        assert_ok!(MultiSig::create_proposal_as_identity(
            alice.clone(),
            musig_address.clone(),
            call.clone(),
            Some(100u64),
            false
        ));

        let proposal_id = MultiSig::proposal_ids(musig_address.clone(), call).unwrap();
        let mut proposal_details = MultiSig::proposal_detail(&(musig_address.clone(), proposal_id));
        assert_eq!(proposal_details.approvals, 1);
        assert_eq!(
            proposal_details.status,
            multisig::ProposalStatus::ActiveOrExpired
        );

        Context::set_current_identity::<Identity>(Some(bob_did));
        assert_ok!(MultiSig::approve_as_identity(
            bob.clone(),
            musig_address.clone(),
            proposal_id
        ));

        proposal_details = MultiSig::proposal_detail(&(musig_address.clone(), proposal_id));
        assert_eq!(proposal_details.approvals, 2);
        assert_eq!(
            proposal_details.status,
            multisig::ProposalStatus::ActiveOrExpired
        );

        // Approval fails when proposal has expired
        Timestamp::set_timestamp(expires_at);
        Context::set_current_identity::<Identity>(Some(charlie_did));
        assert_err!(
            MultiSig::approve_as_identity(charlie.clone(), musig_address.clone(), proposal_id),
            Error::ProposalExpired
        );

        proposal_details = MultiSig::proposal_detail(&(musig_address.clone(), proposal_id));
        assert_eq!(proposal_details.approvals, 2);
        assert_eq!(
            proposal_details.status,
            multisig::ProposalStatus::ActiveOrExpired
        );

        // Approval works when time is expiry - 1
        Timestamp::set_timestamp(expires_at - 1);
        assert_ok!(MultiSig::approve_as_identity(
            charlie.clone(),
            musig_address.clone(),
            proposal_id
        ));

        proposal_details = MultiSig::proposal_detail(&(musig_address.clone(), proposal_id));
        assert_eq!(proposal_details.approvals, 3);
        assert_eq!(
            proposal_details.status,
            multisig::ProposalStatus::ExecutionSuccessful
        );
    });
}

fn setup_multisig(creator_origin: Origin, sigs_required: u64, signers: Vec<Signatory<Public>>) {
    assert_ok!(MultiSig::create_multisig(
        creator_origin,
        signers.clone(),
        sigs_required,
    ));

    for signer in signers {
        let auth_id = get_last_auth_id(&signer);
        assert_ok!(MultiSig::unsafe_accept_multisig_signer(signer, auth_id));
    }
}
