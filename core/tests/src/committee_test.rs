use super::{
    ext_builder::{ExtBuilder, COOL_OFF_PERIOD},
    storage::{
        fast_forward_blocks, get_identity_id, register_keyring_account, root, Call, EventTest,
        TestStorage,
    },
};
use frame_support::{
    assert_err, assert_noop, assert_ok,
    dispatch::{DispatchError, DispatchResult},
};
use frame_system::{EventRecord, Phase};
use pallet_committee::{self as committee, PolymeshVotes, RawEvent as CommitteeRawEvent};
use pallet_group as group;
use pallet_identity as identity;
use pallet_pips::{self as pips, ProposalState, SnapshotResult};
use polymesh_common_utilities::{traits::pip::PipId, MaybeBlock};
use polymesh_primitives::IdentityId;
use sp_core::H256;
use sp_runtime::traits::Hash;
use std::convert::TryFrom;
use test_client::AccountKeyring;

type Committee = committee::Module<TestStorage, committee::Instance1>;
type CommitteeGroup = group::Module<TestStorage, group::Instance1>;
type System = frame_system::Module<TestStorage>;
type Identity = identity::Module<TestStorage>;
type Pips = pips::Module<TestStorage>;
type Origin = <TestStorage as frame_system::Trait>::Origin;

#[test]
fn motions_basic_environment_works() {
    let committee = [AccountKeyring::Alice.public(), AccountKeyring::Bob.public()].to_vec();
    ExtBuilder::default()
        .governance_committee(committee)
        .build()
        .execute_with(motions_basic_environment_works_we);
}

fn motions_basic_environment_works_we() {
    let mut committee = [AccountKeyring::Alice, AccountKeyring::Bob]
        .iter()
        .map(|key| get_identity_id(*key).unwrap())
        .collect::<Vec<_>>();
    committee.sort();

    System::set_block_number(1);
    assert_eq!(Committee::members(), committee);
    assert_eq!(Committee::proposals(), vec![]);
}

fn make_proposal(value: u64) -> Call {
    Call::Identity(identity::Call::accept_primary_key(value, Some(value)))
}

const APPROVE_0: &[(PipId, SnapshotResult)] = &[(0, SnapshotResult::Approve)];

pub fn set_members(ids: Vec<IdentityId>) {
    CommitteeGroup::reset_members(root(), ids).unwrap();
}

fn assert_mem_len(len: u32) {
    assert_ok!(u32::try_from((Committee::members()).len()), len)
}

fn assert_mem(who: IdentityId, is: bool) {
    assert_eq!(Committee::ensure_did_is_member(&who).is_ok(), is);
}

fn abdicate_membership(who: IdentityId, signer: &Origin, n: u32) {
    assert_mem_len(n);
    assert_mem(who, true);
    assert_ok!(CommitteeGroup::abdicate_membership(signer.clone()));
    assert_mem(who, false);
    assert_mem_len(n - 1);
}

fn prepare_proposal(ring: AccountKeyring) {
    let proposal = make_proposal(42);
    let acc = ring.public();
    assert_ok!(Pips::propose(
        Origin::signed(acc),
        Box::new(proposal.clone()),
        50,
        None,
        None,
    ));
    fast_forward_blocks(COOL_OFF_PERIOD);
}

fn check_scheduled(id: PipId) {
    assert_eq!(Pips::proposals(id).unwrap().state, ProposalState::Scheduled);
}

fn enact_snapshot_results_call() -> Call {
    Call::Pips(pallet_pips::Call::enact_snapshot_results(APPROVE_0.into()))
}

fn hash_enact_snapshot_results() -> H256 {
    let call = enact_snapshot_results_call();
    <TestStorage as frame_system::Trait>::Hashing::hash_of(&call)
}

fn vote(who: &Origin, approve: bool) -> DispatchResult {
    Committee::vote_or_propose(
        who.clone(),
        approve,
        Box::new(enact_snapshot_results_call()),
    )
}

#[test]
fn single_member_committee_works() {
    ExtBuilder::default()
        .build()
        .execute_with(single_member_committee_works_we);
}

