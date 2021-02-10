use super::{
    storage::{
        provide_scope_claim_to_multiple_parties, root, Balance, Checkpoint, MaxDidWhts,
        MaxTargetIds, TestStorage, User,
    },
    ExtBuilder,
};
use core::iter;
use frame_support::{
    assert_noop, assert_ok,
    dispatch::{DispatchError, DispatchResult},
    IterableStorageDoubleMap, StorageDoubleMap, StorageMap,
};
use pallet_asset::checkpoint::{ScheduleId, StoredSchedule};
use pallet_corporate_actions::{
    ballot::{self, BallotMeta, BallotTimeRange, BallotVote, Motion},
    distribution::{self, Distribution},
    Agent, CACheckpoint, CADetails, CAId, CAIdSequence, CAKind, CorporateAction, CorporateActions,
    LocalCAId, RecordDate, RecordDateSpec, TargetIdentities, TargetTreatment,
    TargetTreatment::{Exclude, Include},
    Tax,
};
use polymesh_common_utilities::asset::AssetFnTrait;
use polymesh_primitives::{
    asset::AssetName,
    calendar::{CheckpointId, CheckpointSchedule},
    AuthorizationData, Document, DocumentId, IdentityId, Moment, PortfolioId, PortfolioNumber,
    Signatory, Ticker,
};
use sp_arithmetic::Permill;
use std::convert::TryInto;
use test_client::AccountKeyring;

type System = frame_system::Module<TestStorage>;
type Origin = <TestStorage as frame_system::Trait>::Origin;
type Asset = pallet_asset::Module<TestStorage>;
type AssetError = pallet_asset::Error<TestStorage>;
type Timestamp = pallet_timestamp::Module<TestStorage>;
type Identity = pallet_identity::Module<TestStorage>;
type Authorizations = pallet_identity::Authorizations<TestStorage>;
type ComplianceManager = pallet_compliance_manager::Module<TestStorage>;
type CA = pallet_corporate_actions::Module<TestStorage>;
type Ballot = ballot::Module<TestStorage>;
type Dist = distribution::Module<TestStorage>;
type Portfolio = pallet_portfolio::Module<TestStorage>;
type Error = pallet_corporate_actions::Error<TestStorage>;
type BallotError = ballot::Error<TestStorage>;
type DistError = distribution::Error<TestStorage>;
type PError = pallet_portfolio::Error<TestStorage>;
type CPError = pallet_asset::checkpoint::Error<TestStorage>;
type Votes = ballot::Votes<TestStorage>;
type Custodian = pallet_portfolio::PortfolioCustodian;

const CDDP: AccountKeyring = AccountKeyring::Eve;

const P0: Permill = Permill::zero();
const P25: Permill = Permill::from_percent(25);
const P50: Permill = Permill::from_percent(50);
const P75: Permill = Permill::from_percent(75);

#[track_caller]
fn test(logic: impl FnOnce(Ticker, [User; 3])) {
    ExtBuilder::default()
        .cdd_providers(vec![CDDP.public()])
        .build()
        .execute_with(|| {
            System::set_block_number(1);

            // Create some users.
            let alice = User::new(AccountKeyring::Alice);
            let bob = User::new(AccountKeyring::Bob);
            let charlie = User::new(AccountKeyring::Charlie);

            // Create the asset.
            let ticker = create_asset(b"ACME", alice);

            // Execute the test.
            logic(ticker, [alice, bob, charlie])
        });
}

fn transfer(ticker: &Ticker, from: User, to: User) {
    // Provide scope claim to sender and receiver of the transaction.
    provide_scope_claim_to_multiple_parties(&[from.did, to.did], *ticker, CDDP.public());
    assert_ok!(Asset::base_transfer(
        PortfolioId::default_portfolio(from.did),
        PortfolioId::default_portfolio(to.did),
        ticker,
        500
    ));
}

fn create_asset(ticker: &[u8], owner: User) -> Ticker {
    let asset_name: AssetName = ticker.into();
    let ticker = ticker.try_into().unwrap();

    // Create the asset.
    assert_ok!(Asset::create_asset(
        owner.origin(),
        asset_name,
        ticker,
        1_000_000,
        true,
        <_>::default(),
        vec![],
        None
    ));

    assert_eq!(Asset::token_details(ticker).owner_did, owner.did);

    // Allow all transfers
    assert_ok!(ComplianceManager::add_compliance_requirement(
        owner.origin(),
        ticker,
        vec![],
        vec![]
    ));

    ticker
}

fn add_caa_auth(ticker: Ticker, from: User, to: User) -> u64 {
    let sig: Signatory<_> = to.did.into();
    let data = AuthorizationData::TransferCorporateActionAgent(ticker);
    assert_ok!(Identity::add_authorization(from.origin(), sig, data, None));
    Authorizations::iter_prefix_values(sig)
        .next()
        .unwrap()
        .auth_id
}

fn transfer_caa(ticker: Ticker, from: User, to: User) -> DispatchResult {
    let auth_id = add_caa_auth(ticker, from, to);
    Identity::accept_authorization(to.origin(), auth_id)
}

type CAResult = Result<CorporateAction, DispatchError>;

fn get_ca(id: CAId) -> Option<CorporateAction> {
    CA::corporate_actions(id.ticker, id.local_id)
}

fn init_ca(
    owner: User,
    ticker: Ticker,
    kind: CAKind,
    date: Option<RecordDateSpec>,
    details: String,
    targets: Option<TargetIdentities>,
    default_wht: Option<Tax>,
    wht: Option<Vec<(IdentityId, Tax)>>,
) -> CAResult {
    let id = next_ca_id(ticker);
    let sig = owner.origin();
    let details = CADetails(details.as_bytes().to_vec());
    let now = Checkpoint::now_unix();
    CA::initiate_corporate_action(
        sig,
        ticker,
        kind,
        now,
        date,
        details,
        targets,
        default_wht,
        wht,
    )?;
    Ok(get_ca(id).unwrap())
}

fn basic_ca(
    owner: User,
    ticker: Ticker,
    targets: Option<TargetIdentities>,
    default_wht: Option<Tax>,
    wht: Option<Vec<(IdentityId, Tax)>>,
) -> CAResult {
    init_ca(
        owner,
        ticker,
        CAKind::Other,
        None,
        <_>::default(),
        targets,
        default_wht,
        wht,
    )
}

fn dated_ca(owner: User, ticker: Ticker, kind: CAKind, rd: Option<RecordDateSpec>) -> CAResult {
    init_ca(owner, ticker, kind, rd, <_>::default(), None, None, None)
}

fn moment_ca(owner: User, ticker: Ticker, kind: CAKind, rd: Option<Moment>) -> CAResult {
    dated_ca(owner, ticker, kind, rd.map(RecordDateSpec::Scheduled))
}

fn set_schedule_complexity() {
    Timestamp::set_timestamp(1);
    assert_ok!(Checkpoint::set_schedules_max_complexity(root(), 1000));
}

fn next_ca_id(ticker: Ticker) -> CAId {
    let local_id = CA::ca_id_sequence(ticker);
    CAId { ticker, local_id }
}

const T_RANGE: BallotTimeRange = BallotTimeRange {
    start: 3000,
    end: 4000,
};

#[derive(Clone, Eq, PartialEq, Default, Debug)]
struct BallotData {
    meta: Option<BallotMeta>,
    range: Option<BallotTimeRange>,
    choices: Vec<u16>,
    rcv: bool,
    results: Vec<Balance>,
    votes: Vec<(IdentityId, Vec<BallotVote<Balance>>)>,
}

fn ballot_data(id: CAId) -> BallotData {
    BallotData {
        meta: Ballot::metas(id),
        range: Ballot::time_ranges(id),
        choices: Ballot::motion_choices(id),
        rcv: Ballot::rcv(id),
        results: Ballot::results(id),
        votes: Votes::iter_prefix(id).collect(),
    }
}

fn assert_ballot(id: CAId, data: &BallotData) {
    assert_eq!(&ballot_data(id), data);
}

