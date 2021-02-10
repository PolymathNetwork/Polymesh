use super::ext_builder::{
    EXTRINSIC_BASE_WEIGHT, MAX_NO_OF_TM_ALLOWED, NETWORK_FEE_SHARE, TRANSACTION_BYTE_FEE,
    WEIGHT_TO_FEE,
};
use codec::Encode;
use confidential_identity::{compute_cdd_id, compute_scope_id};
use frame_support::{
    assert_ok, impl_outer_dispatch, impl_outer_event, impl_outer_origin, parameter_types,
    traits::{Currency, Imbalance, OnInitialize, OnUnbalanced},
    weights::DispatchInfo,
    weights::{
        RuntimeDbWeight, Weight, WeightToFeeCoefficient, WeightToFeeCoefficients,
        WeightToFeePolynomial,
    },
    StorageDoubleMap,
};
use frame_system::EnsureRoot;
use pallet_asset::{self as asset, checkpoint};
use pallet_balances as balances;
use pallet_bridge as bridge;
use pallet_committee as committee;
use pallet_compliance_manager as compliance_manager;
use pallet_confidential as confidential;
use pallet_corporate_actions as corporate_actions;
use pallet_corporate_actions::ballot as corporate_ballots;
use pallet_corporate_actions::distribution as capital_distributions;
use pallet_group as group;
use pallet_identity as identity;
use pallet_multisig as multisig;
use pallet_pips as pips;
use pallet_portfolio as portfolio;
use pallet_protocol_fee as protocol_fee;
use pallet_settlement as settlement;
use pallet_statistics as statistics;
use pallet_sto as sto;
use pallet_treasury as treasury;
use pallet_utility;
use polymesh_common_utilities::traits::{
    balances::AccountData,
    group::GroupTrait,
    identity::Trait as IdentityTrait,
    transaction_payment::{CddAndFeeDetails, ChargeTxFee},
    CommonTrait, PermissionChecker,
};
use polymesh_common_utilities::Context;
use polymesh_primitives::{
    Authorization, AuthorizationData, CddId, Claim, IdentityId, InvestorUid, InvestorZKProofData,
    Permissions, PortfolioId, PortfolioNumber, Scope, ScopeId, Signatory, Ticker,
};
use polymesh_runtime_common::cdd_check::CddChecker;
use smallvec::smallvec;
use sp_core::{
    crypto::{key_types, Pair as PairTrait},
    sr25519::{Pair, Public},
    H256,
};
use sp_runtime::{
    impl_opaque_keys,
    testing::{Header, UintAuthorityId},
    traits::{BlakeTwo256, ConvertInto, IdentityLookup, OpaqueKeys, Verify},
    transaction_validity::{InvalidTransaction, TransactionValidity, ValidTransaction},
    AnySignature, KeyTypeId, Perbill,
};
use sp_std::{collections::btree_set::BTreeSet, iter};
use std::cell::RefCell;
use std::convert::From;
use test_client::AccountKeyring;

impl_opaque_keys! {
    pub struct MockSessionKeys {
        pub dummy: UintAuthorityId,
    }
}

impl From<UintAuthorityId> for MockSessionKeys {
    fn from(dummy: UintAuthorityId) -> Self {
        Self { dummy }
    }
}

impl_outer_origin! {
    pub enum Origin for TestStorage {
        committee Instance1 <T>,
        committee DefaultInstance <T>,
        committee Instance3 <T>,
        committee Instance4 <T>
    }
}

impl_outer_dispatch! {
    pub enum Call for TestStorage where origin: Origin {
        identity::Identity,
        balances::Balances,
        pips::Pips,
        multisig::MultiSig,
        pallet_contracts::Contracts,
        bridge::Bridge,
        asset::Asset,
        frame_system::System,
        pallet_utility::Utility,
        polymesh_contracts::WrapperContracts,
        self::Committee,
        self::DefaultCommittee,
        pallet_scheduler::Scheduler,
        pallet_settlement::Settlement,
        checkpoint::Checkpoint,
        pallet_portfolio::Portfolio,
    }
}

impl_outer_event! {
    pub enum EventTest for TestStorage {
        identity<T>,
        balances<T>,
        multisig<T>,
        bridge<T>,
        asset<T>,
        pips<T>,
        pallet_contracts<T>,
        pallet_session,
        compliance_manager,
        group Instance1<T>,
        group Instance2<T>,
        group DefaultInstance<T>,
        committee Instance1<T>,
        committee DefaultInstance<T>,
        frame_system<T>,
        protocol_fee<T>,
        treasury<T>,
        settlement<T>,
        sto<T>,
        pallet_utility,
        portfolio<T>,
        confidential,
        polymesh_contracts<T>,
        pallet_scheduler<T>,
        corporate_actions,
        corporate_ballots<T>,
        capital_distributions<T>,
        checkpoint<T>,
        statistics,
    }
}

