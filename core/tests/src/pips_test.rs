use super::{
    assert_event_exists,
    committee_test::{gc_vmo, set_members},
    storage::{
        fast_forward_blocks, make_remark_proposal, root, Call, EventTest, TestStorage, User,
    },
    ExtBuilder,
};
use frame_support::{
    assert_noop, assert_ok,
    dispatch::{DispatchError, DispatchResult},
    traits::{LockableCurrency, WithdrawReasons},
    StorageDoubleMap, StorageMap, StorageValue,
};
use frame_system::{self, EventRecord};
use pallet_balances as balances;
use pallet_committee as committee;
use pallet_group as group;
use pallet_pips::{
    self as pips, DepositInfo, LiveQueue, Pip, PipDescription, PipsMetadata, ProposalState,
    Proposer, RawEvent as Event, SnapshotMetadata, SnapshotResult, SnapshottedPip, Url, Vote,
    VotingResult,
};
use pallet_treasury as treasury;
use polymesh_common_utilities::{pip::PipId, MaybeBlock, GC_DID};
use sp_core::sr25519::Public;
use test_client::AccountKeyring;

type System = frame_system::Module<TestStorage>;
type Balances = balances::Module<TestStorage>;
type Pips = pips::Module<TestStorage>;
type Group = group::Module<TestStorage, group::Instance1>;
type Committee = committee::Module<TestStorage, committee::Instance1>;
type Treasury = treasury::Module<TestStorage>;
type Error = pallet_pips::Error<TestStorage>;
type Deposits = pallet_pips::Deposits<TestStorage>;
type Votes = pallet_pips::ProposalVotes<TestStorage>;
type Scheduler = pallet_scheduler::Module<TestStorage>;
type Agenda = pallet_scheduler::Agenda<TestStorage>;

type Origin = <TestStorage as frame_system::Trait>::Origin;

macro_rules! assert_last_event {
    ($event:pat) => {
        assert_last_event!($event, true);
    };
    ($event:pat, $cond:expr) => {
        assert!(matches!(
            &*System::events(),
            [.., EventRecord {
                event: EventTest::pips($event),
                ..
            }]
            if $cond
        ));
    };
}

macro_rules! assert_bad_origin {
    ($e:expr) => {
        assert_noop!($e, DispatchError::BadOrigin);
    };
}

macro_rules! assert_bad_state {
    ($e:expr) => {{
        assert_noop!($e, Error::IncorrectProposalState);
    }};
}

macro_rules! assert_no_pip {
    ($e:expr) => {{
        assert_noop!($e, Error::NoSuchProposal);
    }};
}

fn spip(id: PipId, dir: bool, power: u128) -> SnapshottedPip<u128> {
    SnapshottedPip {
        id,
        weight: (dir, power),
    }
}

fn make_proposal(value: u64) -> Call {
    Call::Pips(pips::Call::set_min_proposal_deposit(value.into()))
}

fn proposal(
    signer: &Origin,
    proposer: &Proposer<Public>,
    proposal: Call,
    deposit: u128,
    url: Option<Url>,
    desc: Option<PipDescription>,
) -> DispatchResult {
    let before = Pips::pip_id_sequence();
    let active = Pips::active_pip_count();
    let signer = signer.clone();
    let result = Pips::propose(signer, Box::new(proposal), deposit, url, desc);
    let add = result.map_or(0, |_| 1);
    if let Ok(_) = result {
        assert_last_event!(Event::ProposalCreated(_, _, id, ..), *id == before);
        assert_eq!(
            Pips::committee_pips().contains(&before),
            matches!(proposer, Proposer::Committee(_))
        );
        assert_eq!(&Pips::proposals(before).unwrap().proposer, proposer);
    }
    assert_eq!(Pips::pip_id_sequence(), before + add);
    assert_eq!(Pips::active_pip_count(), active + add);
    result
}

fn standard_proposal(
    signer: &Origin,
    proposer: &Proposer<Public>,
    deposit: u128,
) -> DispatchResult {
    proposal(signer, proposer, make_proposal(42), deposit, None, None)
}

fn remark_proposal(signer: &Origin, proposer: &Proposer<Public>, deposit: u128) -> DispatchResult {
    proposal(
        signer,
        proposer,
        make_remark_proposal(),
        deposit,
        None,
        None,
    )
}

const THE_COMMITTEE: Proposer<Public> = Proposer::Committee(pallet_pips::Committee::Upgrade);

fn committee_proposal(deposit: u128) -> DispatchResult {
    standard_proposal(
        &pallet_committee::Origin::<TestStorage, committee::Instance4>::Endorsed(<_>::default())
            .into(),
        &THE_COMMITTEE,
        deposit,
    )
}

fn alice_proposal(deposit: u128) -> DispatchResult {
    let acc = AccountKeyring::Alice.public();
    standard_proposal(&Origin::signed(acc), &Proposer::Community(acc), deposit)
}

fn alice_remark_proposal(deposit: u128) -> DispatchResult {
    let acc = AccountKeyring::Alice.public();
    remark_proposal(&Origin::signed(acc), &Proposer::Community(acc), deposit)
}

fn consensus_call(call: pallet_pips::Call<TestStorage>, signers: &[&Origin]) {
    let call = Box::new(Call::Pips(call));
    for signer in signers.iter().copied().cloned() {
        assert_ok!(Committee::vote_or_propose(signer, true, call.clone()));
    }
}

fn assert_state(id: PipId, care_about_pruned: bool, state: ProposalState) {
    let prop = Pips::proposals(id);
    if care_about_pruned && Pips::prune_historical_pips() {
        assert_eq!(prop, None);
    } else {
        assert_eq!(prop.unwrap().state, state);
    }
}

pub fn assert_balance(acc: Public, free: u128, locked: u128) {
    assert_eq!(Balances::free_balance(&acc), free);
    assert_eq!(Balances::usable_balance(&acc), free - locked);
}

#[test]
fn updating_pips_variables_works() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);

        assert_eq!(Pips::prune_historical_pips(), false);
        assert_ok!(Pips::set_prune_historical_pips(root(), true));
        assert_last_event!(Event::HistoricalPipsPruned(_, false, true));
        assert_eq!(Pips::prune_historical_pips(), true);

        assert_eq!(Pips::min_proposal_deposit(), 50);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 10));
        assert_last_event!(Event::MinimumProposalDepositChanged(_, 50, 10));
        assert_eq!(Pips::min_proposal_deposit(), 10);

        assert_eq!(Pips::default_enactment_period(), 100);
        assert_ok!(Pips::set_default_enactment_period(root(), 10));
        assert_last_event!(Event::DefaultEnactmentPeriodChanged(_, 100, 10));
        assert_eq!(Pips::default_enactment_period(), 10);

        assert_eq!(Pips::pending_pip_expiry(), MaybeBlock::None);
        assert_ok!(Pips::set_pending_pip_expiry(root(), MaybeBlock::Some(13)));
        assert_last_event!(Event::PendingPipExpiryChanged(
            _,
            MaybeBlock::None,
            MaybeBlock::Some(13)
        ));
        assert_eq!(Pips::pending_pip_expiry(), MaybeBlock::Some(13));

        assert_eq!(Pips::max_pip_skip_count(), 1);
        assert_ok!(Pips::set_max_pip_skip_count(root(), 42));
        assert_last_event!(Event::MaxPipSkipCountChanged(_, 1, 42));
        assert_eq!(Pips::max_pip_skip_count(), 42);

        assert_eq!(Pips::active_pip_limit(), 5);
        assert_ok!(Pips::set_active_pip_limit(root(), 42));
        assert_last_event!(Event::ActivePipLimitChanged(_, 5, 42));
        assert_eq!(Pips::active_pip_limit(), 42);
    });
}