#[test]
fn only_caa_authorized() {
    test(|ticker, [owner, caa, other]| {
        set_schedule_complexity();

        // Transfer some to Charlie & Bob.
        transfer(&ticker, owner, caa);
        transfer(&ticker, owner, other);

        let currency = create_asset(b"BETA", owner);

        macro_rules! checks {
            ($user:expr, $assert:ident $(, $tail:expr)?) => {
                // Check for `set_default_targets`, ...
                let owner_set_targets = |treatment| {
                    let ids = TargetIdentities { treatment, identities: vec![] };
                    CA::set_default_targets($user.origin(), ticker, ids)
                };
                $assert!(owner_set_targets(Include) $(, $tail)?);
                $assert!(owner_set_targets(Exclude) $(, $tail)?);
                // ...`set_default_withholding_tax`,
                $assert!(CA::set_default_withholding_tax(
                    $user.origin(),
                    ticker,
                    Permill::zero(),
                ) $(, $tail)?);
                // ...`set_did_withholding_tax`,
                $assert!(CA::set_did_withholding_tax(
                    $user.origin(),
                    ticker,
                    other.did,
                    None,
                ) $(, $tail)?);
                // ..., `initiate_corporate_action`,
                let record_date = Some(RecordDateSpec::Scheduled(2000));
                let mk_ca = |kind| dated_ca($user, ticker, kind, record_date);
                let id = next_ca_id(ticker);
                $assert!(mk_ca(CAKind::IssuerNotice) $(, $tail)?);
                // ..., `link_ca_doc`,
                $assert!(CA::link_ca_doc($user.origin(), id, vec![]) $(, $tail)?);
                // ..., `change_record_date`,
                $assert!(CA::change_record_date($user.origin(), id, record_date) $(, $tail)?);
                // ..., `attach_ballot`,
                let meta = BallotMeta::default();
                $assert!(Ballot::attach_ballot($user.origin(), id, T_RANGE, meta.clone(), false) $(, $tail)?);
                // ..., `change_end`,
                $assert!(Ballot::change_end($user.origin(), id, 5000) $(, $tail)?);
                // ..., `change_meta`,
                $assert!(Ballot::change_meta($user.origin(), id, meta) $(, $tail)?);
                // ..., `change_rcv`,
                $assert!(Ballot::change_rcv($user.origin(), id, true) $(, $tail)?);
                // ..., `remove_ballot`,
                $assert!(Ballot::remove_ballot($user.origin(), id) $(, $tail)?);
                // ..., `remove_ca`,
                $assert!(CA::remove_ca($user.origin(), id) $(, $tail)?);
                // ..., `distribute`,
                let id = next_ca_id(ticker);
                $assert!(mk_ca(CAKind::UnpredictableBenefit) $(, $tail)?);
                $assert!(Dist::distribute($user.origin(), id, None, currency, 0, 3000, None) $(, $tail)?);
                // ..., and `remove_distribution`.
                $assert!(Dist::remove_distribution($user.origin(), id) $(, $tail)?);
            };
        }
        // Ensures passing for owner, but not to-be-CAA (Bob) and other.
        let owner_can_do_it = || {
            checks!(owner, assert_ok);
            checks!(caa, assert_noop, Error::UnauthorizedAsAgent);
            checks!(other, assert_noop, Error::UnauthorizedAsAgent);
        };
        // Ensures passing for Bob (the CAA), not owner, and not other.
        let caa_can_do_it = || {
            checks!(caa, assert_ok);
            checks!(owner, assert_noop, Error::UnauthorizedAsAgent);
            checks!(other, assert_noop, Error::UnauthorizedAsAgent);
        };
        let transfer_caa = |caa| {
            assert_ok!(transfer_caa(ticker, owner, caa));
        };

        // We start with owner being CAA.
        owner_can_do_it();
        // Transfer CAA to Bob.
        transfer_caa(caa);
        caa_can_do_it();
        // Demonstrate that CAA can be transferred back.
        transfer_caa(owner);
        owner_can_do_it();
        // Transfer to Bob again.
        transfer_caa(caa);
        caa_can_do_it();
        // Finally reset; ensuring that CAA is owner.
        assert_ok!(CA::reset_caa(owner.origin(), ticker));
        owner_can_do_it();
    });
}

#[test]
fn only_owner_reset() {
    test(|ticker, [owner, caa, other]| {
        assert_ok!(transfer_caa(ticker, owner, caa));
        let reset = |caller: User| CA::reset_caa(caller.origin(), ticker);
        assert_ok!(reset(owner));
        assert_noop!(reset(caa), AssetError::Unauthorized);
        assert_noop!(reset(other), AssetError::Unauthorized);
    });
}

#[test]
fn only_owner_caa_invite() {
    test(|ticker, [_, caa, other]| {
        let auth_id = add_caa_auth(ticker, other, caa);
        assert_noop!(
            Identity::accept_authorization(caa.origin(), auth_id),
            "Illegal use of Authorization"
        );
    });
}

#[test]
fn not_holder_works() {
    test(|ticker, [owner, _, other]| {
        assert_ok!(CA::set_did_withholding_tax(
            owner.origin(),
            ticker,
            other.did,
            None
        ));

        assert_ok!(CA::set_default_targets(
            owner.origin(),
            ticker,
            TargetIdentities {
                treatment: Exclude,
                identities: vec![other.did],
            }
        ));
    });
}

#[test]
fn set_default_targets_limited() {
    test(|ticker, [owner, ..]| {
        let mut ids = TargetIdentities {
            treatment: Exclude,
            identities: (0..=MaxTargetIds::get() as u128)
                .map(IdentityId::from)
                .collect(),
        };

        let set = |ids| CA::set_default_targets(owner.origin(), ticker, ids);
        let ca = |ids| basic_ca(owner, ticker, Some(ids), None, None);
        assert_noop!(set(ids.clone()), Error::TooManyTargetIds);
        assert_noop!(ca(ids.clone()), Error::TooManyTargetIds);
        ids.identities.pop();
        assert_ok!(set(ids.clone()));
        assert_ok!(ca(ids));
    });
}

#[test]
fn set_default_targets_works() {
    test(|ticker, [owner, foo, bar]| {
        transfer(&ticker, owner, foo);
        transfer(&ticker, owner, bar);

        let set = |treatment, identities, expect_ids| {
            let ids = TargetIdentities {
                treatment,
                identities,
            };
            assert_ok!(CA::set_default_targets(owner.origin(), ticker, ids));
            let ids = TargetIdentities {
                treatment,
                identities: expect_ids,
            };
            assert_eq!(CA::default_target_identities(ticker), ids);
        };
        let expect = vec![foo.did, bar.did];
        set(Exclude, expect.clone(), expect.clone());
        set(Exclude, vec![bar.did, foo.did], expect.clone());
        set(Include, vec![foo.did, bar.did, foo.did], expect);
    });
}

#[test]
fn set_default_withholding_tax_works() {
    test(|ticker, [owner, ..]| {
        assert_eq!(CA::default_withholding_tax(ticker), P0);
        assert_ok!(CA::set_default_withholding_tax(owner.origin(), ticker, P50));
        assert_eq!(CA::default_withholding_tax(ticker), P50);
    });
}

#[test]
fn set_did_withholding_tax_limited() {
    test(|ticker, [owner, ..]| {
        let tax = Tax::one();
        let max = MaxDidWhts::get() as u128;
        let ids = (0..max).map(IdentityId::from);
        let next = IdentityId::from(max);
        let last = IdentityId::from(max - 1);

        // Test in CA creation.
        let mut whts = ids.clone().map(|did| (did, tax)).collect::<Vec<_>>();
        let ca = |whts| basic_ca(owner, ticker, None, None, Some(whts));
        assert_ok!(ca(whts.clone()));
        whts.push((next, tax)); // Intentionally using `next` to test pre-dedup failure.
        assert_noop!(ca(whts), Error::TooManyDidTaxes);

        // Test in asset level defaults.
        let set = |did, tax| CA::set_did_withholding_tax(owner.origin(), ticker, did, tax);
        for did in ids {
            assert_ok!(set(did, Some(tax)));
        }
        assert_noop!(set(next, Some(tax)), Error::TooManyDidTaxes);
        assert_ok!(set(last, Some(Tax::zero())));
        assert_ok!(set(last, None));
        assert_ok!(set(next, Some(tax)));
    });
}

#[test]
fn set_did_withholding_tax_works() {
    test(|ticker, [owner, foo, bar]| {
        transfer(&ticker, owner, foo);
        transfer(&ticker, owner, bar);

        // We will insert bar first, but still expect foo first in results, as DIDs are sorted.
        assert!(foo.did < bar.did);

        let check = |user: User, tax, expect| {
            assert_ok!(CA::set_did_withholding_tax(
                owner.origin(),
                ticker,
                user.did,
                tax
            ));
            assert_eq!(CA::did_withholding_tax(ticker), expect);
        };
        check(bar, Some(P25), vec![(bar.did, P25)]);
        check(foo, Some(P75), vec![(foo.did, P75), (bar.did, P25)]);
        check(bar, Some(P50), vec![(foo.did, P75), (bar.did, P50)]);
        check(bar, None, vec![(foo.did, P75)]);
    });
}

#[test]
fn set_max_details_length_only_root() {
    ExtBuilder::default().build().execute_with(|| {
        let alice = User::new(AccountKeyring::Alice).origin();
        assert_noop!(
            CA::set_max_details_length(alice, 5),
            DispatchError::BadOrigin,
        );
        assert_ok!(CA::set_max_details_length(root(), 10));
        assert_eq!(CA::max_details_length(), 10);
    });
}