#[derive(Copy, Clone)]
pub struct User {
    pub ring: AccountKeyring,
    pub did: IdentityId,
}

impl User {
    pub fn new(ring: AccountKeyring) -> Self {
        let did = register_keyring_account(ring).unwrap();
        Self { ring, did }
    }

    pub fn existing(ring: AccountKeyring) -> Self {
        let did = get_identity_id(ring).unwrap();
        User { ring, did }
    }

    pub fn balance(self, balance: u128) -> Self {
        use frame_support::traits::Currency as _;
        Balances::make_free_balance_be(&self.acc(), balance);
        self
    }

    pub fn acc(&self) -> Public {
        self.ring.public()
    }

    pub fn origin(&self) -> Origin {
        Origin::signed(self.acc())
    }
}

// For testing the module, we construct most of a mock runtime. This means
// first constructing a configuration type (`Test`) which `impl`s each of the
// configuration traits of modules we want to use.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct TestStorage;

pub type AccountId = <AnySignature as Verify>::Signer;

type Index = u64;
type BlockNumber = u64;
type Hash = H256;
type Hashing = BlakeTwo256;
type Lookup = IdentityLookup<AccountId>;
type OffChainSignature = AnySignature;
type SessionIndex = u32;
type AuthorityId = <AnySignature as Verify>::Signer;
type Event = EventTest;
type Version = ();
crate type Balance = u128;

parameter_types! {
    pub const BlockHashCount: u32 = 250;
    pub const MaximumBlockWeight: u64 = 4096;
    pub const MaximumBlockLength: u32 = 4096;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const MaximumExtrinsicWeight: u64 = 2800;
    pub const BlockExecutionWeight: u64 = 10;
    pub TransactionByteFee: Balance = TRANSACTION_BYTE_FEE.with(|v| *v.borrow());
    pub ExtrinsicBaseWeight: u64 = EXTRINSIC_BASE_WEIGHT.with(|v| *v.borrow());
    pub const DbWeight: RuntimeDbWeight = RuntimeDbWeight {
        read: 10,
        write: 100,
    };
    pub FeeCollector: AccountId = account_from(5000);
}

pub type NegativeImbalance<T> =
    <balances::Module<T> as Currency<<T as frame_system::Trait>::AccountId>>::NegativeImbalance;

pub struct DealWithFees<T>(sp_std::marker::PhantomData<T>);

impl OnUnbalanced<NegativeImbalance<TestStorage>> for DealWithFees<TestStorage> {
    fn on_nonzero_unbalanced(amount: NegativeImbalance<TestStorage>) {
        let target = account_from(5000);
        let positive_imbalance = Balances::deposit_creating(&target, amount.peek());
        let _ = amount.offset(positive_imbalance).map_err(|_| 4); // random value mapped for error
    }
}

impl frame_system::Trait for TestStorage {
    /// The basic call filter to use in dispatchable.
    type BaseCallFilter = ();
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The aggregated dispatch type that is available for extrinsics.
    type Call = Call;
    /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
    type Lookup = Lookup;
    /// The index type for storing how many extrinsics an account has signed.
    type Index = Index;
    /// The index type for blocks.
    type BlockNumber = BlockNumber;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// The hashing algorithm used.
    type Hashing = Hashing;
    /// The header type.
    type Header = Header;
    /// The ubiquitous event type.
    type Event = Event;
    /// The ubiquitous origin type.
    type Origin = Origin;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// Maximum weight of each block.
    type MaximumBlockWeight = MaximumBlockWeight;
    /// Maximum size of all encoded transactions (in bytes) that are allowed in one block.
    type MaximumBlockLength = MaximumBlockLength;
    /// Portion of the block weight that is available to all normal transactions.
    type AvailableBlockRatio = AvailableBlockRatio;
    /// Version of the runtime.
    type Version = Version;
    /// Converts a module to the index of the module in `construct_runtime!`.
    ///
    /// This type is being generated by `construct_runtime!`.
    type PalletInfo = ();
    /// What to do if a new account is created.
    type OnNewAccount = ();
    /// What to do if an account is fully reaped from the system.
    type OnKilledAccount = ();
    /// The data to be stored in an account.
    type AccountData = AccountData<<TestStorage as CommonTrait>::Balance>;
    /// The weight of database operations that the runtime can invoke.
    type DbWeight = DbWeight;
    /// The weight of the overhead invoked on the block import process, independent of the
    /// extrinsics included in that block.
    type BlockExecutionWeight = BlockExecutionWeight;
    /// The base weight of any extrinsic processed by the runtime, independent of the
    /// logic of that extrinsic. (Signature verification, nonce increment, fee, etc...)
    type ExtrinsicBaseWeight = ExtrinsicBaseWeight;
    /// The maximum weight that a single extrinsic of `Normal` dispatch class can have,
    /// independent of the logic of that extrinsics. (Roughly max block weight - average on
    /// initialize cost).
    type MaximumExtrinsicWeight = MaximumExtrinsicWeight;
    type SystemWeightInfo = ();
}