#[test]
fn updating_pips_variables_only_root() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        let signer = Origin::signed(AccountKeyring::Alice.public());
        System::reset_events();

        assert_noop!(
            Pips::set_prune_historical_pips(signer.clone(), false),
            DispatchError::BadOrigin,
        );
        assert_noop!(
            Pips::set_min_proposal_deposit(signer.clone(), 0),
            DispatchError::BadOrigin,
        );
        assert_noop!(
            Pips::set_default_enactment_period(signer.clone(), 0),
            DispatchError::BadOrigin,
        );
        assert_noop!(
            Pips::set_max_pip_skip_count(signer.clone(), 0),
            DispatchError::BadOrigin,
        );
        assert_noop!(
            Pips::set_active_pip_limit(signer.clone(), 0),
            DispatchError::BadOrigin,
        );

        assert_eq!(System::events(), vec![])
    });
}

#[test]
fn historical_prune_works() {
    ExtBuilder::default().monied(true).build().execute_with(|| {
        // We just test one case for brevity.
        System::set_block_number(1);
        assert_ok!(Pips::set_prune_historical_pips(root(), true));
        assert_pruned(rejected_proposal());
        let bob = User::new(AccountKeyring::Bob);
        set_members(vec![bob.did]);
        assert_pruned(executed_community_proposal(&bob.origin()));
        assert_pruned(failed_community_proposal(bob, 1337));
        assert_pruned(expired_proposal(13));
    });
}

#[test]
fn min_deposit_works() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        let deposit = 40;
        assert_ok!(Pips::set_min_proposal_deposit(root(), deposit + 1));

        let alice = User::new(AccountKeyring::Alice).balance(300);

        // Error when min deposit requirements are not met.
        assert_eq!(Pips::pip_id_sequence(), 0);
        assert_eq!(Pips::active_pip_count(), 0);
        assert_noop!(alice_proposal(deposit), Error::IncorrectDeposit);

        // Now let's use enough.
        assert_ok!(alice_proposal(deposit + 1));
        assert_state(0, false, ProposalState::Pending);
        assert_eq!(
            Pips::proposals(0).unwrap().proposer,
            Proposer::Community(alice.acc())
        );

        // Committees are exempt from min deposit.
        assert_ok!(committee_proposal(0));
        assert_state(1, false, ProposalState::Pending);
        assert_eq!(Pips::proposals(1).unwrap().proposer, THE_COMMITTEE);
        assert_vote_details(1, VotingResult::default(), vec![], vec![]);
    })
}

#[test]
fn active_limit_works() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));

        assert_eq!(Pips::pip_id_sequence(), 0);
        assert_eq!(Pips::active_pip_count(), 0);

        assert_ok!(alice_proposal(0));
        assert_eq!(Pips::active_pip_count(), 1);

        // Limit reached, so error.
        assert_ok!(Pips::set_active_pip_limit(root(), 1));
        assert_noop!(alice_proposal(0), Error::TooManyActivePips);
        assert_eq!(Pips::active_pip_count(), 1);

        // Bump limit; ok again.
        assert_ok!(Pips::set_active_pip_limit(root(), 2));
        assert_ok!(alice_proposal(0));
        assert_eq!(Pips::active_pip_count(), 2);

        // Reached again, so error.
        assert_noop!(alice_proposal(0), Error::TooManyActivePips);
        assert_eq!(Pips::active_pip_count(), 2);

        // Committees are exempt from limit.
        assert_ok!(committee_proposal(0));
        assert_eq!(Pips::active_pip_count(), 3);

        // Remove limit completely, and let's add more.
        assert_ok!(Pips::set_active_pip_limit(root(), 0));
        assert_ok!(alice_proposal(0));
        assert_eq!(Pips::active_pip_count(), 4);
    })
}

#[test]
fn default_enactment_period_works_community() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);

        let alice = User::new(AccountKeyring::Alice).balance(300);
        set_members(vec![alice.did]);

        let check_community = |period| {
            assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
            assert_ok!(alice_proposal(0));
            let last_id = Pips::pip_id_sequence() - 1;
            fast_forward_blocks(1);
            assert_ok!(Pips::snapshot(alice.origin()));
            assert_ok!(Pips::set_default_enactment_period(root(), period));
            let block_at_approval = System::block_number();
            assert_ok!(Pips::enact_snapshot_results(
                gc_vmo(),
                vec![(last_id, SnapshotResult::Approve)]
            ));
            let expected = Pips::pip_to_schedule(last_id).unwrap();
            let period = period.max(1);
            assert_eq!(expected, block_at_approval + period);
            assert_eq!(1, Agenda::get(expected).len());
        };
        check_community(0);
        check_community(3);
        check_community(42);
        check_community(1337);
    });
}

#[test]
fn default_enactment_period_works_committee() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);

        let alice = User::new(AccountKeyring::Alice).balance(300);
        set_members(vec![alice.did]);

        let check_committee = |period| {
            assert_ok!(committee_proposal(0));
            let last_id = Pips::pip_id_sequence() - 1;
            fast_forward_blocks(1);
            assert_ok!(Pips::set_default_enactment_period(root(), period));
            let block_at_approval = System::block_number();
            assert_ok!(Pips::approve_committee_proposal(gc_vmo(), last_id));
            let expected = Pips::pip_to_schedule(last_id).unwrap();
            let period = period.max(1);
            assert_eq!(expected, block_at_approval + period);
            assert_eq!(1, Agenda::get(expected).len());
        };
        check_committee(0);
        check_committee(3);
        check_committee(42);
        check_committee(1337);
    })
}

#[test]
fn skip_limit_works() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));

        let alice = User::new(AccountKeyring::Alice).balance(300);
        set_members(vec![alice.did]);

        let snap = || Pips::snapshot(alice.origin()).unwrap();
        let count = |n| Pips::set_max_pip_skip_count(root(), n).unwrap();
        let commit = || Pips::enact_snapshot_results(gc_vmo(), vec![(0, SnapshotResult::Skip)]);

        assert_ok!(alice_proposal(0));

        snap();
        count(0);
        assert_noop!(commit(), Error::CannotSkipPip);

        snap();
        count(1);
        assert_ok!(commit());
        snap();
        assert_noop!(commit(), Error::CannotSkipPip);

        snap();
        count(2);
        assert_ok!(commit());
        snap();
        assert_noop!(commit(), Error::CannotSkipPip);
    });
}

fn assert_vote_details(
    id: PipId,
    results: VotingResult<u128>,
    deposits: Vec<DepositInfo<Public, u128>>,
    votes: Vec<Vote<u128>>,
) {
    assert_eq!(results, Pips::proposal_result(id));
    assert_eq!(
        deposits,
        Deposits::iter_prefix_values(id).collect::<Vec<_>>(),
    );
    assert_eq!(votes, Votes::iter_prefix_values(id).collect::<Vec<_>>());
}

fn assert_votes(id: PipId, owner: Public, amount: u128) {
    assert_vote_details(
        id,
        VotingResult {
            ayes_count: 1,
            ayes_stake: amount,
            nays_count: 0,
            nays_stake: 0,
        },
        vec![DepositInfo { owner, amount }],
        vec![Vote(true, amount)],
    );
}

#[test]
fn proposal_details_are_correct() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(42);

        let alice = User::new(AccountKeyring::Alice).balance(300);

        let call = make_remark_proposal();
        let proposal_url: Url = b"www.abc.com".into();
        let proposal_desc: PipDescription = b"Test description".into();

        let proposer = Proposer::Community(alice.acc());

        // Alice starts a proposal with min deposit.
        assert_ok!(proposal(
            &alice.origin(),
            &proposer,
            call.clone(),
            60,
            Some(proposal_url.clone()),
            Some(proposal_desc.clone()),
        ));
        assert_last_event!(Event::ProposalCreated(..));

        let expected = Pip {
            id: 0,
            proposal: call,
            state: ProposalState::Pending,
            proposer,
        };
        assert_eq!(Pips::proposals(0).unwrap(), expected);

        let expected = PipsMetadata {
            id: 0,
            created_at: 42,
            url: Some(proposal_url),
            description: Some(proposal_desc),
            transaction_version: 0,
            expiry: <_>::default(),
        };
        assert_eq!(Pips::proposal_metadata(0).unwrap(), expected);

        assert_balance(alice.acc(), 300, 60);
        assert_votes(0, alice.acc(), 60);
    });
}