#[test]
fn initiate_corporate_action_details() {
    test(|ticker, [owner, ..]| {
        assert_ok!(CA::set_max_details_length(root(), 2));
        let init_ca = |details: &str| -> DispatchResult {
            let ca = init_ca(
                owner,
                ticker,
                CAKind::Other,
                None,
                details.to_owned(),
                None,
                None,
                None,
            )?;
            assert_eq!(details.as_bytes(), ca.details.as_slice());
            Ok(())
        };
        assert_ok!(init_ca("f"));
        assert_ok!(init_ca("fo"));
        assert_noop!(init_ca("foo"), Error::DetailsTooLong);
        assert_noop!(init_ca("❤️"), Error::DetailsTooLong);
    });
}

#[test]
fn initiate_corporate_action_local_id_overflow() {
    test(|ticker, [owner, ..]| {
        CAIdSequence::insert(ticker, LocalCAId(u32::MAX - 2));
        let init_ca = || dated_ca(owner, ticker, CAKind::Other, None);
        assert_ok!(init_ca()); // -2; OK
        assert_ok!(init_ca()); // -1; OK
        assert_noop!(init_ca(), Error::LocalCAIdOverflow); // 0; Next overflows, so error already.
    });
}

#[test]
fn initiate_corporate_action_record_date() {
    test(|ticker, [owner, foo, _]| {
        assert_ok!(Checkpoint::set_schedules_max_complexity(root(), 1));

        Timestamp::set_timestamp(0);

        let mut cp_id = CheckpointId(0);
        let mut schedule_id = ScheduleId(0);

        let mut check = |date| {
            let ca = moment_ca(owner, ticker, CAKind::Other, date).unwrap();
            assert_eq!(date, ca.record_date.map(|x| x.date));
            if let (Some(date), Some(rd)) = (date, ca.record_date) {
                cp_id.0 += 1;
                schedule_id.0 += 1;

                assert_eq!(date, rd.date);
                match rd.checkpoint {
                    CACheckpoint::Scheduled(id, 0) => assert_eq!(schedule_id, id),
                    _ => panic!(),
                }

                Timestamp::set_timestamp(date);
                transfer(&ticker, owner, foo);

                assert_eq!(
                    Checkpoint::schedule_points(ticker, schedule_id),
                    vec![cp_id]
                );
                assert_eq!(date, Checkpoint::timestamps(ticker, cp_id));
            }
        };

        check(None);
        check(Some(50_000));
        check(Some(100_000));

        assert_eq!(Checkpoint::checkpoint_id_sequence(ticker), CheckpointId(2));
    });
}

const ALL_CA_KINDS: &[CAKind] = &[
    CAKind::PredictableBenefit,
    CAKind::UnpredictableBenefit,
    CAKind::IssuerNotice,
    CAKind::Reorganization,
    CAKind::Other,
];

#[test]
fn initiate_corporate_action_kind() {
    test(|ticker, [owner, ..]| {
        for kind in ALL_CA_KINDS {
            assert_eq!(*kind, dated_ca(owner, ticker, *kind, None).unwrap().kind);
        }
    });
}

#[test]
fn initiate_corporate_action_decl_date() {
    test(|ticker, [owner, ..]| {
        set_schedule_complexity();

        let ca = |decl, record| -> DispatchResult {
            let id = next_ca_id(ticker);
            CA::initiate_corporate_action(
                owner.origin(),
                ticker,
                CAKind::Other,
                decl,
                record,
                "".into(),
                None,
                None,
                None,
            )?;
            assert_eq!(get_ca(id).unwrap().decl_date, decl);
            Ok(())
        };

        // Now + no record date works.
        let now = Checkpoint::now_unix();
        assert_ok!(ca(now, None));

        // Now + 1 in the future => error.
        assert_noop!(ca(now + 1, None), Error::DeclDateInFuture);

        // decl date == now == record date
        assert_ok!(ca(now, Some(RecordDateSpec::Scheduled(now))));

        // decl date == now + 1 > record date => error
        assert_noop!(
            ca(now + 1, Some(RecordDateSpec::Scheduled(now))),
            Error::DeclDateInFuture
        );
    });
}

#[test]
fn initiate_corporate_action_default_tax() {
    test(|ticker, [owner, ..]| {
        let ca = |dwt| {
            basic_ca(owner, ticker, None, dwt, None)
                .unwrap()
                .default_withholding_tax
        };
        assert_ok!(CA::set_default_withholding_tax(owner.origin(), ticker, P25));
        assert_eq!(ca(None), P25);
        assert_eq!(ca(Some(P50)), P50);
    });
}

#[test]
fn initiate_corporate_action_did_tax() {
    test(|ticker, [owner, foo, bar]| {
        let ca = |wt| {
            basic_ca(owner, ticker, None, None, wt)
                .unwrap()
                .withholding_tax
        };

        let wts = vec![(foo.did, P25), (bar.did, P75)];
        for (did, wt) in wts.iter().copied() {
            assert_ok!(CA::set_did_withholding_tax(
                owner.origin(),
                ticker,
                did,
                Some(wt)
            ));
        }
        assert_eq!(ca(None), wts);

        // Also ensure `foo` is sorted before `bar` despite providing `bar` first.
        assert!(foo.did < bar.did);
        assert_eq!(
            ca(Some(vec![(bar.did, P50), (foo.did, P0)])),
            vec![(foo.did, P0), (bar.did, P50)]
        );
    });
}

#[test]
#[should_panic]
fn initiate_corporate_action_did_tax_dupe() {
    test(|ticker, [owner, foo, bar]| {
        let wt = Some(vec![(bar.did, P75), (foo.did, P0), (bar.did, P50)]);
        basic_ca(owner, ticker, None, None, wt).unwrap();
    });
}

#[test]
fn initiate_corporate_action_targets() {
    test(|ticker, [owner, foo, bar]| {
        let ca = |targets| {
            basic_ca(owner, ticker, targets, None, None)
                .unwrap()
                .targets
        };
        let ids = |treatment, identities| TargetIdentities {
            treatment,
            identities,
        };

        let t1 = ids(Include, vec![foo.did]);
        assert_ok!(CA::set_default_targets(owner.origin(), ticker, t1.clone()));
        assert_eq!(ca(None), t1);

        assert_eq!(
            ca(Some(ids(Exclude, vec![bar.did, foo.did, bar.did]))),
            ids(Exclude, vec![foo.did, bar.did]),
        );
    });
}

fn add_doc(owner: User, ticker: Ticker) {
    let doc = Document {
        name: b"foo".into(),
        uri: b"https://example.com".into(),
        content_hash: [1u8; 16][..].try_into().unwrap(),
        doc_type: None,
        filing_date: None,
    };
    assert_ok!(Asset::add_documents(owner.origin(), vec![doc], ticker));
}

#[test]
fn link_ca_docs_works() {
    test(|ticker, [owner, ..]| {
        let local_id = LocalCAId(0);
        let id = CAId { ticker, local_id };

        let link = |docs| CA::link_ca_doc(owner.origin(), id, docs);
        let link_ok = |docs: Vec<_>| {
            assert_ok!(link(docs.clone()));
            assert_eq!(CA::ca_doc_link(id), docs);
        };

        // Link to a CA that doesn't exist, and ensure failure.
        assert_noop!(link(vec![]), Error::NoSuchCA);

        // Make it exist, and check that linking to no docs works.
        basic_ca(owner, ticker, None, None, None).unwrap();
        link_ok(vec![]);

        // Now link it to docs that don't exist, and ensure failure.
        let id0 = DocumentId(0);
        assert_noop!(link(vec![id0]), AssetError::NoSuchDoc);

        // Add the document.
        add_doc(owner, ticker);

        // The document exists, but we add a second one that does not, so still expecting failure.
        assert_noop!(link(vec![id0, DocumentId(1)]), AssetError::NoSuchDoc);

        // Finally, we only link the document, and it all works out.
        link_ok(vec![id0]);
    });
}