fn single_member_committee_works_we() {
    System::set_block_number(1);

    let alice_ring = AccountKeyring::Alice;
    let alice_signer = Origin::signed(alice_ring.public());
    let alice_did = register_keyring_account(alice_ring).unwrap();

    set_members(vec![alice_did]);

    // Proposal is executed if committee is comprised of a single member
    prepare_proposal(alice_ring);
    assert_ok!(Pips::snapshot(alice_signer.clone()));
    assert_eq!(Committee::proposals(), vec![]);

    assert_ok!(vote(&alice_signer, true));
    check_scheduled(0);
    let hash = hash_enact_snapshot_results();
    let expected_event = EventRecord {
        phase: Phase::Initialization,
        event: EventTest::committee_Instance1(CommitteeRawEvent::Executed(alice_did, hash, Ok(()))),
        topics: vec![],
    };
    assert_eq!(System::events().contains(&expected_event), true);
}

#[test]
fn preventing_motions_from_non_members_works() {
    ExtBuilder::default()
        .build()
        .execute_with(preventing_motions_from_non_members_works_we);
}

fn preventing_motions_from_non_members_works_we() {
    System::set_block_number(1);

    let alice_ring = AccountKeyring::Alice;
    let alice_signer = Origin::signed(alice_ring.public());
    let _ = register_keyring_account(alice_ring).unwrap();

    prepare_proposal(alice_ring);
    assert_err!(
        Pips::snapshot(alice_signer.clone()),
        pips::Error::<TestStorage>::NotACommitteeMember
    );
    assert_eq!(Committee::proposals(), vec![]);
    assert_noop!(
        vote(&alice_signer, true),
        committee::Error::<TestStorage, committee::Instance1>::NotAMember
    );
}

#[test]
fn preventing_voting_from_non_members_works() {
    ExtBuilder::default()
        .build()
        .execute_with(preventing_voting_from_non_members_works_we);
}

fn preventing_voting_from_non_members_works_we() {
    System::set_block_number(1);

    let alice_ring = AccountKeyring::Alice;
    let alice_signer = Origin::signed(alice_ring.public());
    let alice_did = register_keyring_account(alice_ring).unwrap();
    let bob_signer = Origin::signed(AccountKeyring::Bob.public());
    let _ = register_keyring_account(AccountKeyring::Bob).unwrap();

    set_members(vec![alice_did]);
    prepare_proposal(alice_ring);
    assert_ok!(Pips::snapshot(alice_signer.clone()));
    assert_eq!(Committee::proposals(), vec![]);
    assert_noop!(
        vote(&bob_signer, true),
        committee::Error::<TestStorage, committee::Instance1>::NotAMember
    );
}

#[test]
fn motions_revoting_works() {
    ExtBuilder::default()
        .build()
        .execute_with(motions_revoting_works_we);
}

fn motions_revoting_works_we() {
    System::set_block_number(1);

    let alice_ring = AccountKeyring::Alice;
    let alice_signer = Origin::signed(alice_ring.public());
    let alice_did = register_keyring_account(alice_ring).unwrap();
    let _bob_signer = Origin::signed(AccountKeyring::Bob.public());
    let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
    let _charlie_signer = Origin::signed(AccountKeyring::Charlie.public());
    let charlie_did = register_keyring_account(AccountKeyring::Charlie).unwrap();

    set_members(vec![alice_did, bob_did, charlie_did]);
    prepare_proposal(alice_ring);
    assert_eq!(Committee::proposals(), vec![]);

    assert_ok!(vote(&alice_signer, true));
    let enact_hash = hash_enact_snapshot_results();
    assert_eq!(
        Committee::voting(&enact_hash),
        Some(PolymeshVotes {
            index: 0,
            ayes: vec![alice_did],
            nays: vec![],
            expiry: <_>::default(),
        })
    );
    assert_noop!(
        vote(&alice_signer, true),
        committee::Error::<TestStorage, committee::Instance1>::DuplicateVote
    );
    assert_ok!(vote(&alice_signer, false));
    assert_eq!(
        Committee::voting(&enact_hash),
        Some(PolymeshVotes {
            index: 0,
            ayes: vec![],
            nays: vec![alice_did],
            expiry: <_>::default(),
        })
    );
    assert_noop!(
        vote(&alice_signer, false),
        committee::Error::<TestStorage, committee::Instance1>::DuplicateVote
    );
}

#[test]
fn first_vote_cannot_be_reject() {
    ExtBuilder::default()
        .build()
        .execute_with(first_vote_cannot_be_reject_we);
}