#[test]
fn propose_committee_pip_only_zero_deposit() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(committee_proposal(0));
        assert_noop!(committee_proposal(1337), Error::NotFromCommunity);
    });
}

#[test]
fn vote_no_such_proposal() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        let signer = Origin::signed(AccountKeyring::Bob.public());
        assert_no_pip!(Pips::vote(signer, 0, false, 0));
    });
}

#[test]
fn vote_not_pending() {
    let op_and_check = |op_and_check: &dyn Fn(Origin, PipId)| {
        ExtBuilder::default().build().execute_with(|| {
            System::set_block_number(1);
            assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
            assert_ok!(Pips::set_prune_historical_pips(root(), false));

            let alice = User::new(AccountKeyring::Alice);
            set_members(vec![alice.did]);

            op_and_check(alice.origin(), rejected_proposal());
            op_and_check(alice.origin(), executed_community_proposal(&alice.origin()));
            op_and_check(alice.origin(), failed_community_proposal(alice, 1337));
            op_and_check(alice.origin(), scheduled_proposal(&alice.origin(), 0));
            op_and_check(alice.origin(), expired_proposal(24));
        })
    };
    op_and_check(&|o, id| assert_bad_state!(Pips::vote(o, id, false, 0)));
}

#[test]
fn vote_bond_additional_deposit_works() {
    ExtBuilder::default().monied(true).build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));

        let init_free = 1000;
        let init_amount = 300;
        let then_amount = 137;
        let amount = init_amount + then_amount;

        let acc = AccountKeyring::Alice.public();
        let signer = Origin::signed(acc);
        assert_balance(acc, init_free, 0);

        assert_ok!(alice_proposal(init_amount));
        assert_balance(acc, init_free, init_amount);
        assert_ok!(Pips::vote(signer, 0, true, amount));
        assert_balance(acc, init_free, amount);
        assert_last_event!(Event::Voted(.., true, _));
        assert_votes(0, acc, amount);
    });
}

#[test]
fn vote_owner_below_min_deposit() {
    ExtBuilder::default().monied(true).build().execute_with(|| {
        System::set_block_number(1);
        let min = 50;
        let sub = min - 1;
        assert_ok!(Pips::set_min_proposal_deposit(root(), min));

        let alice = User::new(AccountKeyring::Alice);
        let bob = User::new(AccountKeyring::Bob);

        assert_ok!(alice_proposal(100));
        assert_noop!(
            Pips::vote(alice.origin(), 0, true, sub),
            Error::IncorrectDeposit
        );
        assert_votes(0, alice.acc(), 100);
        // Doesn't apply to Bob though, as they didn't propose it.
        assert_ok!(Pips::vote(bob.origin(), 0, true, sub));
        assert_vote_details(
            0,
            VotingResult {
                ayes_count: 2,
                ayes_stake: 100 + sub,
                ..VotingResult::default()
            },
            vec![
                DepositInfo {
                    owner: alice.acc(),
                    amount: 100,
                },
                DepositInfo {
                    owner: bob.acc(),
                    amount: sub,
                },
            ],
            vec![Vote(true, 100), Vote(true, sub)],
        );
    });
}

#[test]
fn vote_unbond_deposit_works() {
    ExtBuilder::default().monied(true).build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));

        let init_free = 1000;
        let init_amount = 200;
        let then_amount = 100;

        let acc = AccountKeyring::Alice.public();
        let signer = Origin::signed(acc);
        assert_eq!(Balances::free_balance(&acc), init_free);

        assert_ok!(alice_proposal(init_amount));
        assert_balance(acc, init_free, init_amount);
        assert_ok!(Pips::vote(signer, 0, true, then_amount));
        assert_balance(acc, init_free, then_amount);
        assert_last_event!(Event::Voted(.., true, _));
        assert_votes(0, acc, then_amount);
    });
}

#[test]
fn vote_on_community_only() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(committee_proposal(0));
        let signer = Origin::signed(AccountKeyring::Alice.public());
        assert_noop!(Pips::vote(signer, 0, false, 0), Error::NotFromCommunity);
    });
}

#[test]
fn vote_duplicate_ok() {
    ExtBuilder::default().monied(true).build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        let signer = Origin::signed(AccountKeyring::Alice.public());

        assert_ok!(alice_proposal(42));
        assert_eq!(
            Pips::proposal_result(0),
            VotingResult {
                ayes_count: 1,
                ayes_stake: 42,
                ..VotingResult::default()
            }
        );
        assert_ok!(Pips::vote(signer.clone(), 0, true, 21));
        assert_eq!(
            Pips::proposal_result(0),
            VotingResult {
                ayes_count: 1,
                ayes_stake: 21,
                ..VotingResult::default()
            }
        );
        assert_ok!(Pips::vote(signer.clone(), 0, false, 21));
        assert_eq!(
            Pips::proposal_result(0),
            VotingResult {
                nays_count: 1,
                nays_stake: 21,
                ..VotingResult::default()
            }
        );
        assert_ok!(Pips::vote(signer.clone(), 0, false, 42));
        assert_eq!(
            Pips::proposal_result(0),
            VotingResult {
                nays_count: 1,
                nays_stake: 42,
                ..VotingResult::default()
            }
        );
        assert_ok!(Pips::vote(signer.clone(), 0, true, 42));
        assert_eq!(
            Pips::proposal_result(0),
            VotingResult {
                ayes_count: 1,
                ayes_stake: 42,
                ..VotingResult::default()
            }
        );
    });
}

#[test]
fn vote_stake_overflow() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);

        let alice = User::new(AccountKeyring::Alice).balance(u128::MAX);
        let bob = User::new(AccountKeyring::Bob).balance(100);

        assert_ok!(alice_proposal(u128::MAX));
        assert_eq!(
            Pips::proposal_result(0),
            VotingResult {
                ayes_count: 1,
                ayes_stake: u128::MAX,
                ..VotingResult::default()
            }
        );
        assert_noop!(
            Pips::vote(bob.origin(), 0, true, 1),
            Error::StakeAmountOfVotesExceeded,
        );
        assert_ok!(Pips::vote(bob.origin(), 0, false, 1));
        assert_noop!(
            Pips::vote(alice.origin(), 0, false, u128::MAX),
            Error::StakeAmountOfVotesExceeded,
        );
    });
}

#[test]
fn vote_insufficient_reserve() {
    ExtBuilder::default()
        .monied(false)
        .build()
        .execute_with(|| {
            System::set_block_number(1);
            assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
            let signer = Origin::signed(AccountKeyring::Bob.public());
            assert_ok!(alice_proposal(0));
            assert_noop!(
                Pips::vote(signer.clone(), 0, false, 50),
                Error::InsufficientDeposit
            );
            assert_noop!(Pips::vote(signer, 0, true, 1), Error::InsufficientDeposit);
        });
}

#[test]
fn vote_works() {
    ExtBuilder::default().monied(true).build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        let alice_acc = AccountKeyring::Alice.public();
        let bob_acc = AccountKeyring::Bob.public();
        let bob = Origin::signed(bob_acc);
        let charlie_acc = AccountKeyring::Charlie.public();
        let charlie = Origin::signed(charlie_acc);
        assert_ok!(alice_proposal(100));
        assert_balance(bob_acc, 2000, 0);
        assert_balance(charlie_acc, 3000, 0);
        assert_ok!(Pips::vote(bob, 0, false, 1337));
        assert_last_event!(Event::Voted(.., false, 1337));
        assert_ok!(Pips::vote(charlie, 0, true, 2441));
        assert_last_event!(Event::Voted(.., true, 2441));
        assert_balance(bob_acc, 2000, 1337);
        assert_balance(charlie_acc, 3000, 2441);
        assert_vote_details(
            0,
            VotingResult {
                ayes_count: 2,
                ayes_stake: 2541,
                nays_count: 1,
                nays_stake: 1337,
            },
            vec![
                DepositInfo {
                    owner: alice_acc,
                    amount: 100,
                },
                DepositInfo {
                    owner: bob_acc,
                    amount: 1337,
                },
                DepositInfo {
                    owner: charlie_acc,
                    amount: 2441,
                },
            ],
            vec![Vote(true, 100), Vote(false, 1337), Vote(true, 2441)],
        );
    });
}