#[test]
fn remove_ca_works() {
    test(|ticker, [owner, ..]| {
        set_schedule_complexity();

        let ca = |kind, rd| moment_ca(owner, ticker, kind, rd).unwrap();
        let remove = |id| CA::remove_ca(owner.origin(), id);

        let assert_no_ca = |id: CAId| {
            assert_eq!(None, get_ca(id));
            assert_eq!(CA::ca_doc_link(id), vec![]);
        };

        // Remove a CA that doesn't exist, and ensure failure.
        let id = next_ca_id(ticker);
        assert_noop!(remove(id), Error::NoSuchCA);

        // Create a CA, remove it, and ensure its no longer there.
        ca(CAKind::Other, None);
        add_doc(owner, ticker);
        let docs = vec![DocumentId(0)];
        assert_ok!(CA::link_ca_doc(owner.origin(), id, docs.clone()));
        assert_eq!(docs, CA::ca_doc_link(id));
        assert_ok!(remove(id));
        assert_no_ca(id);

        // Create a ballot CA, which hasn't started.
        let time = BallotTimeRange {
            start: 3000,
            end: 4000,
        };
        let motion = Motion {
            title: "".into(),
            info_link: "".into(),
            choices: vec!["".into()],
        };
        let meta = BallotMeta {
            title: vec![].into(),
            motions: vec![motion],
        };
        let mk_ballot = || {
            Timestamp::set_timestamp(0);
            let id = next_ca_id(ticker);
            ca(CAKind::IssuerNotice, Some(1000));
            assert_ballot(id, &<_>::default());
            assert_ok!(Ballot::attach_ballot(
                owner.origin(),
                id,
                time,
                meta.clone(),
                true,
            ));
            id
        };
        let id = mk_ballot();
        // Ensure the details are right.
        assert_ballot(
            id,
            &BallotData {
                meta: Some(meta.clone()),
                range: Some(time),
                choices: vec![1u16],
                rcv: true,
                ..<_>::default()
            },
        );
        // Sucessfully remove it. Edge condition `now == start - 1`.
        Timestamp::set_timestamp(3000 - 1);
        assert_ok!(remove(id));
        // And ensure all details were removed.
        assert_no_ca(id);
        assert_ballot(id, &<_>::default());

        // Create another ballot, move now => start date; try to remove, but fail.
        let id = mk_ballot();
        Timestamp::set_timestamp(3000); // now == start
        assert_noop!(remove(id), BallotError::VotingAlreadyStarted);
        Timestamp::set_timestamp(3001); // now == start + 1
        assert_noop!(remove(id), BallotError::VotingAlreadyStarted);

        // Create a distribution CA, which hasn't started.
        let currency = create_asset(b"BETA", owner);
        let mk_dist = || {
            Timestamp::set_timestamp(0);
            let id = next_ca_id(ticker);
            ca(CAKind::UnpredictableBenefit, Some(1000));
            assert_ok!(Dist::distribute(
                owner.origin(),
                id,
                None,
                currency,
                0,
                3000,
                None,
            ));
            id
        };
        let id = mk_dist();
        // Ensure the details are right.
        assert_eq!(
            Dist::distributions(id),
            Some(Distribution {
                from: PortfolioId::default_portfolio(owner.did),
                currency,
                amount: 0,
                remaining: 0,
                reclaimed: false,
                payment_at: 3000,
                expires_at: None,
            }),
        );
        // Sucessfully remove it. Edge condition `now == start - 1`.
        Timestamp::set_timestamp(3000 - 1);
        assert_ok!(remove(id));
        // And ensure all details were removed.
        assert_no_ca(id);
        assert_eq!(Dist::distributions(id), None);
    });
}

fn next_schedule_id(ticker: Ticker) -> ScheduleId {
    let ScheduleId(id) = Checkpoint::schedule_id_sequence(ticker);
    ScheduleId(id + 1)
}

#[test]
fn change_record_date_works() {
    test(|ticker, [owner, ..]| {
        set_schedule_complexity();

        let ca = |kind, rd| moment_ca(owner, ticker, kind, rd).unwrap();
        let change = |id, date| CA::change_record_date(owner.origin(), id, date);
        let change_ok = |id, date, expect| {
            assert_ok!(change(id, date));
            assert_eq!(expect, get_ca(id).unwrap().record_date);
        };
        let assert_refs =
            |sh_id, count| assert_eq!(Checkpoint::schedule_ref_count(ticker, sh_id), count);
        let assert_fresh = |sh_id| assert_eq!(Checkpoint::schedule_id_sequence(ticker), sh_id);

        // Change for a CA that doesn't exist, and ensure failure.
        let id = next_ca_id(ticker);
        assert_noop!(change(id, None), Error::NoSuchCA);

        let spec_ts = |ts| Some(RecordDateSpec::Scheduled(ts));
        let spec_cp = |id| Some(RecordDateSpec::Existing(CheckpointId(id)));
        let spec_sh = |id| Some(RecordDateSpec::ExistingSchedule(id));
        let rd_cp = |date, id| {
            let checkpoint = CACheckpoint::Existing(CheckpointId(id));
            Some(RecordDate { date, checkpoint })
        };
        let rd_ts = |date, id| {
            let checkpoint = CACheckpoint::Scheduled(id, 0);
            Some(RecordDate { date, checkpoint })
        };

        // Trigger `NoSuchCheckpointId`.
        ca(CAKind::Other, None);
        assert_noop!(change(id, spec_cp(42)), Error::NoSuchCheckpointId);

        // Successfully use a checkpoint which exists.
        assert_ok!(Checkpoint::create_checkpoint(owner.origin(), ticker));
        change_ok(id, spec_cp(1), rd_cp(1, 1));

        // Trigger `NoSuchSchedule`.
        assert_noop!(change(id, spec_sh(ScheduleId(42))), CPError::NoSuchSchedule);

        // Successfully use a schedule which exists (the same one as before).
        let mk_schedule = |at, id| {
            let period = <_>::default();
            let schedule = CheckpointSchedule { start: at, period };
            StoredSchedule {
                at,
                id,
                schedule,
                remaining: 0,
            }
        };
        let mut all_schedules = vec![];
        let mut change_ok_scheduled = || {
            let sh_id = next_schedule_id(ticker);
            change_ok(id, spec_ts(1000), rd_ts(1000, sh_id));
            assert_fresh(sh_id);
            assert_refs(sh_id, 1);
            all_schedules.push(mk_schedule(1000, sh_id));
            assert_eq!(Checkpoint::schedules(ticker), all_schedules);
            change_ok(id, spec_sh(sh_id), rd_ts(1000, sh_id));
            sh_id
        };
        let sh_id1 = change_ok_scheduled();
        assert_eq!(Checkpoint::schedule_ref_count(ticker, sh_id1), 1);

        // Then use a distinct existing ID.
        let sh_id2 = change_ok_scheduled();
        assert_refs(sh_id1, 0);
        assert_refs(sh_id2, 1);

        // Use a removable schedule. Should increment strong ref count.
        let sh_id3 = next_schedule_id(ticker);
        assert_ok!(Checkpoint::create_schedule(
            owner.origin(),
            ticker,
            2000.into()
        ));
        assert_fresh(sh_id3);
        assert_refs(sh_id3, 0);
        assert_eq!(
            Checkpoint::schedules(ticker),
            vec![
                mk_schedule(1000, sh_id1),
                mk_schedule(1000, sh_id2),
                mk_schedule(2000, sh_id3)
            ]
        );
        change_ok(id, spec_sh(sh_id3), rd_ts(2000, sh_id3));
        assert_refs(sh_id3, 1);

        // While at it, let's create a bunch of CAs and use the last schedule.
        for _ in 0..10 {
            let id = next_ca_id(ticker);
            ca(CAKind::IssuerNotice, Some(1000));
            change_ok(id, spec_sh(sh_id3), rd_ts(2000, sh_id3));
        }
        assert_refs(sh_id3, 11);

        // No need to test `RecordDateSpec::Scheduled` branch beyond what we have here.
        // To do so would replicate tests in the checkpoint module.

        // Test ballot branch.
        let id = next_ca_id(ticker);
        ca(CAKind::IssuerNotice, Some(1000));
        let time = BallotTimeRange {
            start: 5000,
            end: 7000,
        };
        let meta = BallotMeta::default();
        assert_ok!(Ballot::attach_ballot(owner.origin(), id, time, meta, true));
        let test_branch = |id, error: DispatchError| {
            let change_ok = |spec, expect| {
                change_ok(id, spec_ts(spec), rd_ts(expect, next_schedule_id(ticker)))
            };
            Timestamp::set_timestamp(3000);
            change_ok(4999, 4000); // floor(4999 / 1000) * 1000 == 4000
            Timestamp::set_timestamp(4999);
            change_ok(4999, 4999); // Flooring not applied cause now == 2999.
            change_ok(5000, 5000); // floor(5000 / 1000) * 1000 == 5000
            change_ok(5001, 5000); // floor(5001 / 1000) * 1000 == 5000
            Timestamp::set_timestamp(5001);
            assert_noop!(change(id, spec_ts(5001)), Error::RecordDateAfterStart); // 5001 < 5000
            assert_noop!(change(id, spec_ts(6000)), Error::RecordDateAfterStart); // 6000 < 5000
            Timestamp::set_timestamp(6000);
            assert_noop!(change(id, spec_cp(1)), error); // 6000 < 4000
            Timestamp::set_timestamp(6001);
            assert_noop!(change(id, spec_cp(1)), error); // 6001 < 4000
        };
        test_branch(id, BallotError::VotingAlreadyStarted.into());

        // Test distribution branch.
        Timestamp::set_timestamp(0);
        let id = next_ca_id(ticker);
        ca(CAKind::PredictableBenefit, Some(1000));
        assert_ok!(Dist::distribute(
            owner.origin(),
            id,
            None,
            create_asset(b"BETA", owner),
            0,
            5000,
            None,
        ));
        test_branch(id, DistError::DistributionStarted.into());
    });
}