fn first_vote_cannot_be_reject_we() {
    System::set_block_number(1);

    let alice_ring = AccountKeyring::Alice;
    let alice_did = register_keyring_account(alice_ring).unwrap();
    let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
    let charlie_did = register_keyring_account(AccountKeyring::Charlie).unwrap();

    set_members(vec![alice_did, bob_did, charlie_did]);
    prepare_proposal(alice_ring);
    assert_eq!(Committee::proposals(), vec![]);
    assert_noop!(
        vote(&Origin::signed(alice_ring.public()), false),
        committee::Error::<TestStorage, committee::Instance1>::FirstVoteReject
    );
}

#[test]
fn changing_vote_threshold_works() {
    ExtBuilder::default()
        .governance_committee_vote_threshold((1, 1))
        .build()
        .execute_with(changing_vote_threshold_works_we);
}

/// Constructs an origin for the governance council voting majority.
pub fn gc_vmo() -> Origin {
    pallet_committee::Origin::<TestStorage, committee::Instance1>::Endorsed(<_>::default()).into()
}

fn changing_vote_threshold_works_we() {
    let alice_signer = Origin::signed(AccountKeyring::Alice.public());
    let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
    let bob_signer = Origin::signed(AccountKeyring::Bob.public());
    let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
    set_members(vec![alice_did, bob_did]);

    assert_eq!(Committee::vote_threshold(), (1, 1));

    let call_svt = Box::new(Call::Committee(pallet_committee::Call::set_vote_threshold(
        4, 17,
    )));
    assert_ok!(Committee::vote_or_propose(
        alice_signer,
        true,
        call_svt.clone()
    ));
    assert_ok!(Committee::vote_or_propose(
        bob_signer,
        true,
        call_svt.clone()
    ));

    assert_eq!(Committee::vote_threshold(), (4, 17));
}

#[test]
fn rage_quit() {
    ExtBuilder::default()
        .governance_committee_vote_threshold((2, 3))
        .build()
        .execute_with(rage_quit_we);
}

fn rage_quit_we() {
    // 1. Add members to committee
    let alice_ring = AccountKeyring::Alice;
    let alice_signer = Origin::signed(alice_ring.public());
    let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
    let bob_signer = Origin::signed(AccountKeyring::Bob.public());
    let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
    let charlie_signer = Origin::signed(AccountKeyring::Charlie.public());
    let charlie_did = register_keyring_account(AccountKeyring::Charlie).unwrap();
    let dave_did = register_keyring_account(AccountKeyring::Dave).unwrap();
    let ferdie_signer = Origin::signed(AccountKeyring::Ferdie.public());
    let ferdie_did = register_keyring_account(AccountKeyring::Ferdie).unwrap();
    set_members(vec![alice_did, bob_did, charlie_did, dave_did]);
    assert_mem_len(4);

    // Ferdie is NOT a member
    assert_mem(ferdie_did, false);
    assert_err!(
        CommitteeGroup::abdicate_membership(ferdie_signer),
        group::Error::<TestStorage, group::Instance1>::NoSuchMember
    );

    // Make a proposal... only Alice & Bob approve it.
    prepare_proposal(alice_ring);
    assert_ok!(Pips::snapshot(alice_signer.clone()));
    assert_eq!(Committee::proposals(), vec![]);

    assert_ok!(vote(&bob_signer, true));
    assert_ok!(vote(&charlie_signer, false));
    let enact_hash = hash_enact_snapshot_results();
    assert_eq!(
        Committee::voting(&enact_hash),
        Some(PolymeshVotes {
            index: 0,
            ayes: vec![bob_did],
            nays: vec![charlie_did],
            expiry: <_>::default(),
        })
    );

    // Bob quits, its vote should be removed.
    abdicate_membership(bob_did, &bob_signer, 4);
    assert_eq!(
        Committee::voting(&enact_hash),
        Some(PolymeshVotes {
            index: 0,
            ayes: vec![],
            nays: vec![charlie_did],
            expiry: <_>::default(),
        })
    );

    // Charlie quits, its vote should be removed.
    abdicate_membership(charlie_did, &charlie_signer, 3);
    assert_eq!(
        Committee::voting(&enact_hash),
        Some(PolymeshVotes {
            index: 0,
            ayes: vec![],
            nays: vec![],
            expiry: <_>::default(),
        })
    );

    set_members(vec![alice_did, bob_did, charlie_did]);
    assert_ok!(vote(&bob_signer, false));
    assert_err!(
        vote(&bob_signer, false),
        committee::Error::<TestStorage, committee::Instance1>::DuplicateVote
    );
    assert_eq!(
        Committee::voting(&enact_hash),
        Some(PolymeshVotes {
            index: 0,
            ayes: vec![],
            nays: vec![bob_did],
            expiry: <_>::default(),
        })
    );
    assert_ok!(vote(&alice_signer, true));
    assert_eq!(
        Committee::voting(&enact_hash),
        Some(PolymeshVotes {
            index: 0,
            ayes: vec![alice_did],
            nays: vec![bob_did],
            expiry: <_>::default(),
        })
    );

    // Alice should not quit because she is the last member.
    System::reset_events();
    abdicate_membership(charlie_did, &charlie_signer, 3);
    abdicate_membership(bob_did, &bob_signer, 2);
    assert_eq!(Committee::voting(&enact_hash), None);
    assert_mem(alice_did, true);
    assert_err!(
        CommitteeGroup::abdicate_membership(alice_signer),
        group::Error::<TestStorage, group::Instance1>::LastMemberCannotQuit
    );
    assert_mem(alice_did, true);

    // By quitting, only alice remains, so threshold passes, and therefore proposal is executed.
    check_scheduled(0);
    let hash = hash_enact_snapshot_results();
    let did = IdentityId::default();
    let expected_event = EventRecord {
        phase: Phase::Initialization,
        event: EventTest::committee_Instance1(CommitteeRawEvent::Executed(did, hash, Ok(()))),
        topics: vec![],
    };
    assert_eq!(System::events().contains(&expected_event), true);
}