#[test]
fn voting_for_pip_uses_stack_over_overlay() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        // Initialize with 100 POLYX.
        let alice = User::new(AccountKeyring::Alice).balance(100);
        // Lock all but 10.
        Balances::set_lock(*b"deadbeef", &alice.acc(), 90, WithdrawReasons::all());
        assert_balance(alice.acc(), 100, 90);
        // OK, because we're overlaying with 90 tokens already locked.
        assert_ok!(alice_proposal(50));
        assert_balance(alice.acc(), 100, 90);
        // OK, because we're still overlaying, but also increasing it by 10.
        assert_ok!(alice_proposal(50));
        assert_balance(alice.acc(), 100, 100);
        // Error, because we don't have 101 tokens to bond.
        assert_noop!(alice_proposal(1), Error::InsufficientDeposit);
    });
}

#[test]
fn approve_committee_proposal_not_pending() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        assert_ok!(Pips::set_prune_historical_pips(root(), false));

        let alice = User::new(AccountKeyring::Alice);
        set_members(vec![alice.did]);

        let acp_bad_state = |id| assert_bad_state!(Pips::approve_committee_proposal(gc_vmo(), id));
        acp_bad_state(rejected_proposal());
        acp_bad_state(executed_community_proposal(&alice.origin()));
        acp_bad_state(failed_community_proposal(alice, 1337));
        acp_bad_state(scheduled_proposal(&alice.origin(), 0));
        acp_bad_state(expired_proposal(9));
    });
}

#[test]
fn approve_committee_proposal_no_such_proposal() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_no_pip!(Pips::approve_committee_proposal(gc_vmo(), 0));
    });
}

#[test]
fn approve_committee_proposal_not_by_committee() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        assert_ok!(alice_proposal(0));
        assert_noop!(
            Pips::approve_committee_proposal(gc_vmo(), 0),
            Error::NotByCommittee,
        );
    });
}

#[test]
fn only_gc_majority_stuff() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));

        let alice = User::new(AccountKeyring::Alice);
        let bob = User::new(AccountKeyring::Bob);
        let charlie = User::new(AccountKeyring::Charlie);
        set_members(vec![bob.did, charlie.did]);

        // Make a proposal
        let id = Pips::pip_id_sequence();
        assert_eq!(id, 0);
        assert_eq!(Pips::active_pip_count(), 0);
        assert_ok!(alice_proposal(0));
        // Alice not part of GC and cannot reject.
        assert_bad_origin!(Pips::reject_proposal(alice.origin(), id));
        // Bob & Charlie but need to seek consensus.
        assert_bad_origin!(Pips::reject_proposal(bob.origin(), id));
        assert_bad_origin!(Pips::reject_proposal(charlie.origin(), id));
        // Ditto for pruning.
        assert_bad_origin!(Pips::prune_proposal(alice.origin(), id));
        // Bob & Charlie but need to seek consensus.
        assert_bad_origin!(Pips::prune_proposal(bob.origin(), id));
        assert_bad_origin!(Pips::prune_proposal(charlie.origin(), id));
        // Ditto for `approve_committee_proposal`.
        assert_bad_origin!(Pips::approve_committee_proposal(alice.origin(), id));
        // Bob & Charlie but need to seek consensus.
        assert_bad_origin!(Pips::approve_committee_proposal(bob.origin(), id));
        assert_bad_origin!(Pips::approve_committee_proposal(charlie.origin(), id));
        // Ditto for `enact_snapshot_result`.
        assert_bad_origin!(Pips::enact_snapshot_results(alice.origin(), vec![]));
        // Bob & Charlie but need to seek consensus.
        assert_bad_origin!(Pips::enact_snapshot_results(bob.origin(), vec![]));
        assert_bad_origin!(Pips::enact_snapshot_results(charlie.origin(), vec![]));

        // VMO can reject.
        assert_ok!(Pips::set_prune_historical_pips(root(), false));
        assert_ok!(Pips::reject_proposal(gc_vmo(), id));
        assert_eq!(Pips::pip_id_sequence(), 1);
        assert_eq!(Pips::active_pip_count(), 0);
        assert_state(id, false, ProposalState::Rejected);
        // VMO can also prune.
        assert_ok!(Pips::prune_proposal(gc_vmo(), id));
        assert_eq!(Pips::proposals(id), None);
        // VMO can also `approve_committee_proposal`.
        let id = Pips::pip_id_sequence();
        assert_ok!(committee_proposal(0));
        assert_ok!(Pips::approve_committee_proposal(gc_vmo(), id));
        assert_ok!(Pips::reject_proposal(gc_vmo(), id));
        assert_ok!(Pips::prune_proposal(gc_vmo(), id));
        // VMO can also `enact_snapshot_results`.
        let id = Pips::pip_id_sequence();
        assert_ok!(alice_proposal(0));
        assert_ok!(Pips::snapshot(bob.origin()));
        assert_ok!(Pips::enact_snapshot_results(
            gc_vmo(),
            vec![(id, SnapshotResult::Reject)]
        ));

        let consensus_call = |call| consensus_call(call, &[&bob.origin(), &charlie.origin()]);

        // Bob & Charlie seek consensus and successfully reject.
        let id = Pips::pip_id_sequence();
        assert_ok!(alice_proposal(0));
        assert_eq!(Pips::pip_id_sequence(), id + 1);
        assert_eq!(Pips::active_pip_count(), 1);
        consensus_call(pallet_pips::Call::reject_proposal(id));
        assert_eq!(Pips::pip_id_sequence(), id + 1);
        assert_eq!(Pips::active_pip_count(), 0);
        assert_state(id, false, ProposalState::Rejected);
        // And now they seek consensus to and do prune.
        consensus_call(pallet_pips::Call::prune_proposal(id));
        assert_eq!(Pips::proposals(id), None);

        // Bob & Charlie seek consensus.
        // They successfully do `approve_committee_proposal` & `enact_snapshot_results`.
        let id_committee = Pips::pip_id_sequence();
        assert_ok!(committee_proposal(0));
        let id_snapshot = Pips::pip_id_sequence();
        assert_ok!(alice_proposal(0));
        assert_ok!(Pips::snapshot(bob.origin()));
        consensus_call(pallet_pips::Call::approve_committee_proposal(id_committee));
        consensus_call(pallet_pips::Call::enact_snapshot_results(vec![(
            id_snapshot,
            SnapshotResult::Approve,
        )]));
        assert_state(id_committee, false, ProposalState::Scheduled);
        assert_state(id_snapshot, false, ProposalState::Scheduled);
    });
}

#[test]
fn cannot_reject_no_such_proposal() {
    ExtBuilder::default().build().execute_with(|| {
        // Rejecting PIP that doesn't exist errors.
        assert_eq!(Pips::pip_id_sequence(), 0);
        assert_eq!(Pips::active_pip_count(), 0);
        assert_no_pip!(Pips::reject_proposal(gc_vmo(), 0));
        assert_eq!(Pips::pip_id_sequence(), 0);
        assert_eq!(Pips::active_pip_count(), 0);
        assert_no_pip!(Pips::prune_proposal(gc_vmo(), 0));
        assert_eq!(Pips::pip_id_sequence(), 0);
        assert_eq!(Pips::active_pip_count(), 0);
    });
}