parameter_types! {
    pub const ExistentialDeposit: u64 = 0;
    pub const MaxLocks: u32 = 50;
}

impl CommonTrait for TestStorage {
    type Balance = Balance;
    type AssetSubTraitTarget = Asset;
    type BlockRewardsReserve = balances::Module<TestStorage>;
}

impl balances::Trait for TestStorage {
    type DustRemoval = ();
    type Event = Event;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = frame_system::Module<TestStorage>;
    type CddChecker = CddChecker<Self>;
    type WeightInfo = polymesh_weights::pallet_balances::WeightInfo;
    type MaxLocks = MaxLocks;
}

parameter_types! {
    pub const MinimumPeriod: u64 = 3;
}

impl pallet_timestamp::Trait for TestStorage {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

parameter_types! {
    pub NetworkShareInFee: Perbill = NETWORK_FEE_SHARE.with(|v| *v.borrow());
}

impl polymesh_contracts::Trait for TestStorage {
    type Event = Event;
    type NetworkShareInFee = NetworkShareInFee;
    type WeightInfo = polymesh_weights::polymesh_contracts::WeightInfo;
}

impl multisig::Trait for TestStorage {
    type Event = Event;
    type Scheduler = Scheduler;
    type SchedulerCall = Call;
    type WeightInfo = polymesh_weights::pallet_multisig::WeightInfo;
}

impl settlement::Trait for TestStorage {
    type Event = Event;
    type Scheduler = Scheduler;
    type SchedulerCall = Call;
    type WeightInfo = polymesh_weights::pallet_settlement::WeightInfo;
}

impl sto::Trait for TestStorage {
    type Event = Event;
    type WeightInfo = polymesh_weights::pallet_sto::WeightInfo;
}

impl ChargeTxFee for TestStorage {
    fn charge_fee(_len: u32, _info: DispatchInfo) -> TransactionValidity {
        Ok(ValidTransaction::default())
    }
}

impl CddAndFeeDetails<AccountId, Call> for TestStorage {
    fn get_valid_payer(
        _: &Call,
        caller: &AccountId,
    ) -> Result<Option<AccountId>, InvalidTransaction> {
        Ok(Some(*caller))
    }
    fn clear_context() {
        Context::set_current_identity::<Identity>(None);
        Context::set_current_payer::<Identity>(None);
    }
    fn set_payer_context(payer: Option<AccountId>) {
        Context::set_current_payer::<Identity>(payer);
    }
    fn get_payer_from_context() -> Option<AccountId> {
        Context::current_payer::<Identity>()
    }
    fn set_current_identity(did: &IdentityId) {
        Context::set_current_identity::<Identity>(Some(*did));
    }
}

pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
    type Balance = Balance;

    fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
        smallvec![WeightToFeeCoefficient {
            degree: 1,
            coeff_frac: Perbill::zero(),
            coeff_integer: WEIGHT_TO_FEE.with(|v| *v.borrow()),
            negative: false,
        }]
    }
}

impl pallet_transaction_payment::Trait for TestStorage {
    type Currency = Balances;
    type OnTransactionPayment = ();
    type TransactionByteFee = TransactionByteFee;
    type WeightToFee = WeightToFee;
    type FeeMultiplierUpdate = ();
    type CddHandler = TestStorage;
}

impl group::Trait<group::DefaultInstance> for TestStorage {
    type Event = Event;
    type LimitOrigin = EnsureRoot<AccountId>;
    type AddOrigin = EnsureRoot<AccountId>;
    type RemoveOrigin = EnsureRoot<AccountId>;
    type SwapOrigin = EnsureRoot<AccountId>;
    type ResetOrigin = EnsureRoot<AccountId>;
    type MembershipInitialized = committee::Module<TestStorage, committee::Instance1>;
    type MembershipChanged = committee::Module<TestStorage, committee::Instance1>;
    type WeightInfo = polymesh_weights::pallet_group::WeightInfo;
}