#[test]
fn existing_schedule_ref_count() {
    test(|ticker, [owner, ..]| {
        set_schedule_complexity();

        let sh_id = next_schedule_id(ticker);
        let spec = Some(RecordDateSpec::ExistingSchedule(sh_id));
        let assert_refs = |count| assert_eq!(Checkpoint::schedule_ref_count(ticker, sh_id), count);
        let remove_ca = |id| CA::remove_ca(owner.origin(), id);
        let remove_sh = || Checkpoint::remove_schedule(owner.origin(), ticker, sh_id);

        // No schedule yet, but count is 0 by default.
        assert_refs(0);

        // Schedule made, count still 0 as no CAs attached.
        assert_ok!(Checkpoint::create_schedule(
            owner.origin(),
            ticker,
            1000.into()
        ));
        assert_refs(0);

        // Attach some CAs, bumping the count by that many.
        let mut ids = (0..5)
            .map(|_| {
                let ca_id = next_ca_id(ticker);
                assert_ok!(dated_ca(owner, ticker, CAKind::IssuerNotice, spec));
                ca_id
            })
            .collect::<Vec<_>>();
        assert_refs(5);

        // Remove all of them, except for one, with the count going back to 1.
        let stashed_id = ids.pop().unwrap();
        for id in ids {
            assert_ok!(remove_ca(id));
        }
        assert_refs(1);

        // Trying to remove the checkpoint, but we're blocked from doing so.
        assert_noop!(remove_sh(), CPError::ScheduleNotRemovable);

        // Remove the last one.. Ref count -> 0.
        assert_ok!(remove_ca(stashed_id));

        // Bow we're able to remove the schedule.
        assert_ok!(remove_sh());
    });
}

fn attach(owner: User, id: CAId, rcv: bool) -> DispatchResult {
    Ballot::attach_ballot(owner.origin(), id, T_RANGE, mk_meta(), rcv)
}

#[test]
fn attach_ballot_no_such_ca() {
    test(|ticker, [owner, ..]| {
        let id = next_ca_id(ticker);
        assert_noop!(attach(owner, id, true), Error::NoSuchCA);
    });
}

#[test]
fn attach_ballot_only_notice() {
    test(|ticker, [owner, ..]| {
        set_schedule_complexity();
        let attach = |id| attach(owner, id, true);
        for &kind in ALL_CA_KINDS {
            let id = next_ca_id(ticker);
            assert_ok!(moment_ca(owner, ticker, kind, Some(1000)));
            if let CAKind::IssuerNotice = kind {
                assert_ok!(attach(id));
            } else {
                assert_noop!(attach(id), BallotError::CANotNotice);
            }
        }
    });
}

fn notice_ca(owner: User, ticker: Ticker, rd: Option<Moment>) -> Result<CAId, DispatchError> {
    let id = next_ca_id(ticker);
    moment_ca(owner, ticker, CAKind::IssuerNotice, rd)?;
    Ok(id)
}

#[test]
fn attach_ballot_range_invariant() {
    test(|ticker, [owner, ..]| {
        set_schedule_complexity();

        let id = notice_ca(owner, ticker, Some(1000)).unwrap();

        let mut data = BallotData {
            rcv: true,
            meta: Some(<_>::default()),
            ..BallotData::default()
        };

        let mut attach = |id, time| -> DispatchResult {
            data.range = Some(time);
            let meta = data.meta.clone().unwrap();
            Ballot::attach_ballot(owner.origin(), id, time, meta, data.rcv)?;
            assert_ballot(id, &data);
            Ok(())
        };
        let range = |start| BallotTimeRange { start, end: 6000 };

        assert_noop!(attach(id, range(6001)), BallotError::StartAfterEnd);

        Timestamp::set_timestamp(6001);
        assert_noop!(attach(id, range(6000)), BallotError::NowAfterEnd);

        Timestamp::set_timestamp(4000);
        assert_ok!(attach(id, range(6000)));

        let id = notice_ca(owner, ticker, Some(5000)).unwrap();
        assert_noop!(attach(id, range(4999)), Error::RecordDateAfterStart);
        assert_ok!(attach(id, range(5000)));

        let id = notice_ca(owner, ticker, None).unwrap();
        assert_noop!(attach(id, range(6000)), Error::NoRecordDate);
    });
}

#[test]
fn attach_ballot_already_exists() {
    test(|ticker, [owner, ..]| {
        set_schedule_complexity();

        let id = notice_ca(owner, ticker, Some(1000)).unwrap();

        let attach = |id| Ballot::attach_ballot(owner.origin(), id, T_RANGE, mk_meta(), true);

        assert_ok!(attach(id));
        assert_noop!(attach(id), BallotError::AlreadyExists);
        assert_ok!(Ballot::remove_ballot(owner.origin(), id));
        assert_ok!(attach(id));
    });
}

fn overflowing_meta() -> BallotMeta {
    BallotMeta {
        title: "".into(),
        motions: vec![Motion {
            title: "".into(),
            info_link: "".into(),
            choices: iter::repeat("".into())
                // `u16::MAX` doesn't overflow, but +1 does.
                .take(1 + u16::MAX as usize)
                .collect(),
        }],
    }
}

#[test]
fn attach_ballot_num_choices_overflow_u16() {
    test(|ticker, [owner, ..]| {
        set_schedule_complexity();

        // N.B. we do not test the total-choices-overflows-usize case since
        // that actually requires allocating an `usize` + 1 number of choices,
        // which is not reasonable as a test.

        let id = notice_ca(owner, ticker, Some(1000)).unwrap();
        assert_noop!(
            Ballot::attach_ballot(owner.origin(), id, T_RANGE, overflowing_meta(), false),
            BallotError::NumberOfChoicesOverflow,
        );
    });
}

fn mk_meta() -> BallotMeta {
    let motion_a = Motion {
        title: "foo".into(),
        info_link: "www.acme.com".into(),
        choices: vec!["foo".into(), "bar".into(), "baz".into()],
    };
    let motion_b = Motion {
        title: "bar".into(),
        info_link: "www.emca.com".into(),
        choices: vec!["foo".into()],
    };
    BallotMeta {
        title: vec![].into(),
        motions: vec![motion_a, motion_b],
    }
}

fn init_bd(range: BallotTimeRange, meta: BallotMeta) -> BallotData {
    BallotData {
        meta: Some(meta),
        range: Some(range),
        ..<_>::default()
    }
}

#[test]
fn attach_ballot_works() {
    test(|ticker, [owner, ..]| {
        set_schedule_complexity();

        let mut data = init_bd(T_RANGE, mk_meta());
        data.choices = vec![3, 1];

        let id = notice_ca(owner, ticker, Some(1000)).unwrap();
        assert_ok!(attach(owner, id, false));
        assert_ballot(id, &data);
    });
}

#[test]
fn change_end_works() {
    test(|ticker, [owner, ..]| {
        set_schedule_complexity();

        assert_noop!(
            Ballot::change_end(owner.origin(), next_ca_id(ticker), 0),
            BallotError::NoSuchBallot,
        );

        let range = BallotTimeRange {
            start: 2000,
            end: 4000,
        };

        let mut data = init_bd(range, <_>::default());

        let id = notice_ca(owner, ticker, Some(1000)).unwrap();
        assert_ok!(Ballot::attach_ballot(
            owner.origin(),
            id,
            range,
            <_>::default(),
            false
        ));
        assert_ballot(id, &data);

        let mut change = |end| -> DispatchResult {
            Ballot::change_end(owner.origin(), id, end)?;
            data.range = Some(BallotTimeRange { end, ..range });
            assert_ballot(id, &data);
            Ok(())
        };

        Timestamp::set_timestamp(1999);
        assert_ok!(change(5000)); // Not started yet, OK.
        assert_ok!(change(2000)); // start == end, OK.
        assert_noop!(change(1999), BallotError::StartAfterEnd); // end is before start; bad!
        Timestamp::set_timestamp(2000);
        assert_noop!(change(5000), BallotError::VotingAlreadyStarted);
    });
}