fn scheduled_proposal(signer: &Origin, deposit: u128) -> PipId {
    let next_id = Pips::pip_id_sequence();
    assert_ok!(alice_proposal(deposit));
    let active = Pips::active_pip_count();
    assert_ok!(Pips::snapshot(signer.clone()));
    assert_ok!(Pips::enact_snapshot_results(
        gc_vmo(),
        vec![(next_id, SnapshotResult::Approve)]
    ));
    assert_event_exists!(
        EventTest::pallet_scheduler(pallet_scheduler::RawEvent::Scheduled(b, ..)),
        *b == System::block_number() + Pips::default_enactment_period()
    );
    assert_state(next_id, false, ProposalState::Scheduled);
    assert_eq!(Pips::active_pip_count(), active);
    next_id
}

fn executed_community_proposal(signer: &Origin) -> PipId {
    let deposit = Pips::min_proposal_deposit();
    let next_id = scheduled_proposal(signer, deposit);
    let active = Pips::active_pip_count();
    fast_forward_blocks(Pips::default_enactment_period() + 1);
    assert_ok!(Pips::set_min_proposal_deposit(root(), deposit));
    assert_state(next_id, true, ProposalState::Executed);
    assert_eq!(Pips::active_pip_count(), active - 1);
    next_id
}

fn failed_community_proposal(user: User, bad_id: PipId) -> PipId {
    let next_id = Pips::pip_id_sequence();
    let deposit = Pips::min_proposal_deposit();
    assert_ok!(proposal(
        &user.origin(),
        &Proposer::Community(user.acc()),
        Call::Pips(pallet_pips::Call::reject_proposal(bad_id)),
        deposit,
        None,
        None
    ));
    let active = Pips::active_pip_count();
    assert_ok!(Pips::snapshot(user.origin()));
    assert_ok!(Pips::enact_snapshot_results(
        gc_vmo(),
        vec![(next_id, SnapshotResult::Approve)]
    ));
    assert_state(next_id, false, ProposalState::Scheduled);
    assert_eq!(Pips::active_pip_count(), active);
    fast_forward_blocks(Pips::default_enactment_period() + 1);
    assert_state(next_id, true, ProposalState::Failed);
    assert_eq!(Pips::active_pip_count(), active - 1);
    next_id
}

fn rejected_proposal() -> PipId {
    let next_id = Pips::pip_id_sequence();
    assert_ok!(alice_proposal(Pips::min_proposal_deposit()));
    let active = Pips::active_pip_count();
    assert_ok!(Pips::reject_proposal(gc_vmo(), next_id));
    assert_state(next_id, true, ProposalState::Rejected);
    assert_eq!(Pips::active_pip_count(), active - 1);
    assert_eq!(Pips::pip_id_sequence(), next_id + 1);
    next_id
}

fn expired_proposal(expiry: u64) -> PipId {
    let next_id = Pips::pip_id_sequence();

    // Save old config data and set new ones for expiry.
    let old_expiry = Pips::pending_pip_expiry();
    assert_ok!(Pips::set_pending_pip_expiry(
        root(),
        MaybeBlock::Some(expiry)
    ));

    // Create a proposal and verify its pending.
    let active = Pips::active_pip_count();
    assert_ok!(alice_proposal(Pips::min_proposal_deposit()));
    assert_state(next_id, false, ProposalState::Pending);
    assert_eq!(
        Pips::proposal_metadata(next_id).unwrap().expiry,
        MaybeBlock::Some(expiry + System::block_number())
    );

    // Now fast forward.
    fast_forward_blocks(expiry + 1); // Forward exactly to expiry point + 1.
    assert_eq!(Pips::active_pip_count(), active);

    // Restore config to before function was called.
    assert_ok!(Pips::set_pending_pip_expiry(root(), old_expiry));

    next_id
}

#[test]
fn cannot_reject_incorrect_state() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        assert_ok!(Pips::set_prune_historical_pips(root(), false));

        let bob = User::new(AccountKeyring::Bob);
        set_members(vec![bob.did]);

        let reject_bad_state = |id| assert_bad_state!(Pips::reject_proposal(gc_vmo(), id));
        // Cannot reject executed, failed, and rejected:
        let exec_id = executed_community_proposal(&bob.origin());
        reject_bad_state(exec_id);
        reject_bad_state(failed_community_proposal(bob, exec_id));
        reject_bad_state(rejected_proposal());
        reject_bad_state(expired_proposal(23));
    });
}

fn assert_pruned(id: PipId) {
    assert_eq!(Pips::proposal_metadata(id), None);
    assert_eq!(Deposits::iter_prefix_values(id).count(), 0);
    assert_eq!(Pips::proposals(id), None);
    assert_vote_details(id, VotingResult::default(), vec![], vec![]);
    assert_eq!(Pips::pip_to_schedule(id), None);
    // TODO: Check that the PIP has been removed from the schedule. This should be easily done after
    // fixing this issue: https://github.com/paritytech/substrate/issues/7449
    assert!(Pips::snapshot_queue().iter().all(|p| p.id != id));
    assert_eq!(Pips::pip_skip_count(id), 0);
}

#[test]
fn can_prune_states_that_cannot_be_rejected() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_prune_historical_pips(root(), false));

        let init_bal = 1000;
        let alice = User::new(AccountKeyring::Alice).balance(init_bal);
        let bob = User::new(AccountKeyring::Bob);
        set_members(vec![bob.did]);

        // Can prune executed:
        assert_balance(alice.acc(), init_bal, 0);
        assert_eq!(Balances::free_balance(&alice.acc()), init_bal);
        assert_ok!(alice_proposal(200));
        assert_balance(alice.acc(), init_bal, 200);
        assert_eq!(Pips::pip_id_sequence(), 1);
        assert_eq!(Pips::active_pip_count(), 1);
        assert_ok!(Pips::snapshot(bob.origin()));
        assert_ok!(Pips::enact_snapshot_results(
            gc_vmo(),
            vec![(0, SnapshotResult::Approve)]
        ));
        assert_state(0, false, ProposalState::Scheduled);
        assert_balance(alice.acc(), init_bal, 200);
        assert_eq!(Pips::active_pip_count(), 1);
        fast_forward_blocks(Pips::default_enactment_period() + 1);
        assert_state(0, false, ProposalState::Executed);
        assert_balance(alice.acc(), init_bal, 0);
        assert_eq!(Pips::active_pip_count(), 0);
        assert_ok!(Pips::prune_proposal(gc_vmo(), 0));
        assert_balance(alice.acc(), init_bal, 0);
        assert_pruned(0);

        // Can prune failed:
        assert_ok!(proposal(
            &alice.origin(),
            &Proposer::Community(alice.acc()),
            Call::Pips(pallet_pips::Call::reject_proposal(1337)),
            300,
            None,
            None
        ));
        assert_balance(alice.acc(), init_bal, 300);
        assert_eq!(Pips::pip_id_sequence(), 2);
        assert_eq!(Pips::active_pip_count(), 1);
        assert_ok!(Pips::snapshot(bob.origin()));
        assert_ok!(Pips::enact_snapshot_results(
            gc_vmo(),
            vec![(1, SnapshotResult::Approve)]
        ));
        assert_state(1, false, ProposalState::Scheduled);
        assert_balance(alice.acc(), init_bal, 300);
        assert_eq!(Pips::active_pip_count(), 1);
        fast_forward_blocks(Pips::default_enactment_period() + 1);
        assert_state(1, false, ProposalState::Failed);
        assert_balance(alice.acc(), init_bal, 0);
        assert_eq!(Pips::active_pip_count(), 0);
        assert_ok!(Pips::prune_proposal(gc_vmo(), 1));
        assert_balance(alice.acc(), init_bal, 0);
        assert_pruned(1);

        // Can prune rejected:
        assert_ok!(alice_proposal(400));
        assert_balance(alice.acc(), init_bal, 400);
        assert_eq!(Pips::pip_id_sequence(), 3);
        assert_eq!(Pips::active_pip_count(), 1);
        assert_ok!(Pips::reject_proposal(gc_vmo(), 2));
        assert_balance(alice.acc(), init_bal, 0);
        assert_state(2, false, ProposalState::Rejected);
        assert_eq!(Pips::active_pip_count(), 0);
        assert_ok!(Pips::prune_proposal(gc_vmo(), 2));
        assert_balance(alice.acc(), init_bal, 0);
        assert_pruned(2);
    });
}