/// PolymeshCommittee as an instance of group
impl group::Trait<group::Instance1> for TestStorage {
    type Event = Event;
    type LimitOrigin = EnsureRoot<AccountId>;
    type AddOrigin = EnsureRoot<AccountId>;
    type RemoveOrigin = EnsureRoot<AccountId>;
    type SwapOrigin = EnsureRoot<AccountId>;
    type ResetOrigin = EnsureRoot<AccountId>;
    type MembershipInitialized = committee::Module<TestStorage, committee::Instance1>;
    type MembershipChanged = committee::Module<TestStorage, committee::Instance1>;
    type WeightInfo = polymesh_weights::pallet_group::WeightInfo;
}

impl group::Trait<group::Instance2> for TestStorage {
    type Event = Event;
    type LimitOrigin = EnsureRoot<AccountId>;
    type AddOrigin = EnsureRoot<AccountId>;
    type RemoveOrigin = EnsureRoot<AccountId>;
    type SwapOrigin = EnsureRoot<AccountId>;
    type ResetOrigin = EnsureRoot<AccountId>;
    type MembershipInitialized = identity::Module<TestStorage>;
    type MembershipChanged = identity::Module<TestStorage>;
    type WeightInfo = polymesh_weights::pallet_group::WeightInfo;
}

pub type CommitteeOrigin<T, I> = committee::RawOrigin<<T as frame_system::Trait>::AccountId, I>;

/// Voting majority origin for `Instance`.
type VMO<Instance> = committee::EnsureThresholdMet<AccountId, Instance>;

impl committee::Trait<committee::Instance1> for TestStorage {
    type Origin = Origin;
    type Proposal = Call;
    type CommitteeOrigin = VMO<committee::Instance1>;
    type VoteThresholdOrigin = Self::CommitteeOrigin;
    type Event = Event;
    type WeightInfo = polymesh_weights::pallet_committee::WeightInfo;
}

impl committee::Trait<committee::DefaultInstance> for TestStorage {
    type Origin = Origin;
    type Proposal = Call;
    type CommitteeOrigin = EnsureRoot<AccountId>;
    type VoteThresholdOrigin = Self::CommitteeOrigin;
    type Event = Event;
    type WeightInfo = polymesh_weights::pallet_committee::WeightInfo;
}

impl IdentityTrait for TestStorage {
    type Event = Event;
    type Proposal = Call;
    type MultiSig = multisig::Module<TestStorage>;
    type Portfolio = portfolio::Module<TestStorage>;
    type CddServiceProviders = CddServiceProvider;
    type Balances = balances::Module<TestStorage>;
    type ChargeTxFeeTarget = TestStorage;
    type CddHandler = TestStorage;
    type Public = AccountId;
    type OffChainSignature = OffChainSignature;
    type ProtocolFee = protocol_fee::Module<TestStorage>;
    type GCVotingMajorityOrigin = VMO<committee::Instance1>;
    type WeightInfo = polymesh_weights::pallet_identity::WeightInfo;
    type CorporateAction = CorporateActions;
    type IdentityFn = identity::Module<TestStorage>;
    type SchedulerOrigin = OriginCaller;
}

parameter_types! {
    pub const SignedClaimHandicap: u64 = 2;
    pub const StorageSizeOffset: u32 = 8;
    pub const TombstoneDeposit: Balance = 16;
    pub const RentByteFee: Balance = 100;
    pub const RentDepositOffset: Balance = 100000;
    pub const SurchargeReward: Balance = 1500;
    pub const MaxDepth: u32 = 100;
    pub const MaxValueSize: u32 = 16_384;
}

impl pallet_contracts::Trait for TestStorage {
    type Time = Timestamp;
    type Randomness = Randomness;
    type Currency = Balances;
    type Event = Event;
    type DetermineContractAddress = polymesh_contracts::NonceBasedAddressDeterminer<TestStorage>;
    type TrieIdGenerator = pallet_contracts::TrieIdFromParentCounter<TestStorage>;
    type RentPayment = ();
    type SignedClaimHandicap = SignedClaimHandicap;
    type TombstoneDeposit = TombstoneDeposit;
    type StorageSizeOffset = StorageSizeOffset;
    type RentByteFee = RentByteFee;
    type RentDepositOffset = RentDepositOffset;
    type SurchargeReward = SurchargeReward;
    type MaxDepth = MaxDepth;
    type MaxValueSize = MaxValueSize;
    type WeightPrice = pallet_transaction_payment::Module<Self>;
}

parameter_types! {
    pub const MaxTransferManagersPerAsset: u32 = 3;
}
impl statistics::Trait for TestStorage {
    type Event = Event;
    type Asset = Asset;
    type MaxTransferManagersPerAsset = MaxTransferManagersPerAsset;
    type WeightInfo = polymesh_weights::pallet_statistics::WeightInfo;
}