#[test]
fn change_rcv_works() {
    test(|ticker, [owner, ..]| {
        for &rcv in &[true, false] {
            set_schedule_complexity();

            let id = notice_ca(owner, ticker, Some(1000)).unwrap();
            let change = |rcv| Ballot::change_rcv(owner.origin(), id, rcv);
            assert_noop!(change(rcv), BallotError::NoSuchBallot);
            assert_ballot(id, &<_>::default());

            let range = BallotTimeRange {
                start: 3000,
                end: 5000,
            };
            let mut data = init_bd(range, <_>::default());
            data.rcv = rcv;

            assert_ok!(Ballot::attach_ballot(
                owner.origin(),
                id,
                range,
                <_>::default(),
                data.rcv
            ));
            assert_ballot(id, &data);

            Timestamp::set_timestamp(2999);
            data.rcv ^= true;
            assert_ok!(change(data.rcv));
            assert_ballot(id, &data);

            Timestamp::set_timestamp(3000);
            assert_noop!(change(!data.rcv), BallotError::VotingAlreadyStarted);
            assert_ballot(id, &data);
        }
    });
}

#[test]
fn change_meta_works() {
    test(|ticker, [owner, ..]| {
        set_schedule_complexity();

        let id = notice_ca(owner, ticker, Some(1000)).unwrap();
        let change = |meta| Ballot::change_meta(owner.origin(), id, meta);

        assert_noop!(change(<_>::default()), BallotError::NoSuchBallot);

        let range = BallotTimeRange {
            start: 4000,
            end: 6000,
        };
        let mut data = init_bd(range, <_>::default());

        assert_ok!(Ballot::attach_ballot(
            owner.origin(),
            id,
            range,
            <_>::default(),
            data.rcv,
        ));
        assert_ballot(id, &data);

        Timestamp::set_timestamp(3999);
        assert_ok!(change(mk_meta()));
        data.meta = Some(mk_meta());
        data.choices = vec![3, 1];
        assert_ballot(id, &data);

        assert_noop!(
            change(overflowing_meta()),
            BallotError::NumberOfChoicesOverflow,
        );

        Timestamp::set_timestamp(4000);
        assert_noop!(change(mk_meta()), BallotError::VotingAlreadyStarted);
        assert_ballot(id, &data);
    });
}

#[test]
fn remove_ballot_works() {
    test(|ticker, [owner, ..]| {
        set_schedule_complexity();

        let id = notice_ca(owner, ticker, Some(1000)).unwrap();
        let remove = || Ballot::remove_ballot(owner.origin(), id);

        assert_noop!(remove(), BallotError::NoSuchBallot);

        let range = BallotTimeRange {
            start: 5000,
            end: 6000,
        };
        let data = init_bd(range, <_>::default());

        assert_ok!(Ballot::attach_ballot(
            owner.origin(),
            id,
            range,
            <_>::default(),
            data.rcv,
        ));
        assert_ballot(id, &data);

        Timestamp::set_timestamp(5000);
        assert_noop!(remove(), BallotError::VotingAlreadyStarted);
        assert_ballot(id, &data);

        Timestamp::set_timestamp(4999);
        assert_ok!(remove());
        assert_ballot(id, &<_>::default());

        assert_noop!(remove(), BallotError::NoSuchBallot);
    });
}

#[test]
fn vote_no_such_ballot() {
    test(|ticker, [.., voter]| {
        assert_noop!(
            Ballot::vote(voter.origin(), next_ca_id(ticker), vec![]),
            BallotError::NoSuchBallot,
        );
    });
}

#[test]
fn vote_wrong_dates() {
    test(|ticker, [owner, _, voter]| {
        set_schedule_complexity();

        let id = notice_ca(owner, ticker, Some(1000)).unwrap();
        let range = BallotTimeRange {
            start: 6000,
            end: 9000,
        };
        assert_ok!(Ballot::attach_ballot(
            owner.origin(),
            id,
            range,
            <_>::default(),
            false,
        ));

        let vote = || Ballot::vote(voter.origin(), id, vec![]);

        Timestamp::set_timestamp(range.start - 1);
        assert_noop!(vote(), BallotError::VotingNotStarted);
        Timestamp::set_timestamp(range.end + 1);
        assert_noop!(vote(), BallotError::VotingAlreadyEnded);
        Timestamp::set_timestamp(range.start);
        assert_ok!(vote());
        Timestamp::set_timestamp(range.end);
        assert_ok!(vote());
    });
}

fn test_not_targeted(
    ca: impl Fn() -> CAId,
    foo: User,
    bar: User,
    action: impl Fn(CAId) -> DispatchResult,
) {
    let change_targets = |id: CAId, treatment, identities| {
        let targets = TargetIdentities {
            treatment,
            identities,
        };
        CorporateActions::mutate(id.ticker, id.local_id, |ca| {
            ca.as_mut().unwrap().targets = targets
        })
    };

    let id = ca();
    change_targets(id, TargetTreatment::Exclude, vec![foo.did]);
    assert_noop!(action(id), Error::NotTargetedByCA);

    let id = ca();
    change_targets(id, TargetTreatment::Include, vec![bar.did]);
    assert_noop!(action(id), Error::NotTargetedByCA);

    let id = ca();
    change_targets(id, TargetTreatment::Include, vec![foo.did]);
    assert_ok!(action(id));

    let id = ca();
    change_targets(id, TargetTreatment::Exclude, vec![bar.did]);
    assert_ok!(action(id));
}

#[test]
fn vote_not_targeted() {
    test(|ticker, [owner, other, voter]| {
        set_schedule_complexity();
        let ca = || {
            Timestamp::set_timestamp(1);
            let id = notice_ca(owner, ticker, Some(1)).unwrap();
            assert_ok!(attach(owner, id, false));
            Timestamp::set_timestamp(T_RANGE.start);
            id
        };
        let vote = |id| Ballot::vote(voter.origin(), id, votes(&[0, 0, 0, 0]));
        test_not_targeted(ca, voter, other, vote);
    });
}

#[test]
fn vote_wrong_count() {
    test(|ticker, [owner, _, voter]| {
        set_schedule_complexity();

        let id = notice_ca(owner, ticker, Some(1)).unwrap();
        assert_ok!(attach(owner, id, false));
        Timestamp::set_timestamp(T_RANGE.start);

        let vote = |count| {
            let votes = iter::repeat(BallotVote::default()).take(count).collect();
            Ballot::vote(voter.origin(), id, votes)
        };

        for &count in &[0, 3, 5, 10] {
            assert_noop!(vote(count), BallotError::WrongVoteCount);
        }

        assert_ok!(vote(4));
    });
}

fn fallbacks(fs: &[Option<u16>]) -> Vec<BallotVote<Balance>> {
    fs.iter()
        .copied()
        .map(|fallback| BallotVote { power: 0, fallback })
        .collect()
}

fn votes(vs: &[Balance]) -> Vec<BallotVote<Balance>> {
    vs.iter()
        .copied()
        .map(|power| BallotVote {
            power,
            fallback: None,
        })
        .collect()
}

#[test]
fn vote_rcv_not_allowed() {
    test(|ticker, [owner, _, voter]| {
        set_schedule_complexity();

        let id = notice_ca(owner, ticker, Some(1)).unwrap();
        assert_ok!(attach(owner, id, false));
        Timestamp::set_timestamp(T_RANGE.start);

        assert_noop!(
            Ballot::vote(voter.origin(), id, fallbacks(&[None, None, Some(42), None])),
            BallotError::RCVNotAllowed,
        );
    });
}

#[test]
fn vote_rcv_fallback_pointers() {
    test(|ticker, [owner, _, voter]| {
        set_schedule_complexity();

        let id = notice_ca(owner, ticker, Some(1)).unwrap();
        assert_ok!(attach(owner, id, true));
        Timestamp::set_timestamp(T_RANGE.start);

        let vote = |fs| Ballot::vote(voter.origin(), id, fallbacks(fs));

        // Self cycle, 0 -> 0 in choice 1.
        assert_noop!(
            vote(&[None, None, None, Some(0)]),
            BallotError::RCVSelfCycle,
        );

        // Self cycle, 1 -> 1 in choice 0.
        assert_noop!(
            vote(&[None, Some(1), None, None]),
            BallotError::RCVSelfCycle,
        );

        // Dangling fallback, 1 (choice 0) -> 1 (choice 1).
        assert_noop!(
            vote(&[None, Some(3), None, None]),
            BallotError::NoSuchRCVFallback,
        );

        // Dangling fallback, 1 (choice 0) -> Non-existent choice.
        assert_noop!(
            vote(&[None, Some(4), None, None]),
            BallotError::NoSuchRCVFallback,
        );

        // OK fallbacks. Graph is:
        //
        //     0 -> 2
        //     ^     \
        //      \    v
        //       --- 1
        //
        let fs = &[Some(2), Some(0), Some(1), None];
        let data = ballot_data(id);
        assert_ok!(vote(fs));
        assert_ballot(
            id,
            &BallotData {
                votes: vec![(voter.did, fallbacks(fs))],
                results: vec![0, 0, 0, 0],
                ..data
            },
        );
    });
}