#[test]
fn cannot_prune_active() {
    ExtBuilder::default().monied(true).build().execute_with(|| {
        System::set_block_number(1);

        let init_bal = 300;
        let alice = User::new(AccountKeyring::Alice).balance(init_bal);
        set_members(vec![alice.did]);

        // Alice starts a proposal with some deposit.
        assert_ok!(alice_proposal(50));
        assert_balance(alice.acc(), init_bal, 50);
        // Now remove that PIP and check that funds are back.
        assert_ok!(Pips::set_prune_historical_pips(root(), false));
        assert_state(0, false, ProposalState::Pending);
        assert_bad_state!(Pips::prune_proposal(gc_vmo(), 0));
        assert_eq!(Pips::pip_id_sequence(), 1);
        assert_eq!(Pips::active_pip_count(), 1);
        assert_balance(alice.acc(), init_bal, 50);

        // Alice starts a proposal with some deposit.
        assert_ok!(alice_proposal(60));
        assert_balance(alice.acc(), init_bal, 50 + 60);
        // Schedule the PIP.
        assert_ok!(Pips::snapshot(alice.origin()));
        assert_ok!(Pips::enact_snapshot_results(
            gc_vmo(),
            vec![(1, SnapshotResult::Approve)]
        ));
        assert_state(1, false, ProposalState::Scheduled);
        // Now remove that PIP and check that funds are back.
        assert_bad_state!(Pips::prune_proposal(gc_vmo(), 1));
        assert_eq!(Pips::pip_id_sequence(), 2);
        assert_eq!(Pips::active_pip_count(), 2);
        assert_balance(alice.acc(), init_bal, 50 + 60);
    });
}

#[test]
fn reject_proposal_works() {
    ExtBuilder::default().monied(true).build().execute_with(|| {
        System::set_block_number(1);

        let init_bal = 300;
        let alice = User::new(AccountKeyring::Alice).balance(init_bal);
        set_members(vec![alice.did]);

        // Alice starts a proposal with some deposit.
        assert_ok!(alice_proposal(50));
        assert_balance(alice.acc(), init_bal, 50);
        let result = VotingResult {
            ayes_count: 1,
            ayes_stake: 50,
            nays_count: 0,
            nays_stake: 0,
        };
        assert_eq!(Pips::proposal_result(0), result);

        // Now remove that PIP and check that funds are back.
        assert_ok!(Pips::set_prune_historical_pips(root(), false));
        assert_state(0, false, ProposalState::Pending);
        assert_ok!(Pips::reject_proposal(gc_vmo(), 0));
        assert_eq!(Pips::pip_id_sequence(), 1);
        assert_eq!(Pips::active_pip_count(), 0);
        assert_eq!(
            Pips::proposals(0).unwrap(),
            Pip {
                id: 0,
                proposal: make_proposal(42),
                state: ProposalState::Rejected,
                proposer: Proposer::Community(alice.acc()),
            }
        );
        assert_balance(alice.acc(), init_bal, 0);
        assert_eq!(Deposits::iter_prefix_values(0).count(), 0);
        // We keep this info for posterity.
        assert_eq!(Votes::iter_prefix_values(0).count(), 1);
        assert_eq!(Pips::proposal_result(0), result);

        // Alice starts a proposal with some deposit.
        assert_ok!(alice_proposal(60));
        assert_balance(alice.acc(), init_bal, 60);
        let result = VotingResult {
            ayes_count: 1,
            ayes_stake: 60,
            nays_count: 0,
            nays_stake: 0,
        };
        assert_eq!(Pips::proposal_result(1), result);

        // Schedule the PIP.
        assert_ok!(Pips::snapshot(alice.origin()));
        assert_ok!(Pips::enact_snapshot_results(
            gc_vmo(),
            vec![(1, SnapshotResult::Approve)]
        ));
        assert_state(1, false, ProposalState::Scheduled);

        // Now remove that PIP and check that funds are back.
        assert_ok!(Pips::reject_proposal(gc_vmo(), 1));
        assert_eq!(Pips::pip_id_sequence(), 2);
        assert_eq!(Pips::active_pip_count(), 0);
        assert_eq!(
            Pips::proposals(1).unwrap(),
            Pip {
                id: 1,
                proposal: make_proposal(42),
                state: ProposalState::Rejected,
                proposer: Proposer::Community(alice.acc()),
            }
        );
        assert_balance(alice.acc(), init_bal, 0);
        assert_eq!(Deposits::iter_prefix_values(1).count(), 0);
        // We keep this info for posterity.
        assert_eq!(Votes::iter_prefix_values(1).count(), 1);
        assert_eq!(Pips::proposal_result(1), result);
    });
}

#[test]
fn reject_proposal_will_unsnapshot() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        assert_ok!(Pips::set_prune_historical_pips(root(), false));

        let alice = User::new(AccountKeyring::Alice).balance(300);
        set_members(vec![alice.did]);

        assert_ok!(alice_proposal(0));
        assert_ok!(Pips::snapshot(alice.origin()));
        assert_eq!(Pips::snapshot_queue()[0].id, 0);
        assert_ok!(Pips::reject_proposal(gc_vmo(), 0));
        assert_eq!(Pips::snapshot_queue(), vec![]);
    });
}

#[test]
fn reject_proposal_will_unschedule() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        assert_ok!(Pips::set_prune_historical_pips(root(), false));

        let alice = User::new(AccountKeyring::Alice).balance(300);
        set_members(vec![alice.did]);

        let check = |id: PipId| {
            let scheduled_at = Pips::pip_to_schedule(id).unwrap();
            assert_ok!(Pips::reject_proposal(gc_vmo(), id));
            assert_eq!(Pips::pip_to_schedule(id), None);
            assert_event_exists!(
                EventTest::pallet_scheduler(pallet_scheduler::RawEvent::Canceled(when, ..)),
                *when == scheduled_at
            );
        };

        // Test snapshot method.
        assert_ok!(alice_proposal(0));
        assert_ok!(Pips::snapshot(alice.origin()));
        assert_ok!(Pips::enact_snapshot_results(
            gc_vmo(),
            vec![(0, SnapshotResult::Approve)]
        ));
        check(0);

        // Test committee method.
        assert_ok!(committee_proposal(0));
        assert_ok!(Pips::approve_committee_proposal(gc_vmo(), 1));
        check(1);
    });
}

#[test]
fn reschedule_execution_only_release_coordinator() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);

        let alice = User::new(AccountKeyring::Alice);
        let bob = User::new(AccountKeyring::Bob);
        let charlie = User::new(AccountKeyring::Charlie);
        set_members(vec![alice.did, bob.did, charlie.did]);
        assert_ok!(Committee::set_release_coordinator(gc_vmo(), charlie.did));

        assert_bad_origin!(Pips::reschedule_execution(root(), 0, None));
        assert_noop!(
            Pips::reschedule_execution(alice.origin(), 0, None),
            Error::RescheduleNotByReleaseCoordinator
        );
        assert_noop!(
            Pips::reschedule_execution(bob.origin(), 0, None),
            Error::RescheduleNotByReleaseCoordinator
        );

        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        let id = scheduled_proposal(&alice.origin(), 0);
        let scheduled_at = Pips::pip_to_schedule(id);
        consensus_call(
            pallet_pips::Call::reschedule_execution(0, None),
            &[&alice.origin(), &bob.origin(), &charlie.origin()],
        );
        assert_eq!(scheduled_at, Pips::pip_to_schedule(id));
        assert_ok!(Pips::reschedule_execution(charlie.origin(), id, None));
        assert_ne!(scheduled_at, Pips::pip_to_schedule(id));
    });
}