parameter_types! {
    pub const MaxConditionComplexity: u32 = 50;
    pub const MaxDefaultTrustedClaimIssuers: usize = 10;
    pub const MaxTrustedIssuerPerCondition: usize = 10;
    pub const MaxSenderConditionsPerCompliance: usize = 30;
    pub const MaxReceiverConditionsPerCompliance: usize = 30;
    pub const MaxCompliancePerRequirement: usize = 10;

}
impl compliance_manager::Trait for TestStorage {
    type Event = Event;
    type Asset = Asset;
    type WeightInfo = polymesh_weights::pallet_compliance_manager::WeightInfo;
    type MaxConditionComplexity = MaxConditionComplexity;
}

impl protocol_fee::Trait for TestStorage {
    type Event = Event;
    type Currency = Balances;
    type OnProtocolFeePayment = DealWithFees<TestStorage>;
    type WeightInfo = polymesh_weights::pallet_protocol_fee::WeightInfo;
}

impl portfolio::Trait for TestStorage {
    type Event = Event;
    type WeightInfo = polymesh_weights::pallet_portfolio::WeightInfo;
}

parameter_types! {
    pub MaxNumberOfTMExtensionForAsset: u32 = MAX_NO_OF_TM_ALLOWED.with(|v| *v.borrow());
    pub const AssetNameMaxLength: usize = 128;
    pub const FundingRoundNameMaxLength: usize = 128;
    pub const AllowedGasLimit: u64 = 13_000_000_000;
}

impl asset::Trait for TestStorage {
    type Event = Event;
    type Currency = balances::Module<TestStorage>;
    type ComplianceManager = compliance_manager::Module<TestStorage>;
    type MaxNumberOfTMExtensionForAsset = MaxNumberOfTMExtensionForAsset;
    type UnixTime = Timestamp;
    type AssetNameMaxLength = AssetNameMaxLength;
    type FundingRoundNameMaxLength = FundingRoundNameMaxLength;
    type AssetFn = Asset;
    type AllowedGasLimit = AllowedGasLimit;
    type WeightInfo = polymesh_weights::pallet_asset::WeightInfo;
    type CPWeightInfo = polymesh_weights::pallet_checkpoint::WeightInfo;
}

parameter_types! {
    pub const BlockRangeForTimelock: BlockNumber = 1000;
    pub const MaxTargetIds: u32 = 10;
    pub const MaxDidWhts: u32 = 10;
}

impl bridge::Trait for TestStorage {
    type Event = Event;
    type Proposal = Call;
    type Scheduler = Scheduler;
}

impl corporate_actions::Trait for TestStorage {
    type Event = Event;
    type MaxTargetIds = MaxTargetIds;
    type MaxDidWhts = MaxDidWhts;
    type WeightInfo = polymesh_weights::pallet_corporate_actions::WeightInfo;
    type BallotWeightInfo = polymesh_weights::pallet_corporate_ballot::WeightInfo;
    type DistWeightInfo = polymesh_weights::pallet_capital_distribution::WeightInfo;
}

impl treasury::Trait for TestStorage {
    type Event = Event;
    type Currency = Balances;
    type WeightInfo = polymesh_weights::pallet_treasury::WeightInfo;
}

thread_local! {
    pub static FORCE_SESSION_END: RefCell<bool> = RefCell::new(false);
    pub static SESSION_LENGTH: RefCell<u64> = RefCell::new(2);
}

pub struct TestSessionHandler;
impl pallet_session::SessionHandler<AuthorityId> for TestSessionHandler {
    const KEY_TYPE_IDS: &'static [KeyTypeId] = &[key_types::DUMMY];

    fn on_new_session<Ks: OpaqueKeys>(
        _changed: bool,
        _validators: &[(AuthorityId, Ks)],
        _queued_validators: &[(AuthorityId, Ks)],
    ) {
    }

    fn on_disabled(_validator_index: usize) {}

    fn on_genesis_session<Ks: OpaqueKeys>(_validators: &[(AuthorityId, Ks)]) {}
    fn on_before_session_ending() {}
}

pub struct TestShouldEndSession;
impl pallet_session::ShouldEndSession<BlockNumber> for TestShouldEndSession {
    fn should_end_session(now: BlockNumber) -> bool {
        let l = SESSION_LENGTH.with(|l| *l.borrow());
        now % l == 0
            || FORCE_SESSION_END.with(|l| {
                let r = *l.borrow();
                *l.borrow_mut() = false;
                r
            })
    }
}

pub struct TestSessionManager;
impl pallet_session::SessionManager<AccountId> for TestSessionManager {
    fn end_session(_: SessionIndex) {}
    fn start_session(_: SessionIndex) {}
    fn new_session(_: SessionIndex) -> Option<Vec<AccountId>> {
        None
    }
}

parameter_types! {
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(33);
}