#[test]
fn vote_works() {
    test(|ticker, [owner, other, voter]| {
        set_schedule_complexity();

        // Total asset balance voter == 500.
        transfer(&ticker, owner, voter);
        transfer(&ticker, owner, other);
        assert_eq!(Asset::balance(&ticker, voter.did), 500);

        let id = notice_ca(owner, ticker, Some(1)).unwrap();
        assert_ok!(attach(owner, id, false));
        Timestamp::set_timestamp(T_RANGE.start);

        let vote = |vs| Ballot::vote(voter.origin(), id, votes(vs));

        let data = ballot_data(id);
        let noop = |vs| {
            assert_noop!(vote(vs), BallotError::InsufficientVotes);
            assert_ballot(id, &data);
        };
        noop(&[501, 501, 501, 501]);
        noop(&[500, 500, 500, 500]);
        noop(&[499, 499, 499, 500]);
        noop(&[499, 499, 499, 499]);
        noop(&[499, 499, 499, 0]);
        noop(&[200, 300, 1, 0]);

        let ok = |vs| {
            assert_ok!(vote(vs));
            assert_ballot(
                id,
                &BallotData {
                    votes: vec![(voter.did, votes(vs))],
                    results: vs.into(),
                    ..data.clone()
                },
            )
        };

        ok(&[200, 300, 0, 0]);
        ok(&[200, 250, 50, 0]);
        ok(&[200, 300, 0, 500]);
        let vs1 = &[200, 250, 50, 500];
        ok(vs1);

        let vs2 = &[500, 0, 0, 250];
        assert_ok!(Ballot::vote(other.origin(), id, votes(vs2)));
        assert_ballot(
            id,
            &BallotData {
                votes: vec![(other.did, votes(vs2)), (voter.did, votes(vs1))],
                results: vs1.iter().zip(vs2).map(|(a, b)| a + b).collect(),
                ..data.clone()
            },
        )
    });
}

fn vote_cp_test(mk_ca: impl FnOnce(Ticker, User) -> CAId) {
    test(|ticker, [owner, other, voter]| {
        set_schedule_complexity();

        // Transfer 500 ==> voter.
        transfer(&ticker, owner, voter);

        let id = mk_ca(ticker, owner);

        // Transfer 500 <== other. N.B. this is after the CP was made.
        Timestamp::set_timestamp(3000);
        transfer(&ticker, voter, other);

        let time = BallotTimeRange {
            start: 4000,
            end: 6000,
        };
        assert_ok!(Ballot::attach_ballot(
            owner.origin(),
            id,
            time,
            mk_meta(),
            false,
        ));

        let vote = |user: User, vs| Ballot::vote(user.origin(), id, votes(vs));

        Timestamp::set_timestamp(4000);
        let data = ballot_data(id);
        assert_noop!(vote(other, &[1, 0, 0, 0]), BallotError::InsufficientVotes);
        assert_ballot(id, &data);

        assert_ok!(vote(voter, &[500, 0, 0, 500]));
    });
}

#[test]
fn vote_existing_checkpoint() {
    vote_cp_test(|ticker, owner| {
        assert_ok!(Checkpoint::create_checkpoint(owner.origin(), ticker));
        let rd = Some(RecordDateSpec::Existing(
            Checkpoint::checkpoint_id_sequence(ticker),
        ));
        let id = notice_ca(owner, ticker, Some(1000)).unwrap();
        assert_ok!(CA::change_record_date(owner.origin(), id, rd));
        id
    });
}

#[test]
fn vote_scheduled_checkpoint() {
    vote_cp_test(|ticker, owner| notice_ca(owner, ticker, Some(2000)).unwrap());
}

fn dist_ca(owner: User, ticker: Ticker, rd: Option<Moment>) -> Result<CAId, DispatchError> {
    let id = next_ca_id(ticker);
    moment_ca(owner, ticker, CAKind::UnpredictableBenefit, rd)?;
    Ok(id)
}

#[test]
fn dist_distribute_works() {
    test(|ticker, [owner, other, _]| {
        set_schedule_complexity();

        let currency = create_asset(b"BETA", owner);

        // Test no CA at id.
        let id = next_ca_id(ticker);
        assert_noop!(
            Dist::distribute(owner.origin(), id, None, currency, 0, 1, None),
            Error::NoSuchCA
        );

        let id = dist_ca(owner, ticker, Some(1)).unwrap();

        // Test same-asset logic.
        assert_noop!(
            Dist::distribute(owner.origin(), id, None, ticker, 0, 0, None),
            DistError::DistributingAsset
        );

        // Test expiry.
        for &(pay, expiry) in &[(5, 5), (6, 5)] {
            assert_noop!(
                Dist::distribute(owner.origin(), id, None, currency, 0, pay, Some(expiry)),
                DistError::ExpiryBeforePayment
            );
        }
        Timestamp::set_timestamp(5);
        assert_ok!(Dist::distribute(
            owner.origin(),
            id,
            None,
            currency,
            0,
            5,
            Some(6)
        ));

        // Start before now.
        assert_noop!(
            Dist::distribute(owner.origin(), id, None, currency, 0, 4, None),
            DistError::NowAfterPayment
        );

        // Distribution already exists.
        assert_noop!(
            Dist::distribute(owner.origin(), id, None, currency, 0, 5, None),
            DistError::AlreadyExists
        );

        // Portfolio doesn't exist.
        let id = dist_ca(owner, ticker, Some(5)).unwrap();
        let num = PortfolioNumber(42);
        assert_noop!(
            Dist::distribute(owner.origin(), id, Some(num), currency, 0, 5, None),
            PError::PortfolioDoesNotExist
        );

        // No custody over portfolio.
        let custody =
            |who: User| Custodian::insert(PortfolioId::default_portfolio(owner.did), who.did);
        let dist = |id| Dist::distribute(owner.origin(), id, None, currency, 0, 6, None);
        custody(other);
        assert_noop!(dist(id), PError::UnauthorizedCustodian);
        custody(owner);

        // Only benefits, no other kinds.
        for &kind in ALL_CA_KINDS {
            let id = next_ca_id(ticker);
            assert_ok!(moment_ca(owner, ticker, kind, Some(5)));
            if kind.is_benefit() {
                assert_ok!(dist(id));
            } else {
                assert_noop!(dist(id), DistError::CANotBenefit);
            }
        }

        // No record date.
        let id = dist_ca(owner, ticker, None).unwrap();
        assert_noop!(dist(id), Error::NoRecordDate);

        // Record date after start.
        let dist = |id, start| Dist::distribute(owner.origin(), id, None, currency, 0, start, None);
        let id = dist_ca(owner, ticker, Some(5000)).unwrap();
        assert_noop!(dist(id, 4999), Error::RecordDateAfterStart);
        assert_ok!(dist(id, 5000));

        // Test sufficient currency balance.
        Agent::insert(ticker, other.did);
        transfer(&currency, owner, other);
        let id = dist_ca(other, ticker, Some(5)).unwrap();
        let dist =
            |amount| Dist::distribute(other.origin(), id, None, currency, amount, 5, Some(13));
        assert_noop!(dist(501), PError::InsufficientPortfolioBalance);
        assert_ok!(dist(500));
        assert_eq!(
            Dist::distributions(id),
            Some(Distribution {
                from: PortfolioId::default_portfolio(other.did),
                currency,
                amount: 500,
                remaining: 500,
                reclaimed: false,
                payment_at: 5,
                expires_at: Some(13),
            })
        )
    });
}

#[test]
fn dist_remove_works() {
    test(|ticker, [owner, ..]| {
        set_schedule_complexity();

        let remove = |id| Dist::remove_distribution(owner.origin(), id);

        // Test no dist at id.
        let id = next_ca_id(ticker);
        assert_noop!(remove(id), DistError::NoSuchDistribution);

        // Already started.
        let id = dist_ca(owner, ticker, Some(1)).unwrap();
        assert_ok!(Dist::distribute(
            owner.origin(),
            id,
            None,
            create_asset(b"BETA", owner),
            0,
            5,
            Some(6)
        ));
        Timestamp::set_timestamp(5);
        assert_noop!(remove(id), DistError::DistributionStarted);

        // Not started, and can remove.
        Timestamp::set_timestamp(4);
        assert_ok!(remove(id));
        assert_eq!(Dist::distributions(id), None);
    });
}