fn init_rc() -> Origin {
    let user = User::new(AccountKeyring::Alice);
    set_members(vec![user.did]);
    assert_ok!(Committee::set_release_coordinator(gc_vmo(), user.did));
    user.origin()
}

#[test]
fn reschedule_execution_no_such_proposal() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        let signer = init_rc();
        assert_no_pip!(Pips::reschedule_execution(signer, 0, None));
    });
}

#[test]
fn reschedule_execution_not_scheduled() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        let rc = init_rc();
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        let id = Pips::pip_id_sequence();
        assert_ok!(alice_proposal(0));
        assert_bad_state!(Pips::reschedule_execution(rc.clone(), id, None));
        assert_ok!(Pips::reject_proposal(gc_vmo(), id));
        assert_ok!(Pips::prune_proposal(gc_vmo(), id));
        let id = rejected_proposal();
        assert_bad_state!(Pips::reschedule_execution(rc.clone(), id, None));
        let id = executed_community_proposal(&rc);
        assert_bad_state!(Pips::reschedule_execution(rc.clone(), id, None));
        let id = failed_community_proposal(User::existing(AccountKeyring::Alice), 1337);
        assert_bad_state!(Pips::reschedule_execution(rc.clone(), id, None));
        let id = expired_proposal(2);
        assert_bad_state!(Pips::reschedule_execution(rc.clone(), id, None));
    });
}

#[test]
fn reschedule_execution_in_the_past() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        let rc = init_rc();
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        let id = scheduled_proposal(&rc, 0);
        let next = System::block_number() + 1;
        assert_noop!(
            Pips::reschedule_execution(rc.clone(), id, Some(next - 1)),
            Error::InvalidFutureBlockNumber
        );
        assert_noop!(
            Pips::reschedule_execution(rc, id, Some(next - 2)),
            Error::InvalidFutureBlockNumber
        );
    });
}

#[test]
fn reschedule_execution_works() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        let rc = init_rc();
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        let id = scheduled_proposal(&rc, 0);
        let scheduled_at = Pips::pip_to_schedule(id).unwrap();
        assert_eq!(1, Agenda::get(scheduled_at).len());

        let next = System::block_number() + 1;
        assert_ok!(Pips::reschedule_execution(rc.clone(), id, None));
        assert_event_exists!(EventTest::pallet_scheduler(
            pallet_scheduler::RawEvent::Canceled(..)
        ));
        assert_eq!(Pips::pip_to_schedule(id).unwrap(), next);
        assert_eq!(
            1, /* the Agenda vec item is None */
            Agenda::get(scheduled_at).len()
        );
        assert_eq!(1, Agenda::get(next).len());

        assert_ok!(Pips::reschedule_execution(rc.clone(), id, Some(next + 50)));
        assert_eq!(Pips::pip_to_schedule(id).unwrap(), next + 50);
        assert_eq!(
            1, /* the Agenda vec item is None */
            Agenda::get(scheduled_at).len()
        );
        assert_eq!(
            1, /* the Agenda vec item is None */
            Agenda::get(next).len()
        );
        assert_eq!(1, Agenda::get(next + 50).len());
    });
}

#[test]
fn clear_snapshot_not_gc_member() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        init_rc();
        assert_bad_origin!(Pips::clear_snapshot(root()));
        let bob = User::new(AccountKeyring::Bob);
        assert_noop!(
            Pips::clear_snapshot(bob.origin()),
            Error::NotACommitteeMember,
        );
    });
}

#[test]
fn clear_snapshot_works() {
    ExtBuilder::default().monied(true).build().execute_with(|| {
        System::set_block_number(1);
        let rc = init_rc();
        // No snapshot, but we can still clear.
        assert_eq!(Pips::snapshot_queue(), vec![]);
        assert_eq!(Pips::snapshot_metadata(), None);
        assert_ok!(Pips::clear_snapshot(rc.clone()));
        assert_eq!(Pips::snapshot_queue(), vec![]);
        assert_eq!(Pips::snapshot_metadata(), None);

        // Make a snapshot with something and clear it.
        assert_ok!(alice_proposal(100));
        assert_ok!(alice_proposal(200));
        assert_ok!(alice_proposal(400));
        assert_ok!(Pips::snapshot(rc.clone()));
        assert_ne!(Pips::snapshot_queue(), vec![]);
        assert_ne!(Pips::snapshot_metadata(), None);
        assert_ok!(Pips::clear_snapshot(rc));
        assert_eq!(Pips::snapshot_queue(), vec![]);
        assert_eq!(Pips::snapshot_metadata(), None);
    });
}

#[test]
fn snapshot_not_gc_member() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        init_rc();
        assert_bad_origin!(Pips::snapshot(root()));
        let bob = User::new(AccountKeyring::Bob);
        assert_noop!(Pips::snapshot(bob.origin()), Error::NotACommitteeMember);
    });
}

#[test]
fn snapshot_only_pending_hot_community() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        let rc = init_rc();

        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));

        let r = rejected_proposal();
        let e = executed_community_proposal(&rc);
        let f = failed_community_proposal(User::existing(AccountKeyring::Alice), 1337);
        let s = scheduled_proposal(&rc, 0);
        let ex = expired_proposal(1);
        assert_ok!(alice_proposal(0));
        let p = Pips::pip_id_sequence() - 1;
        for id in &[r, e, f, s, p, ex] {
            assert!(matches!(
                Pips::proposals(*id).unwrap().proposer,
                Proposer::Community(_)
            ));
        }
        assert_ok!(committee_proposal(0));

        assert_ok!(Pips::snapshot(rc));
        assert_eq!(Pips::snapshot_queue(), vec![spip(p, true, 0)]);
        assert_ne!(Pips::snapshot_metadata(), None);
    });
}

#[test]
fn snapshot_works() {
    ExtBuilder::default().monied(true).build().execute_with(|| {
        System::set_block_number(1);

        let user = User::new(AccountKeyring::Bob);
        set_members(vec![user.did]);

        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        assert_ok!(Pips::set_active_pip_limit(root(), 0));

        assert_ok!(alice_proposal(0)); // 0
        assert_ok!(Pips::vote(user.origin(), 0, true, 100));

        assert_ok!(alice_proposal(0)); // 1
        assert_ok!(Pips::vote(user.origin(), 1, false, 100));

        assert_ok!(alice_proposal(0)); // 2
        assert_ok!(alice_proposal(0)); // 3

        assert_ok!(alice_proposal(50)); // 4
        assert_ok!(Pips::vote(user.origin(), 4, true, 100));

        assert_ok!(alice_proposal(50)); // 5
        assert_ok!(Pips::vote(user.origin(), 5, false, 100));

        assert_ok!(Pips::snapshot(user.origin()));
        assert_eq!(
            Pips::snapshot_queue(),
            vec![
                spip(1, false, 100),
                spip(5, false, 50),
                spip(3, true, 0),
                spip(2, true, 0),
                spip(0, true, 100),
                spip(4, true, 150),
            ]
        );

        let assert_snapshot = |id| {
            assert_eq!(
                Pips::snapshot_metadata(),
                Some(SnapshotMetadata {
                    created_at: 1,
                    made_by: user.acc(),
                    id,
                })
            );
        };
        assert_snapshot(0);

        assert_ok!(Pips::snapshot(user.origin()));
        assert_snapshot(1);

        assert_ok!(Pips::snapshot(user.origin()));
        assert_snapshot(2);
    });
}

#[test]
fn enact_snapshot_results_input_too_large() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);

        let user = User::new(AccountKeyring::Bob);
        set_members(vec![user.did]);

        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));

        assert_ok!(Pips::snapshot(user.origin()));
        assert_noop!(
            Pips::enact_snapshot_results(gc_vmo(), vec![(0, SnapshotResult::Skip)]),
            Error::SnapshotResultTooLarge,
        );
        assert_noop!(
            Pips::enact_snapshot_results(
                gc_vmo(),
                vec![(0, SnapshotResult::Reject), (1, SnapshotResult::Approve)]
            ),
            Error::SnapshotResultTooLarge,
        );
        assert_ok!(alice_proposal(0));
        assert_ok!(alice_proposal(0));
        assert_ok!(Pips::snapshot(user.origin()));
        assert_noop!(
            Pips::enact_snapshot_results(
                gc_vmo(),
                vec![
                    (0, SnapshotResult::Reject),
                    (1, SnapshotResult::Approve),
                    (2, SnapshotResult::Skip)
                ]
            ),
            Error::SnapshotResultTooLarge,
        );
    });
}