impl pallet_session::Trait for TestStorage {
    type Event = Event;
    type ValidatorId = AccountId;
    type ValidatorIdOf = ConvertInto;
    type ShouldEndSession = TestShouldEndSession;
    type NextSessionRotation = ();
    type SessionManager = TestSessionManager;
    type SessionHandler = TestSessionHandler;
    type Keys = MockSessionKeys;
    type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
    type WeightInfo = ();
}

impl pips::Trait for TestStorage {
    type Currency = balances::Module<Self>;
    type VotingMajorityOrigin = VMO<committee::Instance1>;
    type GovernanceCommittee = Committee;
    type TechnicalCommitteeVMO = VMO<committee::Instance3>;
    type UpgradeCommitteeVMO = VMO<committee::Instance4>;
    type Treasury = treasury::Module<Self>;
    type Event = Event;
    type WeightInfo = polymesh_weights::pallet_pips::WeightInfo;
    type Scheduler = Scheduler;
    type SchedulerCall = Call;
}

impl confidential::Trait for TestStorage {
    type Event = Event;
    type Asset = Asset;
    type WeightInfo = polymesh_weights::pallet_confidential::WeightInfo;
}

impl pallet_utility::Trait for TestStorage {
    type Event = Event;
    type Call = Call;
    type WeightInfo = polymesh_weights::pallet_utility::WeightInfo;
}

impl PermissionChecker for TestStorage {
    type Checker = Identity;
}

parameter_types! {
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * MaximumBlockWeight::get();
    pub const MaxScheduledPerBlock: u32 = 50;
}

impl pallet_scheduler::Trait for TestStorage {
    type Event = Event;
    type Origin = Origin;
    type PalletsOrigin = OriginCaller;
    type Call = Call;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = MaxScheduledPerBlock;
    type WeightInfo = ();
}

// Publish type alias for each module
pub type Identity = identity::Module<TestStorage>;
pub type Pips = pips::Module<TestStorage>;
pub type Balances = balances::Module<TestStorage>;
pub type Asset = asset::Module<TestStorage>;
pub type Checkpoint = checkpoint::Module<TestStorage>;
pub type MultiSig = multisig::Module<TestStorage>;
pub type Randomness = pallet_randomness_collective_flip::Module<TestStorage>;
pub type Timestamp = pallet_timestamp::Module<TestStorage>;
pub type Contracts = pallet_contracts::Module<TestStorage>;
pub type Bridge = bridge::Module<TestStorage>;
pub type GovernanceCommittee = group::Module<TestStorage, group::Instance1>;
pub type CddServiceProvider = group::Module<TestStorage, group::Instance2>;
pub type Committee = committee::Module<TestStorage, committee::Instance1>;
pub type DefaultCommittee = committee::Module<TestStorage, committee::DefaultInstance>;
pub type Utility = pallet_utility::Module<TestStorage>;
pub type System = frame_system::Module<TestStorage>;
pub type Portfolio = portfolio::Module<TestStorage>;
pub type WrapperContracts = polymesh_contracts::Module<TestStorage>;
pub type ComplianceManager = compliance_manager::Module<TestStorage>;
pub type CorporateActions = corporate_actions::Module<TestStorage>;
pub type Scheduler = pallet_scheduler::Module<TestStorage>;
pub type Settlement = pallet_settlement::Module<TestStorage>;

pub fn make_account(
    id: AccountId,
) -> Result<(<TestStorage as frame_system::Trait>::Origin, IdentityId), &'static str> {
    let uid = InvestorUid::from(format!("{}", id).as_str());
    make_account_with_uid(id, uid)
}

pub fn make_account_with_portfolio(
    id: AccountId,
) -> (
    <TestStorage as frame_system::Trait>::Origin,
    IdentityId,
    PortfolioId,
) {
    let (origin, did) = make_account(id).unwrap();
    let portfolio = PortfolioId::default_portfolio(did);
    (origin, did, portfolio)
}

pub fn make_account_with_scope(
    id: AccountId,
    ticker: Ticker,
    cdd_provider: AccountId,
) -> Result<
    (
        <TestStorage as frame_system::Trait>::Origin,
        IdentityId,
        ScopeId,
    ),
    &'static str,
> {
    let uid = create_investor_uid(id);
    let (origin, did) = make_account_with_uid(id, uid.clone()).unwrap();
    let scope_id = provide_scope_claim(did, ticker, uid, cdd_provider);
    Ok((origin, did, scope_id))
}

pub fn make_account_with_uid(
    id: AccountId,
    uid: InvestorUid,
) -> Result<(<TestStorage as frame_system::Trait>::Origin, IdentityId), &'static str> {
    make_account_with_balance(id, uid, 1_000_000)
}