#[test]
fn release_coordinator() {
    let committee = [AccountKeyring::Alice.public(), AccountKeyring::Bob.public()].to_vec();
    ExtBuilder::default()
        .governance_committee(committee)
        .governance_committee_vote_threshold((2, 3))
        .build()
        .execute_with(release_coordinator_we);
}

fn release_coordinator_we() {
    let alice = Origin::signed(AccountKeyring::Alice.public());
    let alice_id = get_identity_id(AccountKeyring::Alice).expect("Alice is part of the committee");
    let bob = Origin::signed(AccountKeyring::Bob.public());
    let bob_id = get_identity_id(AccountKeyring::Bob).expect("Bob is part of the committee");
    let charlie_id = register_keyring_account(AccountKeyring::Charlie).unwrap();

    assert_eq!(
        Committee::release_coordinator(),
        Some(IdentityId::from(999))
    );

    assert_err!(
        Committee::set_release_coordinator(alice.clone(), bob_id),
        DispatchError::BadOrigin
    );

    assert_err!(
        Committee::set_release_coordinator(gc_vmo(), charlie_id),
        committee::Error::<TestStorage, committee::Instance1>::NotAMember
    );

    assert_ok!(Committee::set_release_coordinator(gc_vmo(), bob_id));
    assert_eq!(Committee::release_coordinator(), Some(bob_id));

    // Bob abdicates
    assert_ok!(CommitteeGroup::abdicate_membership(bob));
    assert_eq!(Committee::release_coordinator(), None);

    assert_ok!(Committee::set_release_coordinator(gc_vmo(), alice_id));
    assert_eq!(Committee::release_coordinator(), Some(alice_id));
}

#[test]
fn release_coordinator_majority() {
    let committee = [AccountKeyring::Alice.public(), AccountKeyring::Bob.public()].to_vec();
    ExtBuilder::default()
        .governance_committee(committee)
        .governance_committee_vote_threshold((2, 3))
        .build()
        .execute_with(release_coordinator_majority_we);
}