#[test]
fn enact_snapshot_results_id_mismatch() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);

        let user = User::new(AccountKeyring::Bob);
        set_members(vec![user.did]);

        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));

        assert_ok!(alice_proposal(0));
        assert_ok!(alice_proposal(0));
        assert_ok!(Pips::snapshot(user.origin()));
        assert_noop!(
            Pips::enact_snapshot_results(gc_vmo(), vec![(1, SnapshotResult::Skip)]),
            Error::SnapshotIdMismatch,
        );
        assert_noop!(
            Pips::enact_snapshot_results(
                gc_vmo(),
                vec![(0, SnapshotResult::Reject), (2, SnapshotResult::Approve)]
            ),
            Error::SnapshotIdMismatch,
        );
    });
}

#[test]
fn enact_snapshot_results_works() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);

        let user = User::new(AccountKeyring::Bob);
        set_members(vec![user.did]);

        assert_ok!(Pips::set_prune_historical_pips(root(), false));
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));

        let mk_queue = |ids: &[PipId]| ids.iter().map(|&id| spip(id, true, 0)).collect::<Vec<_>>();

        // Make 3 PIPs, snapshot, and enact results for all, emptying the queue.
        assert_ok!(alice_proposal(0));
        assert_ok!(alice_proposal(0));
        assert_ok!(alice_proposal(0));
        assert_ok!(Pips::snapshot(user.origin()));
        assert_eq!(Pips::snapshot_queue(), mk_queue(&[2, 1, 0]));
        assert_eq!(Pips::pip_skip_count(1), 0);
        assert_ok!(Pips::enact_snapshot_results(
            gc_vmo(),
            vec![
                (0, SnapshotResult::Reject),
                (1, SnapshotResult::Skip),
                (2, SnapshotResult::Approve)
            ],
        ));
        assert_state(0, false, ProposalState::Rejected);
        assert_state(1, false, ProposalState::Pending);
        assert_eq!(Pips::pip_skip_count(1), 1);
        assert_state(2, false, ProposalState::Scheduled);
        assert_eq!(Pips::snapshot_queue(), vec![]);
        assert_ne!(Pips::snapshot_metadata(), None);

        // Add another proposal; we previously skipped one, so queue size is 2.
        // Only enact for 1 proposal, leaving the last added PIP in the queue.
        assert_ok!(alice_proposal(0));
        assert_ok!(Pips::snapshot(user.origin()));
        assert_eq!(Pips::snapshot_queue(), mk_queue(&[3, 1]));
        assert_ok!(Pips::enact_snapshot_results(
            gc_vmo(),
            vec![(1, SnapshotResult::Approve)]
        ));
        assert_last_event!(
            Event::SnapshotResultsEnacted(_, Some(1), a, b, c),
            a.is_empty() && b.is_empty() && c == &[1]
        );
        assert_state(1, false, ProposalState::Scheduled);
        assert_eq!(Pips::snapshot_queue(), mk_queue(&[3]));

        // Cleared queue + enacting zero-length results => noop.
        assert_ok!(Pips::clear_snapshot(user.origin()));
        assert_ok!(Pips::enact_snapshot_results(gc_vmo(), vec![]));
        assert_last_event!(
            Event::SnapshotResultsEnacted(_, None, a, b, c),
            a.is_empty() && b.is_empty() && c.is_empty()
        )
    });
}

#[test]
fn expiry_works() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));

        // Test non-prune logic. Prune logic is tested elsewhere.
        assert_ok!(Pips::set_prune_historical_pips(root(), false));
        let id = expired_proposal(13);
        assert_state(id, true, ProposalState::Expired);
        // Travel back in time, and ensure expiry is sticky.
        // This doesn't arise in a real world scenario, but tests an edge case in the code.
        System::set_block_number(1);
        assert_state(id, true, ProposalState::Expired);

        // Make sure non-pending PIPs cannot expire.
        assert_ok!(Pips::set_prune_historical_pips(root(), false));
        assert_ok!(Pips::set_pending_pip_expiry(root(), MaybeBlock::Some(13)));
        let alice = User::new(AccountKeyring::Alice);
        set_members(vec![alice.did]);
        let r = rejected_proposal();
        let e = executed_community_proposal(&alice.origin());
        let f = failed_community_proposal(User::existing(AccountKeyring::Alice), 1337);
        let s = scheduled_proposal(&alice.origin(), 0);
        fast_forward_blocks(13 + 100);
        for id in &[r, e, f, s] {
            assert_ne!(Pips::proposals(id).unwrap().state, ProposalState::Expired);
        }
    });
}

#[test]
#[should_panic = "called `Result::unwrap_err()` on an `Ok` value: 0"]
fn propose_dupe_live_insert_panics() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));

        // Manipulate storage to provoke panic in `insert_live_queue`.
        <LiveQueue<TestStorage>>::mutate(|queue| *queue = vec![spip(0, true, 0)]);

        // Triggers a panic, assertion never reached.
        assert_ok!(alice_proposal(0));
    });
}

#[test]
fn execute_scheduled_pip() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        assert_ok!(Pips::set_prune_historical_pips(root(), true));
        let pip_id = Pips::pip_id_sequence();
        assert_ok!(alice_remark_proposal(0));
        let user = User::new(AccountKeyring::Alice);
        set_members(vec![user.did]);
        assert_ok!(Pips::snapshot(user.origin()));
        assert_ok!(Pips::enact_snapshot_results(
            gc_vmo(),
            vec![(pip_id, SnapshotResult::Approve)],
        ));
        assert_state(pip_id, false, ProposalState::Scheduled);
        assert_ok!(Pips::execute_scheduled_pip(root(), pip_id));
        assert_pruned(pip_id);
    });
}

#[test]
fn expire_scheduled_pip() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        assert_ok!(Pips::set_prune_historical_pips(root(), true));
        let pip_id = Pips::pip_id_sequence();
        assert_ok!(alice_remark_proposal(0));
        assert_state(pip_id, false, ProposalState::Pending);
        assert_ok!(Pips::expire_scheduled_pip(root(), GC_DID, pip_id));
        assert_pruned(pip_id);
    });
}

#[test]
fn live_queue_off_by_one_insertion_regression_test() {
    ExtBuilder::default().monied(true).build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));

        assert_ok!(alice_proposal(2));
        assert_ok!(alice_proposal(4));
        assert_eq!(Pips::live_queue(), vec![spip(0, true, 2), spip(1, true, 4)]);

        let user = User::new(AccountKeyring::Bob);
        assert_ok!(Pips::vote(user.origin(), 0, true, 1));
        assert_eq!(Pips::live_queue(), vec![spip(0, true, 3), spip(1, true, 4)]);
    });
}

#[test]
fn live_queue_off_by_one_insertion_regression_test2() {
    ExtBuilder::default().monied(true).build().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Pips::set_min_proposal_deposit(root(), 0));
        assert_ok!(Pips::set_active_pip_limit(root(), 0));

        let user = User::new(AccountKeyring::Bob);

        assert_ok!(alice_proposal(0)); // 0
        assert_ok!(alice_proposal(0)); // 1
        assert_ok!(alice_proposal(50)); // 2
        assert_ok!(Pips::vote(user.origin(), 0, false, 100));
        assert_ok!(Pips::vote(user.origin(), 2, false, 100));
        assert_eq!(
            Pips::live_queue(),
            vec![spip(0, false, 100), spip(2, false, 50), spip(1, true, 0)]
        );
    });
}