/// It creates an Account and registers its DID and its InvestorUid.
pub fn make_account_with_balance(
    id: AccountId,
    uid: InvestorUid,
    balance: <TestStorage as CommonTrait>::Balance,
) -> Result<(<TestStorage as frame_system::Trait>::Origin, IdentityId), &'static str> {
    let signed_id = Origin::signed(id.clone());
    Balances::make_free_balance_be(&id, balance);

    // If we have CDD providers, first of them executes the registration.
    let cdd_providers = CddServiceProvider::get_members();
    let did = match cdd_providers.into_iter().nth(0) {
        Some(cdd_provider) => {
            let cdd_acc = Public::from_raw(Identity::did_records(&cdd_provider).primary_key.0);
            let _ = Identity::cdd_register_did(Origin::signed(cdd_acc), id, vec![])
                .map_err(|_| "CDD register DID failed")?;

            // Add CDD Claim
            let did = Identity::get_identity(&id).unwrap();
            let (cdd_id, _) = create_cdd_id(did, Ticker::default(), uid);
            let cdd_claim = Claim::CustomerDueDiligence(cdd_id);
            Identity::add_claim(Origin::signed(cdd_acc), did, cdd_claim, None)
                .map_err(|_| "CDD provider cannot add the CDD claim")?;
            did
        }
        _ => {
            let _ = Identity::register_did(signed_id.clone(), uid, vec![])
                .map_err(|_| "Register DID failed")?;
            Identity::get_identity(&id).unwrap()
        }
    };

    Ok((signed_id, did))
}

pub fn make_account_without_cdd(
    id: AccountId,
) -> Result<(<TestStorage as frame_system::Trait>::Origin, IdentityId), &'static str> {
    let signed_id = Origin::signed(id.clone());
    Balances::make_free_balance_be(&id, 10_000_000);
    let did = Identity::_register_did(id.clone(), vec![], None).expect("did");
    Ok((signed_id, did))
}

pub fn register_keyring_account(acc: AccountKeyring) -> Result<IdentityId, &'static str> {
    register_keyring_account_with_balance(acc, 10_000_000)
}

pub fn register_keyring_account_with_balance(
    acc: AccountKeyring,
    balance: <TestStorage as CommonTrait>::Balance,
) -> Result<IdentityId, &'static str> {
    let acc_pub = acc.public();
    let uid = create_investor_uid(acc_pub.clone());
    make_account_with_balance(acc_pub, uid, balance).map(|(_, id)| id)
}

pub fn register_keyring_account_without_cdd(
    acc: AccountKeyring,
) -> Result<IdentityId, &'static str> {
    let acc_pub = acc.public();
    make_account_without_cdd(acc_pub).map(|(_, id)| id)
}

pub fn add_secondary_key(did: IdentityId, signer: Signatory<AccountId>) {
    let _primary_key = Identity::did_records(&did).primary_key;
    let auth_id = Identity::add_auth(
        did.clone(),
        signer,
        AuthorizationData::JoinIdentity(Permissions::default()),
        None,
    );
    assert_ok!(Identity::join_identity(signer, auth_id));
}

pub fn account_from(id: u64) -> AccountId {
    let mut enc_id_vec = id.encode();
    enc_id_vec.resize_with(32, Default::default);

    let mut enc_id = [0u8; 32];
    enc_id.copy_from_slice(enc_id_vec.as_slice());

    Pair::from_seed(&enc_id).public()
}

pub fn get_identity_id(acc: AccountKeyring) -> Option<IdentityId> {
    let key = acc.public();
    Identity::get_identity(&key)
}

pub fn authorizations_to(to: &Signatory<AccountId>) -> Vec<Authorization<AccountId, u64>> {
    identity::Authorizations::<TestStorage>::iter_prefix_values(to).collect::<Vec<_>>()
}

pub fn fast_forward_to_block(n: u64) {
    let next_block = System::block_number() + 1;
    (next_block..=n).for_each(|block| {
        System::set_block_number(block);
        Scheduler::on_initialize(block);
    });
}

pub fn fast_forward_blocks(n: u64) {
    fast_forward_to_block(n + System::block_number());
}

// `iter_prefix_values` has no guarantee that it will iterate in a sequential
// order. However, we need the latest `auth_id`. Which is why we search for the claim
// with the highest `auth_id`.
pub fn get_last_auth(signatory: &Signatory<AccountId>) -> Authorization<AccountId, u64> {
    <identity::Authorizations<TestStorage>>::iter_prefix_values(signatory)
        .into_iter()
        .max_by_key(|x| x.auth_id)
        .expect("there are no authorizations")
}

pub fn get_last_auth_id(signatory: &Signatory<AccountId>) -> u64 {
    get_last_auth(signatory).auth_id
}