fn release_coordinator_majority_we() {
    let alice = Origin::signed(AccountKeyring::Alice.public());
    let bob = Origin::signed(AccountKeyring::Bob.public());
    let bob_id = get_identity_id(AccountKeyring::Bob).expect("Bob is part of the committee");

    assert_eq!(
        Committee::release_coordinator(),
        Some(IdentityId::from(999))
    );

    // Vote to change RC => bob.
    let call = Call::Committee(pallet_committee::Call::set_release_coordinator(bob_id));
    assert_ok!(Committee::vote_or_propose(
        alice.clone(),
        true,
        Box::new(call.clone()),
    ));

    // No majority yet.
    assert_eq!(
        Committee::release_coordinator(),
        Some(IdentityId::from(999))
    );

    // Bob votes for themselves, this time *via hash*.
    let hash = <TestStorage as frame_system::Trait>::Hashing::hash_of(&call);
    assert_ok!(Committee::vote(bob, hash, 0, true));

    // Now we have a new RC.
    assert_eq!(Committee::release_coordinator(), Some(bob_id));
}

#[test]
fn enact() {
    let committee = vec![
        AccountKeyring::Alice.public(),
        AccountKeyring::Bob.public(),
        AccountKeyring::Charlie.public(),
    ];
    ExtBuilder::default()
        .governance_committee(committee)
        .build()
        .execute_with(enact_we);
}

fn enact_we() {
    System::set_block_number(1);

    let alice = AccountKeyring::Alice;
    let alice_signer = Origin::signed(alice.public());
    let _ = register_keyring_account(alice);
    let bob = AccountKeyring::Bob.public();
    let _ = register_keyring_account(AccountKeyring::Bob);
    let dave = AccountKeyring::Dave.public();
    let _ = register_keyring_account(AccountKeyring::Dave);

    // 1. Create the PIP.
    prepare_proposal(alice);
    assert_ok!(Pips::snapshot(alice_signer.clone()));
    assert_eq!(Committee::proposals(), vec![]);

    // 2. Alice and Bob vote to enact that pip, they are 2/3 of committee.
    assert_ok!(vote(&alice_signer, true));
    assert_err!(
        vote(&Origin::signed(dave), true),
        committee::Error::<TestStorage, committee::Instance1>::NotAMember,
    );
    assert_ok!(vote(&Origin::signed(bob), true));
    check_scheduled(0);
}

#[test]
fn mesh_1065_regression_test() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);

        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let bob_signer = Origin::signed(AccountKeyring::Bob.public());
        let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
        let charlie_did = register_keyring_account(AccountKeyring::Charlie).unwrap();
        set_members(vec![alice_did, bob_did, charlie_did]);
        assert_mem_len(3);

        let assert_ayes = |ayes| {
            assert_eq!(
                Committee::voting(&hash_enact_snapshot_results()),
                Some(PolymeshVotes {
                    index: 0,
                    ayes,
                    nays: vec![],
                    expiry: <_>::default(),
                })
            );
        };

        // Bob votes for something.
        // It doesn't matter that no PIP exist; we're only testing voting itself.
        assert_ok!(vote(&bob_signer, true));
        assert_ayes(vec![bob_did]);

        // Bob abdicates.
        abdicate_membership(bob_did, &bob_signer, 3);
        assert_ayes(vec![]);

        // Bob rejoins.
        set_members(vec![alice_did, bob_did, charlie_did]);
        assert_mem_len(3);
        assert_ayes(vec![]);

        // Bob revotes, and there's no `DuplicateVote` error.
        assert_ok!(vote(&bob_signer, true));
        assert_ayes(vec![bob_did]);
    });
}

#[test]
fn expiry_works() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(Committee::set_expires_after(gc_vmo(), MaybeBlock::Some(13)));

        let alice_ring = AccountKeyring::Alice;
        let alice_signer = Origin::signed(alice_ring.public());
        let alice_did = register_keyring_account(alice_ring).unwrap();
        let _bob_signer = Origin::signed(AccountKeyring::Bob.public());
        let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
        let _charlie_signer = Origin::signed(AccountKeyring::Charlie.public());
        let charlie_did = register_keyring_account(AccountKeyring::Charlie).unwrap();

        set_members(vec![alice_did, bob_did, charlie_did]);
        prepare_proposal(alice_ring);
        assert_eq!(Committee::proposals(), vec![]);

        assert_ok!(vote(&alice_signer, true));
        assert_eq!(
            Committee::voting(&hash_enact_snapshot_results())
                .unwrap()
                .expiry,
            MaybeBlock::Some(System::block_number() + 13),
        );
        fast_forward_blocks(13 + 1);
        assert_err!(
            vote(&alice_signer, true),
            committee::Error::<TestStorage, committee::Instance1>::ProposalExpired
        );
    });
}