#[test]
fn dist_reclaim_works() {
    test(|ticker, [owner, other, _]| {
        set_schedule_complexity();

        let currency = create_asset(b"BETA", owner);

        let reclaim = |id, who: User| Dist::reclaim(who.origin(), id);

        // Test no dist at id.
        let id = next_ca_id(ticker);
        assert_noop!(reclaim(id, owner), DistError::NoSuchDistribution);

        // Dist creator different from CAA.
        transfer(&currency, owner, other);
        let id = dist_ca(owner, ticker, Some(1)).unwrap();
        Agent::insert(ticker, other.did);
        assert_ok!(Dist::distribute(
            other.origin(),
            id,
            None,
            currency,
            500,
            5,
            Some(6)
        ));
        Agent::insert(ticker, owner.did);
        assert_noop!(reclaim(id, owner), DistError::NotDistributionCreator);

        // Not expired yet.
        Timestamp::set_timestamp(5);
        assert_noop!(reclaim(id, other), DistError::NotExpired);

        // Test successful behavior.
        Agent::insert(ticker, other.did);
        Timestamp::set_timestamp(6);
        let pid = PortfolioId::default_portfolio(other.did);
        let ensure = |x| Portfolio::ensure_sufficient_balance(&pid, &currency, &x);
        assert_noop!(ensure(1), PError::InsufficientPortfolioBalance);
        let dist = Dist::distributions(id).unwrap();
        assert_ok!(reclaim(id, other));
        assert_ok!(ensure(500));
        assert_noop!(ensure(501), PError::InsufficientPortfolioBalance);
        assert_eq!(
            Dist::distributions(id).unwrap(),
            Distribution {
                reclaimed: true,
                remaining: 0,
                ..dist
            }
        );

        // Now that we have reclaimed, we cannot do so again.
        assert_noop!(reclaim(id, other), DistError::AlreadyReclaimed);
    });
}

#[test]
fn dist_claim_misc_bad() {
    test(|ticker, [owner, claimant, _]| {
        set_schedule_complexity();

        // Important for the end. A scope claim exists for `ticker`, but *not* for `BETA`.
        transfer(&ticker, owner, claimant);

        let id = dist_ca(owner, ticker, Some(1)).unwrap();

        let noop = |err: DispatchError| {
            assert_noop!(Dist::claim(claimant.origin(), id), err);
            assert_noop!(Dist::push_benefit(owner.origin(), id, claimant.did), err);
        };

        // Dist doesn't exist yet.
        noop(DistError::NoSuchDistribution.into());

        // Now it does.
        assert_ok!(Dist::distribute(
            owner.origin(),
            id,
            None,
            create_asset(b"BETA", owner),
            0,
            5,
            Some(6)
        ));

        // But it hasn't started yet.
        Timestamp::set_timestamp(4);
        noop(DistError::CannotClaimBeforeStart.into());

        // And now it has already expired.
        Timestamp::set_timestamp(6);
        noop(DistError::CannotClaimAfterExpiry.into());

        // Travel back in time. Now dist is active, but no scope claims, so transfer fails.
        Timestamp::set_timestamp(5);
        noop(AssetError::InvalidTransfer.into());
    });
}

#[test]
fn dist_claim_not_targeted() {
    test(|ticker, [owner, foo, bar]| {
        set_schedule_complexity();

        let currency = create_asset(b"BETA", owner);

        provide_scope_claim_to_multiple_parties(
            &[owner.did, foo.did, bar.did],
            currency,
            CDDP.public(),
        );

        let ca = || {
            let id = dist_ca(owner, ticker, Some(1)).unwrap();
            assert_ok!(Dist::distribute(
                owner.origin(),
                id,
                None,
                currency,
                0,
                1,
                None,
            ));
            id
        };
        test_not_targeted(ca, foo, bar, |id| Dist::claim(foo.origin(), id));
    });
}

#[test]
fn dist_claim_works() {
    test(|ticker, [owner, foo, bar]| {
        set_schedule_complexity();

        // Add scope claims for `BETA`, allowing transfers to go through.
        let currency = create_asset(b"BETA", owner);
        provide_scope_claim_to_multiple_parties(
            &[owner.did, foo.did, bar.did],
            currency,
            CDDP.public(),
        );

        // Transfer 500 to `foo` and 1000 to `bar`.
        transfer(&ticker, owner, foo);
        transfer(&ticker, owner, bar);
        transfer(&ticker, owner, bar);

        // Create the dist.
        let id = dist_ca(owner, ticker, Some(1)).unwrap();
        let amount = 200_000;
        let supply = Asset::total_supply(ticker);
        assert_ok!(Dist::distribute(
            owner.origin(),
            id,
            None,
            currency,
            amount,
            5,
            None,
        ));
        Timestamp::set_timestamp(5);

        // Alter taxes, using both default and DID-specific taxes.
        CorporateActions::mutate(ticker, id.local_id, |ca| {
            let ca = ca.as_mut().unwrap();
            ca.default_withholding_tax = P25;
            ca.withholding_tax = vec![(bar.did, Tax::from_rational_approximation(1u32, 3u32))];
        });

        // Ensures that holder cannot claim or be pushed to again.
        let already = |user: User| {
            assert_noop!(Dist::claim(user.origin(), id), DistError::HolderAlreadyPaid);
            assert_noop!(
                Dist::push_benefit(owner.origin(), id, user.did),
                DistError::HolderAlreadyPaid
            );
        };

        // `foo` claims with 25% tax.
        assert_ok!(Dist::claim(foo.origin(), id));
        already(foo);
        let benefit_foo = 500 * amount / supply;
        let post_tax_foo = benefit_foo * 3 / 4;
        assert_eq!(Asset::balance(&currency, foo.did), post_tax_foo);
        let assert_rem =
            |removed| assert_eq!(Dist::distributions(id).unwrap().remaining, amount - removed);
        assert_rem(benefit_foo);

        // `bar` is pushed to with 1/3 tax.
        assert_ok!(Dist::push_benefit(owner.origin(), id, bar.did));
        already(bar);
        let benefit_bar = 1_000 * amount / supply;
        let post_tax_bar = benefit_bar * 2 / 3; // Using 1/3 tax to test rounding.
        assert_eq!(Asset::balance(&currency, bar.did), post_tax_bar);
        assert_rem(benefit_foo + benefit_bar);

        // Owner should have some free currency balance due to withheld taxes.
        let pid = PortfolioId::default_portfolio(owner.did);
        let wht = benefit_foo - post_tax_foo + benefit_bar - post_tax_bar;
        let rem = supply - amount + wht;
        assert_ok!(Portfolio::ensure_sufficient_balance(&pid, &currency, &rem));
        assert_noop!(
            Portfolio::ensure_sufficient_balance(&pid, &currency, &(rem + 1)),
            PError::InsufficientPortfolioBalance,
        );
    });
}

fn dist_claim_cp_test(mk_ca: impl FnOnce(Ticker, User) -> CAId) {
    test(|ticker, [owner, other, claimant]| {
        set_schedule_complexity();

        // Owner ==[500]==> Voter.
        transfer(&ticker, owner, claimant);

        let id = mk_ca(ticker, owner);

        // Voter ==[500]==> Other. N.B. this is after the CP was made.
        Timestamp::set_timestamp(3000);
        transfer(&ticker, claimant, other);

        // Create the distribution.
        let currency = create_asset(b"BETA", owner);
        provide_scope_claim_to_multiple_parties(
            &[owner.did, other.did, claimant.did],
            currency,
            CDDP.public(),
        );
        let amount = 200_000;
        let supply = Asset::total_supply(ticker);
        assert_ok!(Dist::distribute(
            owner.origin(),
            id,
            None,
            currency,
            amount,
            4000,
            None,
        ));

        // Claim the distribution.
        Timestamp::set_timestamp(4000);
        assert_ok!(Dist::claim(claimant.origin(), id));
        assert_ok!(Dist::push_benefit(owner.origin(), id, other.did));

        // Check the balances; tax is 0%.
        assert_eq!(
            Asset::balance(&currency, claimant.did),
            500 * amount / supply
        );
        assert_eq!(Asset::balance(&currency, other.did), 0);
    });
}

#[test]
fn dist_claim_existing_checkpoint() {
    dist_claim_cp_test(|ticker, owner| {
        assert_ok!(Checkpoint::create_checkpoint(owner.origin(), ticker));
        let rd = Some(RecordDateSpec::Existing(
            Checkpoint::checkpoint_id_sequence(ticker),
        ));
        let id = dist_ca(owner, ticker, Some(1000)).unwrap();
        assert_ok!(CA::change_record_date(owner.origin(), id, rd));
        id
    });
}

#[test]
fn dist_claimscheduled_checkpoint() {
    dist_claim_cp_test(|ticker, owner| dist_ca(owner, ticker, Some(2000)).unwrap());
}