/// Returns a btreeset that contains default portfolio for the identity.
pub fn default_portfolio_btreeset(did: IdentityId) -> BTreeSet<PortfolioId> {
    iter::once(PortfolioId::default_portfolio(did)).collect::<BTreeSet<_>>()
}

/// Returns a vector that contains default portfolio for the identity.
pub fn default_portfolio_vec(did: IdentityId) -> Vec<PortfolioId> {
    vec![PortfolioId::default_portfolio(did)]
}

/// Returns a btreeset that contains a portfolio for the identity.
pub fn user_portfolio_btreeset(did: IdentityId, num: PortfolioNumber) -> BTreeSet<PortfolioId> {
    iter::once(PortfolioId::user_portfolio(did, num)).collect::<BTreeSet<_>>()
}

/// Returns a vector that contains a portfolio for the identity.
pub fn user_portfolio_vec(did: IdentityId, num: PortfolioNumber) -> Vec<PortfolioId> {
    vec![PortfolioId::user_portfolio(did, num)]
}

pub fn create_cdd_id(
    claim_to: IdentityId,
    scope: Ticker,
    investor_uid: InvestorUid,
) -> (CddId, InvestorZKProofData) {
    let proof: InvestorZKProofData = InvestorZKProofData::new(&claim_to, &investor_uid, &scope);
    let cdd_claim = InvestorZKProofData::make_cdd_claim(&claim_to, &investor_uid);
    (
        compute_cdd_id(&cdd_claim).compress().to_bytes().into(),
        proof,
    )
}

pub fn create_investor_uid(acc: AccountId) -> InvestorUid {
    InvestorUid::from(format!("{}", acc).as_str())
}

pub fn provide_scope_claim(
    claim_to: IdentityId,
    scope: Ticker,
    investor_uid: InvestorUid,
    cdd_provider: AccountId,
) -> ScopeId {
    let (cdd_id, proof) = create_cdd_id(claim_to, scope, investor_uid);
    let scope_claim = InvestorZKProofData::make_scope_claim(&scope.as_slice(), &investor_uid);
    let scope_id = compute_scope_id(&scope_claim).compress().to_bytes().into();

    let signed_claim_to = Origin::signed(Identity::did_records(claim_to).primary_key);

    // Add cdd claim first
    assert_ok!(Identity::add_claim(
        Origin::signed(cdd_provider),
        claim_to,
        Claim::CustomerDueDiligence(cdd_id),
        None
    ));

    // Provide the InvestorUniqueness.
    assert_ok!(Identity::add_investor_uniqueness_claim(
        signed_claim_to,
        claim_to,
        Claim::InvestorUniqueness(Scope::Ticker(scope), scope_id, cdd_id),
        proof,
        None
    ));

    scope_id
}

pub fn provide_scope_claim_to_multiple_parties(
    parties: &[IdentityId],
    ticker: Ticker,
    cdd_provider: AccountId,
) {
    parties.iter().enumerate().for_each(|(_, id)| {
        let uid = create_investor_uid(Identity::did_records(id).primary_key);
        provide_scope_claim(*id, ticker, uid, cdd_provider);
    });
}

pub fn root() -> Origin {
    Origin::from(frame_system::RawOrigin::Root)
}

pub fn create_cdd_id_and_investor_uid(identity_id: IdentityId) -> (CddId, InvestorUid) {
    let uid = create_investor_uid(Identity::did_records(identity_id).primary_key);
    let (cdd_id, _) = create_cdd_id(identity_id, Ticker::default(), uid);
    (cdd_id, uid)
}

pub fn make_remark_proposal() -> Call {
    Call::System(frame_system::Call::remark(vec![b'X'; 100])).into()
}

#[macro_export]
macro_rules! assert_last_event {
    ($event:pat) => {
        assert_last_event!($event, true);
    };
    ($event:pat, $cond:expr) => {
        assert!(matches!(
            &*System::events(),
            [.., EventRecord {
                event: $event,
                ..
            }]
            if $cond
        ));
    };
}

#[macro_export]
macro_rules! assert_event_exists {
    ($event:pat) => {
        assert_event_exists!($event, true);
    };
    ($event:pat, $cond:expr) => {
        assert!(System::events().iter().any(|e| {
            matches!(
                e,
                EventRecord {
                    event: $event,
                    ..
                }
                if $cond
            )
        }));
    };
}

#[macro_export]
macro_rules! assert_event_doesnt_exist {
    ($event:pat) => {
        assert_event_doesnt_exist!($event, true);
    };
    ($event:pat, $cond:expr) => {
        assert!(System::events().iter().all(|e| {
            !matches!(
                e,
                EventRecord {
                    event: $event,
                    ..
                }
                if $cond
            )
        }));
    };
}
