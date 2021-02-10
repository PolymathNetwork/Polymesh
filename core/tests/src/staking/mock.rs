// This file is part of Substrate.

// Copyright (C) 2018-2020 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Test utilities
use crate::storage::create_cdd_id;
use chrono::prelude::Utc;
use frame_support::traits::KeyOwnerProofSystem;
use frame_support::{
    assert_ok,
    dispatch::DispatchResult,
    impl_outer_dispatch, impl_outer_event, impl_outer_origin, parameter_types,
    traits::{
        Contains, Currency, FindAuthor, Get, Imbalance, OnFinalize, OnInitialize, OnUnbalanced,
    },
    weights::{constants::RocksDbWeight, DispatchInfo, Weight},
    IterableStorageMap, StorageDoubleMap, StorageMap, StorageValue,
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use pallet_group as group;
use pallet_identity as identity;
use pallet_protocol_fee as protocol_fee;
use pallet_staking::{self as staking, *};
use polymesh_common_utilities::{
    constants::currency::POLY,
    traits::{
        asset::AssetSubTrait,
        balances::{AccountData, CheckCdd},
        group::{GroupTrait, InactiveMember},
        identity::{IdentityToCorporateAction, Trait as IdentityTrait},
        multisig::MultiSigSubTrait,
        portfolio::PortfolioSubTrait,
        transaction_payment::{CddAndFeeDetails, ChargeTxFee},
        CommonTrait, PermissionChecker,
    },
};
use polymesh_primitives::{
    Authorization, AuthorizationData, CddId, Claim, IdentityId, InvestorUid, Moment, Permissions,
    PortfolioId, ScopeId, SecondaryKey, Signatory, Ticker,
};
use sp_core::H256;
use sp_npos_elections::{
    build_support_map, evaluate_support, reduce, ElectionScore, ExtendedBalance, StakedAssignment,
    VoteWeight,
};
use sp_runtime::{
    curve::PiecewiseLinear,
    testing::{Header, TestSignature, TestXt, UintAuthorityId},
    traits::{Convert, IdentityLookup, SaturatedConversion, Zero},
    transaction_validity::{InvalidTransaction, TransactionValidity, ValidTransaction},
    KeyTypeId, Perbill, Permill,
};
use sp_staking::{
    offence::{OffenceDetails, OnOffenceHandler},
    SessionIndex,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashSet},
};

pub const INIT_TIMESTAMP: u64 = 30_000;

/// The AccountId alias in this test module.
pub(crate) type AccountId = u64;
pub(crate) type AccountIndex = u64;
pub(crate) type BlockNumber = u64;
pub(crate) type Balance = u128;

/// Simple structure that exposes how u64 currency can be represented as... u64.
pub struct CurrencyToVoteHandler;
impl Convert<Balance, u64> for CurrencyToVoteHandler {
    fn convert(x: Balance) -> u64 {
        x.saturated_into()
    }
}
impl Convert<u128, Balance> for CurrencyToVoteHandler {
    fn convert(x: u128) -> Balance {
        x
    }
}

thread_local! {
    static SESSION: RefCell<(Vec<AccountId>, HashSet<AccountId>)> = RefCell::new(Default::default());
    static SESSION_PER_ERA: RefCell<SessionIndex> = RefCell::new(3);
    static EXISTENTIAL_DEPOSIT: RefCell<Balance> = RefCell::new(0);
    static SLASH_DEFER_DURATION: RefCell<EraIndex> = RefCell::new(0);
    static ELECTION_LOOKAHEAD: RefCell<BlockNumber> = RefCell::new(0);
    static PERIOD: RefCell<BlockNumber> = RefCell::new(1);
    static MAX_ITERATIONS: RefCell<u32> = RefCell::new(0);
}

/// Another session handler struct to test on_disabled.
pub struct OtherSessionHandler;
impl pallet_session::OneSessionHandler<AccountId> for OtherSessionHandler {
    type Key = UintAuthorityId;

    fn on_genesis_session<'a, I: 'a>(_: I)
    where
        I: Iterator<Item = (&'a AccountId, Self::Key)>,
        AccountId: 'a,
    {
    }

    fn on_new_session<'a, I: 'a>(_: bool, validators: I, _: I)
    where
        I: Iterator<Item = (&'a AccountId, Self::Key)>,
        AccountId: 'a,
    {
        SESSION.with(|x| {
            *x.borrow_mut() = (validators.map(|x| x.0.clone()).collect(), HashSet::new())
        });
    }

    fn on_disabled(validator_index: usize) {
        SESSION.with(|d| {
            let mut d = d.borrow_mut();
            let value = d.0[validator_index];
            d.1.insert(value);
        })
    }
}

impl sp_runtime::BoundToRuntimeAppPublic for OtherSessionHandler {
    type Public = UintAuthorityId;
}

pub fn is_disabled(controller: AccountId) -> bool {
    let stash = Staking::ledger(&controller).unwrap().stash;
    SESSION.with(|d| d.borrow().1.contains(&stash))
}

pub struct ExistentialDeposit;
impl Get<Balance> for ExistentialDeposit {
    fn get() -> Balance {
        EXISTENTIAL_DEPOSIT.with(|v| *v.borrow())
    }
}

pub struct SessionsPerEra;
impl Get<SessionIndex> for SessionsPerEra {
    fn get() -> SessionIndex {
        SESSION_PER_ERA.with(|v| *v.borrow())
    }
}
impl Get<BlockNumber> for SessionsPerEra {
    fn get() -> BlockNumber {
        SESSION_PER_ERA.with(|v| *v.borrow() as BlockNumber)
    }
}

pub struct ElectionLookahead;
impl Get<BlockNumber> for ElectionLookahead {
    fn get() -> BlockNumber {
        ELECTION_LOOKAHEAD.with(|v| *v.borrow())
    }
}

pub struct Period;
impl Get<BlockNumber> for Period {
    fn get() -> BlockNumber {
        PERIOD.with(|v| *v.borrow())
    }
}

pub struct SlashDeferDuration;
impl Get<EraIndex> for SlashDeferDuration {
    fn get() -> EraIndex {
        SLASH_DEFER_DURATION.with(|v| *v.borrow())
    }
}

pub struct MaxIterations;
impl Get<u32> for MaxIterations {
    fn get() -> u32 {
        MAX_ITERATIONS.with(|v| *v.borrow())
    }
}

impl_outer_origin! {
    pub enum Origin for Test  where system = frame_system {}
}

type Pips = pallet_pips::Module<Test>;

impl_outer_dispatch! {
    pub enum Call for Test where origin: Origin {
        staking::Staking,
        pallet_pips::Pips,
        frame_system::System,
        pallet_scheduler::Scheduler,
    }
}

use frame_system as system;
use pallet_balances as balances;
use pallet_session as session;

impl_outer_event! {
    pub enum MetaEvent for Test {
        system<T>,
        balances<T>,
        session,
        pallet_pips<T>,
        pallet_treasury<T>,
        staking<T>,
        protocol_fee<T>,
        identity<T>,
        group Instance2<T>,
        pallet_scheduler<T>,
    }
}

/// Author of block is always 11
pub struct Author11;
impl FindAuthor<AccountId> for Author11 {
    fn find_author<'a, I>(_digests: I) -> Option<AccountId>
    where
        I: 'a + IntoIterator<Item = (frame_support::ConsensusEngineId, &'a [u8])>,
    {
        Some(11)
    }
}

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Test;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::one();
    pub const MaxLocks: u32 = 50;
}
impl frame_system::Trait for Test {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Index = AccountIndex;
    type BlockNumber = BlockNumber;
    type Call = Call;
    type Hash = H256;
    type Hashing = ::sp_runtime::traits::BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = MetaEvent;
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type DbWeight = RocksDbWeight;
    type BlockExecutionWeight = ();
    type ExtrinsicBaseWeight = ();
    type MaximumExtrinsicWeight = MaximumBlockWeight;
    type AvailableBlockRatio = AvailableBlockRatio;
    type MaximumBlockLength = MaximumBlockLength;
    type Version = ();
    type PalletInfo = ();
    type AccountData = AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
}

impl CommonTrait for Test {
    type Balance = Balance;
    type AssetSubTraitTarget = Test;
    type BlockRewardsReserve = balances::Module<Test>;
}

impl balances::Trait for Test {
    type DustRemoval = ();
    type Event = MetaEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type CddChecker = Test;
    type WeightInfo = polymesh_weights::pallet_balances::WeightInfo;
    type MaxLocks = MaxLocks;
}

parameter_types! {
    pub const Offset: BlockNumber = 0;
    pub const UncleGenerations: u64 = 0;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(25);
}
sp_runtime::impl_opaque_keys! {
    pub struct SessionKeys {
        pub other: OtherSessionHandler,
    }
}
impl pallet_session::Trait for Test {
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Test, Staking>;
    type Keys = SessionKeys;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionHandler = (OtherSessionHandler,);
    type Event = MetaEvent;
    type ValidatorId = AccountId;
    type ValidatorIdOf = StashOf<Test>;
    type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type WeightInfo = ();
}

impl pallet_session::historical::Trait for Test {
    type FullIdentification = Exposure<AccountId, Balance>;
    type FullIdentificationOf = ExposureOf<Test>;
}

impl pallet_pips::Trait for Test {
    type Currency = pallet_balances::Module<Self>;
    type VotingMajorityOrigin = frame_system::EnsureRoot<AccountId>;
    type GovernanceCommittee = crate::storage::Committee;
    type TechnicalCommitteeVMO = frame_system::EnsureRoot<AccountId>;
    type UpgradeCommitteeVMO = frame_system::EnsureRoot<AccountId>;
    type Treasury = pallet_treasury::Module<Self>;
    type Event = MetaEvent;
    type WeightInfo = polymesh_weights::pallet_pips::WeightInfo;
    type Scheduler = Scheduler;
    type SchedulerCall = Call;
}

impl pallet_treasury::Trait for Test {
    type Event = MetaEvent;
    type Currency = pallet_balances::Module<Self>;
    type WeightInfo = polymesh_weights::pallet_treasury::WeightInfo;
}

impl pallet_authorship::Trait for Test {
    type FindAuthor = Author11;
    type UncleGenerations = UncleGenerations;
    type FilterUncle = ();
    type EventHandler = pallet_staking::Module<Test>;
}
parameter_types! {
    pub const MinimumPeriod: u64 = 5;
}
impl pallet_timestamp::Trait for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl group::Trait<group::Instance2> for Test {
    type Event = MetaEvent;
    type LimitOrigin = frame_system::EnsureRoot<AccountId>;
    type AddOrigin = frame_system::EnsureRoot<AccountId>;
    type RemoveOrigin = frame_system::EnsureRoot<AccountId>;
    type SwapOrigin = frame_system::EnsureRoot<AccountId>;
    type ResetOrigin = frame_system::EnsureRoot<AccountId>;
    type MembershipInitialized = ();
    type MembershipChanged = ();
    type WeightInfo = polymesh_weights::pallet_group::WeightInfo;
}

impl protocol_fee::Trait for Test {
    type Event = MetaEvent;
    type Currency = Balances;
    type OnProtocolFeePayment = ();
    type WeightInfo = polymesh_weights::pallet_protocol_fee::WeightInfo;
}

impl IdentityTrait for Test {
    type Event = MetaEvent;
    type Proposal = Call;
    type MultiSig = Test;
    type Portfolio = Test;
    type CddServiceProviders = group::Module<Test, group::Instance2>;
    type Balances = Balances;
    type ChargeTxFeeTarget = Test;
    type CddHandler = Test;
    type Public = UintAuthorityId;
    type OffChainSignature = TestSignature;
    type ProtocolFee = protocol_fee::Module<Test>;
    type GCVotingMajorityOrigin = frame_system::EnsureRoot<AccountId>;
    type WeightInfo = polymesh_weights::pallet_identity::WeightInfo;
    type CorporateAction = Test;
    type IdentityFn = identity::Module<Test>;
    type SchedulerOrigin = OriginCaller;
}

parameter_types! {
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * MaximumBlockWeight::get();
    pub const MaxScheduledPerBlock: u32 = 50;
}

impl pallet_scheduler::Trait for Test {
    type Event = MetaEvent;
    type Origin = Origin;
    type PalletsOrigin = OriginCaller;
    type Call = Call;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = MaxScheduledPerBlock;
    type WeightInfo = ();
}

impl CddAndFeeDetails<AccountId, Call> for Test {
    fn get_valid_payer(_: &Call, _: &AccountId) -> Result<Option<AccountId>, InvalidTransaction> {
        Ok(None)
    }
    fn clear_context() {}
    fn set_payer_context(_: Option<AccountId>) {}
    fn get_payer_from_context() -> Option<AccountId> {
        None
    }
    fn set_current_identity(_: &IdentityId) {}
}

impl ChargeTxFee for Test {
    fn charge_fee(_len: u32, _info: DispatchInfo) -> TransactionValidity {
        Ok(ValidTransaction::default())
    }
}

impl GroupTrait<Moment> for Test {
    fn get_members() -> Vec<IdentityId> {
        return Group::active_members();
    }

    fn get_inactive_members() -> Vec<InactiveMember<Moment>> {
        vec![]
    }

    fn disable_member(
        _who: IdentityId,
        _expiry: Option<Moment>,
        _at: Option<Moment>,
    ) -> DispatchResult {
        unimplemented!();
    }

    fn add_member(_who: IdentityId) -> DispatchResult {
        unimplemented!()
    }

    fn get_active_members() -> Vec<IdentityId> {
        Self::get_members()
    }

    /// Current set size
    fn member_count() -> usize {
        Self::get_members().len()
    }

    fn is_member(member_id: &IdentityId) -> bool {
        Self::get_members().contains(member_id)
    }

    /// It returns the current "active members" and any "inactive member" which its
    /// expiration time-stamp is greater than `moment`.
    fn get_valid_members_at(moment: Moment) -> Vec<IdentityId> {
        Self::get_active_members()
            .into_iter()
            .chain(
                Self::get_inactive_members()
                    .into_iter()
                    .filter(|m| !Self::is_member_expired(&m, moment))
                    .map(|m| m.id),
            )
            .collect::<Vec<_>>()
    }

    fn is_member_expired(_member: &InactiveMember<Moment>, _now: Moment) -> bool {
        false
    }
}

impl AssetSubTrait<Balance> for Test {
    fn accept_ticker_transfer(_: IdentityId, _: u64) -> DispatchResult {
        Ok(())
    }
    fn accept_primary_issuance_agent_transfer(_: IdentityId, _: u64) -> DispatchResult {
        Ok(())
    }
    fn accept_asset_ownership_transfer(_: IdentityId, _: u64) -> DispatchResult {
        Ok(())
    }
    fn update_balance_of_scope_id(_: ScopeId, _: IdentityId, _: Ticker) -> DispatchResult {
        Ok(())
    }
    fn balance_of_at_scope(_: &ScopeId, _: &IdentityId) -> Balance {
        0
    }
}

impl IdentityToCorporateAction for Test {
    fn accept_corporate_action_agent_transfer(_: IdentityId, _: u64) -> DispatchResult {
        Ok(())
    }
}

impl MultiSigSubTrait<AccountId> for Test {
    fn accept_multisig_signer(_: Signatory<AccountId>, _: u64) -> DispatchResult {
        unimplemented!()
    }
    fn get_key_signers(_multisig: &AccountId) -> Vec<AccountId> {
        unimplemented!()
    }
    fn is_multisig(_account: &AccountId) -> bool {
        unimplemented!()
    }
    fn is_signer(_key: &AccountId) -> bool {
        // Allow all keys when mocked
        false
    }
}

impl PortfolioSubTrait<Balance, AccountId> for Test {
    fn accept_portfolio_custody(_: IdentityId, _: u64) -> DispatchResult {
        unimplemented!()
    }
    fn ensure_portfolio_custody(_: PortfolioId, _: IdentityId) -> DispatchResult {
        unimplemented!()
    }

    fn lock_tokens(_: &PortfolioId, _: &Ticker, _: &Balance) -> DispatchResult {
        unimplemented!()
    }

    fn unlock_tokens(_: &PortfolioId, _: &Ticker, _: &Balance) -> DispatchResult {
        unimplemented!()
    }

    fn ensure_portfolio_custody_and_permission(
        _: PortfolioId,
        _: IdentityId,
        _: Option<&SecondaryKey<AccountId>>,
    ) -> DispatchResult {
        unimplemented!()
    }
}

impl CheckCdd<AccountId> for Test {
    fn check_key_cdd(_key: &AccountId) -> bool {
        true
    }
    fn get_key_cdd_did(_key: &AccountId) -> Option<IdentityId> {
        None
    }
}

impl PermissionChecker for Test {
    type Checker = Identity;
}

parameter_types! {
    pub const EpochDuration: u64 = 10;
    pub const ExpectedBlockTime: u64 = 1;
}

impl From<pallet_babe::Call<Test>> for Call {
    fn from(_: pallet_babe::Call<Test>) -> Self {
        unimplemented!()
    }
}

impl pallet_babe::Trait for Test {
    type WeightInfo = ();
    type EpochDuration = EpochDuration;
    type ExpectedBlockTime = ExpectedBlockTime;
    type EpochChangeTrigger = pallet_babe::ExternalTrigger;

    type KeyOwnerProofSystem = ();
    type KeyOwnerProof = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
        KeyTypeId,
        pallet_babe::AuthorityId,
    )>>::Proof;
    type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
        KeyTypeId,
        pallet_babe::AuthorityId,
    )>>::IdentificationTuple;
    type HandleEquivocation = pallet_babe::EquivocationHandler<Self::KeyOwnerIdentification, ()>;
}

pallet_staking_reward_curve::build! {
    const I_NPOS: PiecewiseLinear<'static> = curve!(
        min_inflation: 0_025_000,
        max_inflation: 0_100_000,
        ideal_stake: 0_500_000,
        falloff: 0_050_000,
        max_piece_count: 40,
        test_precision: 0_005_000,
    );
}
parameter_types! {
    pub const BondingDuration: EraIndex = 3;
    pub const RewardCurve: &'static PiecewiseLinear<'static> = &I_NPOS;
    pub const MaxNominatorRewardedPerValidator: u32 = 64;
    pub const UnsignedPriority: u64 = 1 << 20;
    pub const MinSolutionScoreBump: Perbill = Perbill::zero();
    pub const MaxValidatorPerIdentity: Permill = Permill::from_percent(33);
    pub const MaxVariableInflationTotalIssuance: Balance = 1_000_000_000 * POLY;
    pub const FixedYearlyReward: Balance = 200_000_000 * POLY;
}

thread_local! {
    pub static REWARD_REMAINDER_UNBALANCED: RefCell<u128> = RefCell::new(0);
}

pub struct RewardRemainderMock;

impl OnUnbalanced<NegativeImbalanceOf<Test>> for RewardRemainderMock {
    fn on_nonzero_unbalanced(amount: NegativeImbalanceOf<Test>) {
        REWARD_REMAINDER_UNBALANCED.with(|v| {
            *v.borrow_mut() += amount.peek();
        });
        drop(amount);
    }
}

parameter_types! {
    pub const TwoThousand: AccountId = 2000;
    pub const ThreeThousand: AccountId = 3000;
    pub const FourThousand: AccountId = 4000;
    pub const FiveThousand: AccountId = 5000;
}

impl Contains<u64> for TwoThousand {
    fn sorted_members() -> std::vec::Vec<u64> {
        [2000, 3000, 4000, 5000].to_vec()
    }
}

impl Trait for Test {
    type Currency = Balances;
    type UnixTime = Timestamp;
    type CurrencyToVote = CurrencyToVoteHandler;
    type RewardRemainder = RewardRemainderMock;
    type Event = MetaEvent;
    type Slash = ();
    type Reward = ();
    type SessionsPerEra = SessionsPerEra;
    type SlashDeferDuration = SlashDeferDuration;
    type SlashCancelOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type BondingDuration = BondingDuration;
    type SessionInterface = Self;
    type RewardCurve = RewardCurve;
    type NextNewSession = Session;
    type ElectionLookahead = ElectionLookahead;
    type Call = Call;
    type MaxIterations = MaxIterations;
    type MinSolutionScoreBump = MinSolutionScoreBump;
    type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
    type UnsignedPriority = UnsignedPriority;
    type RequiredAddOrigin = frame_system::EnsureRoot<AccountId>;
    type RequiredRemoveOrigin = EnsureSignedBy<TwoThousand, Self::AccountId>;
    type RequiredComplianceOrigin = frame_system::EnsureRoot<AccountId>;
    type RequiredCommissionOrigin = frame_system::EnsureRoot<AccountId>;
    type RequiredChangeHistoryDepthOrigin = frame_system::EnsureRoot<AccountId>;
    type RewardScheduler = Scheduler;
    type PalletsOrigin = OriginCaller;
    type MaxValidatorPerIdentity = MaxValidatorPerIdentity;
    type MaxVariableInflationTotalIssuance = MaxVariableInflationTotalIssuance;
    type FixedYearlyReward = FixedYearlyReward;
    type WeightInfo = polymesh_weights::pallet_staking::WeightInfo;
}

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for Test
where
    Call: From<LocalCall>,
{
    type OverarchingCall = Call;
    type Extrinsic = Extrinsic;
}

pub type Extrinsic = TestXt<Call, ()>;

pub struct ExtBuilder {
    session_length: BlockNumber,
    election_lookahead: BlockNumber,
    session_per_era: SessionIndex,
    existential_deposit: Balance,
    validator_pool: bool,
    nominate: bool,
    validator_count: u32,
    minimum_validator_count: u32,
    slash_defer_duration: EraIndex,
    fair: bool,
    num_validators: Option<u32>,
    invulnerables: Vec<AccountId>,
    has_stakers: bool,
    max_offchain_iterations: u32,
    slashing_allowed_for: SlashingSwitch,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            session_length: 1,
            election_lookahead: 0,
            session_per_era: 3,
            existential_deposit: 1,
            validator_pool: false,
            nominate: true,
            validator_count: 2,
            minimum_validator_count: 0,
            slash_defer_duration: 0,
            fair: true,
            num_validators: None,
            invulnerables: vec![],
            has_stakers: true,
            max_offchain_iterations: 0,
            slashing_allowed_for: SlashingSwitch::Validator,
        }
    }
}

impl ExtBuilder {
    pub fn existential_deposit(mut self, existential_deposit: Balance) -> Self {
        self.existential_deposit = existential_deposit;
        self
    }
    pub fn validator_pool(mut self, validator_pool: bool) -> Self {
        self.validator_pool = validator_pool;
        self
    }
    pub fn nominate(mut self, nominate: bool) -> Self {
        self.nominate = nominate;
        self
    }
    pub fn validator_count(mut self, count: u32) -> Self {
        self.validator_count = count;
        self
    }
    pub fn minimum_validator_count(mut self, count: u32) -> Self {
        self.minimum_validator_count = count;
        self
    }
    pub fn slash_defer_duration(mut self, eras: EraIndex) -> Self {
        self.slash_defer_duration = eras;
        self
    }
    pub fn fair(mut self, is_fair: bool) -> Self {
        self.fair = is_fair;
        self
    }
    pub fn num_validators(mut self, num_validators: u32) -> Self {
        self.num_validators = Some(num_validators);
        self
    }
    pub fn invulnerables(mut self, invulnerables: Vec<AccountId>) -> Self {
        self.invulnerables = invulnerables;
        self
    }
    pub fn session_per_era(mut self, length: SessionIndex) -> Self {
        self.session_per_era = length;
        self
    }
    pub fn election_lookahead(mut self, look: BlockNumber) -> Self {
        self.election_lookahead = look;
        self
    }
    pub fn session_length(mut self, length: BlockNumber) -> Self {
        self.session_length = length;
        self
    }
    pub fn has_stakers(mut self, has: bool) -> Self {
        self.has_stakers = has;
        self
    }
    pub fn max_offchain_iterations(mut self, iterations: u32) -> Self {
        self.max_offchain_iterations = iterations;
        self
    }
    pub fn offchain_phragmen_ext(self) -> Self {
        self.session_per_era(4)
            .session_length(5)
            .election_lookahead(3)
    }
    pub fn slashing_allowed_for(mut self, status: SlashingSwitch) -> Self {
        self.slashing_allowed_for = status;
        self
    }
    pub fn set_associated_constants(&self) {
        EXISTENTIAL_DEPOSIT.with(|v| *v.borrow_mut() = self.existential_deposit);
        SLASH_DEFER_DURATION.with(|v| *v.borrow_mut() = self.slash_defer_duration);
        SESSION_PER_ERA.with(|v| *v.borrow_mut() = self.session_per_era);
        ELECTION_LOOKAHEAD.with(|v| *v.borrow_mut() = self.election_lookahead);
        PERIOD.with(|v| *v.borrow_mut() = self.session_length);
        MAX_ITERATIONS.with(|v| *v.borrow_mut() = self.max_offchain_iterations);
    }
    pub fn build(self) -> sp_io::TestExternalities {
        let _ = env_logger::try_init();
        self.set_associated_constants();
        let mut storage = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();
        let balance_factor = if self.existential_deposit > 1 { 256 } else { 1 };

        let num_validators = self.num_validators.unwrap_or(self.validator_count);
        let validators = (0..num_validators)
            .map(|x| ((x + 1) * 10 + 1) as AccountId)
            .collect::<Vec<_>>();

        let _ = pallet_balances::GenesisConfig::<Test> {
            balances: vec![
                (1, 10 * balance_factor),
                (2, 20 * balance_factor),
                (3, 300 * balance_factor),
                (4, 400 * balance_factor),
                (10, balance_factor),
                (11, balance_factor * 1000),
                (20, balance_factor),
                (21, balance_factor * 2000),
                (30, balance_factor),
                (31, balance_factor * 2000),
                (40, balance_factor),
                (41, balance_factor * 2000),
                (100, 2000 * balance_factor),
                (101, 2000 * balance_factor),
                (1005, 200000 * balance_factor),
                // This allows us to have a total_payout different from 0.
                (999, 1_000_000_000_000),
            ],
        }
        .assimilate_storage(&mut storage);

        let _ = group::GenesisConfig::<Test, group::Instance2> {
            active_members_limit: u32::MAX,
            active_members: vec![IdentityId::from(1), IdentityId::from(2)],
            phantom: Default::default(),
        }
        .assimilate_storage(&mut storage);

        let _ = identity::GenesisConfig::<Test> {
            identities: vec![
                // (primary_account_id, service provider did, target did, expiry time of CustomerDueDiligence claim i.e 10 days is ms)
                // Provide Identity
                (
                    1005,
                    IdentityId::from(1),
                    IdentityId::from(1),
                    InvestorUid::from(b"uid1".as_ref()),
                    None,
                ),
                (
                    11,
                    IdentityId::from(1),
                    IdentityId::from(11),
                    InvestorUid::from(b"uid11".as_ref()),
                    None,
                ),
                (
                    21,
                    IdentityId::from(1),
                    IdentityId::from(21),
                    InvestorUid::from(b"uid21".as_ref()),
                    None,
                ),
                (
                    31,
                    IdentityId::from(1),
                    IdentityId::from(31),
                    InvestorUid::from(b"uid31".as_ref()),
                    None,
                ),
                (
                    41,
                    IdentityId::from(1),
                    IdentityId::from(41),
                    InvestorUid::from(b"uid41".as_ref()),
                    None,
                ),
                (
                    101,
                    IdentityId::from(1),
                    IdentityId::from(101),
                    InvestorUid::from(b"uid101".as_ref()),
                    None,
                ),
            ],
            ..Default::default()
        }
        .assimilate_storage(&mut storage);

        let mut stakers = vec![];
        if self.has_stakers {
            let stake_21 = if self.fair { 1000 } else { 2000 };
            let stake_31 = if self.validator_pool {
                balance_factor * 1000
            } else {
                1
            };
            let status_41 = if self.validator_pool {
                StakerStatus::<AccountId>::Validator
            } else {
                StakerStatus::<AccountId>::Idle
            };
            let nominated = if self.nominate { vec![11, 21] } else { vec![] };
            stakers = vec![
                // (IdentityId, stash, controller, staked_amount, status)
                (
                    IdentityId::from(11),
                    11,
                    10,
                    balance_factor * 1000,
                    StakerStatus::<AccountId>::Validator,
                ),
                (
                    IdentityId::from(21),
                    21,
                    20,
                    stake_21,
                    StakerStatus::<AccountId>::Validator,
                ),
                (
                    IdentityId::from(31),
                    31,
                    30,
                    stake_31,
                    StakerStatus::<AccountId>::Validator,
                ),
                (
                    IdentityId::from(41),
                    41,
                    40,
                    balance_factor * 1000,
                    status_41,
                ),
                // nominator
                (
                    IdentityId::from(101),
                    101,
                    100,
                    balance_factor * 500,
                    StakerStatus::<AccountId>::Nominator(nominated),
                ),
            ];
        }
        let _ = pallet_staking::GenesisConfig::<Test> {
            stakers,
            validator_count: self.validator_count,
            minimum_validator_count: self.minimum_validator_count,
            invulnerables: self.invulnerables,
            slash_reward_fraction: Perbill::from_percent(10),
            slashing_allowed_for: self.slashing_allowed_for,
            ..Default::default()
        }
        .assimilate_storage(&mut storage);

        let _ = pallet_session::GenesisConfig::<Test> {
            keys: validators
                .iter()
                .map(|x| {
                    (
                        *x,
                        *x,
                        SessionKeys {
                            other: UintAuthorityId(*x as u64),
                        },
                    )
                })
                .collect(),
        }
        .assimilate_storage(&mut storage);

        let mut ext = sp_io::TestExternalities::from(storage);
        ext.execute_with(|| {
            let validators = Session::validators();
            SESSION.with(|x| *x.borrow_mut() = (validators.clone(), HashSet::new()));
        });

        // We consider all test to start after timestamp is initialized
        // This must be ensured by having `timestamp::on_initialize` called before
        // `staking::on_initialize`
        ext.execute_with(|| {
            System::set_block_number(1);
            Timestamp::set_timestamp(INIT_TIMESTAMP);
        });

        ext
    }
    pub fn build_and_execute(self, test: impl FnOnce() -> ()) {
        let mut ext = self.build();
        ext.execute_with(test);
        ext.execute_with(post_conditions);
    }
}

pub type System = frame_system::Module<Test>;
pub type Balances = balances::Module<Test>;
pub type Session = pallet_session::Module<Test>;
pub type Timestamp = pallet_timestamp::Module<Test>;
pub type Group = group::Module<Test, group::Instance2>;
pub type Staking = pallet_staking::Module<Test>;
pub type Identity = identity::Module<Test>;
pub type Scheduler = pallet_scheduler::Module<Test>;

pub(crate) fn current_era() -> EraIndex {
    Staking::current_era().unwrap()
}

fn post_conditions() {
    check_nominators();
    check_exposures();
    check_ledgers();
}

pub(crate) fn active_era() -> EraIndex {
    Staking::active_era().unwrap().index
}

pub fn provide_did_to_user(account: AccountId) -> bool {
    if <identity::KeyToIdentityIds<Test>>::contains_key(&account) {
        return false;
    }
    let cdd_account_id = 1005;
    let cdd = Origin::signed(cdd_account_id);
    assert!(
        <identity::KeyToIdentityIds<Test>>::contains_key(&cdd_account_id),
        "CDD provider account not mapped to identity"
    );
    let cdd_did = <identity::KeyToIdentityIds<Test>>::get(&cdd_account_id);
    assert!(
        <identity::DidRecords<Test>>::contains_key(&cdd_did),
        "CDD provider identity has no DID record"
    );
    let cdd_did_record = <identity::DidRecords<Test>>::get(&cdd_did);
    assert!(
        cdd_did_record.primary_key == cdd_account_id,
        "CDD identity primary key mismatch"
    );
    assert!(
        Identity::cdd_register_did(cdd.clone(), account, vec![]).is_ok(),
        "Error in registering the DID"
    );
    let did = Identity::get_identity(&account).expect("DID not find in the storage");
    let (cdd_id, _) = create_cdd_id_and_investor_uid(did);
    assert!(
        Identity::add_claim(cdd.clone(), did, Claim::CustomerDueDiligence(cdd_id), None).is_ok(),
        "Error CDD Claim cannot be added to DID"
    );
    true
}

pub fn add_secondary_key(stash_key: AccountId, to_secondary_key: AccountId) {
    if !get_identity(to_secondary_key) {
        let _did = Identity::get_identity(&stash_key).unwrap();
        assert!(
            Identity::add_authorization(
                Origin::signed(stash_key),
                Signatory::Account(to_secondary_key),
                AuthorizationData::JoinIdentity(Permissions::default()),
                None
            )
            .is_ok(),
            "Error in providing the authorization"
        );
        let auth_id = get_last_auth_id(&Signatory::Account(to_secondary_key));
        assert_ok!(Identity::join_identity_as_key(
            Origin::signed(to_secondary_key),
            auth_id
        ));
    }
}

pub fn get_identity(key: AccountId) -> bool {
    <identity::KeyToIdentityIds<Test>>::contains_key(&key)
}

fn check_ledgers() {
    // check the ledger of all stakers.
    Bonded::<Test>::iter().for_each(|(_, ctrl)| assert_ledger_consistent(ctrl))
}

fn check_exposures() {
    // a check per validator to ensure the exposure struct is always sane.
    let era = active_era();
    ErasStakers::<Test>::iter_prefix_values(era).for_each(|expo| {
        assert_eq!(
            expo.total as u128,
            expo.own as u128 + expo.others.iter().map(|e| e.value as u128).sum::<u128>(),
            "wrong total exposure.",
        );
    })
}

fn check_nominators() {
    // a check per nominator to ensure their entire stake is correctly distributed. Will only kick-
    // in if the nomination was submitted before the current era.
    let era = active_era();
    <Nominators<Test>>::iter()
        .filter_map(|(nominator, nomination)| {
            if nomination.submitted_in > era {
                Some(nominator)
            } else {
                None
            }
        })
        .for_each(|nominator| {
            // must be bonded.
            assert_is_stash(nominator);
            let mut sum = 0;
            Session::validators()
                .iter()
                .map(|v| Staking::eras_stakers(era, v))
                .for_each(|e| {
                    let individual = e
                        .others
                        .iter()
                        .filter(|e| e.who == nominator)
                        .collect::<Vec<_>>();
                    let len = individual.len();
                    match len {
                        0 => { /* not supporting this validator at all. */ }
                        1 => sum += individual[0].value,
                        _ => panic!("nominator cannot back a validator more than once."),
                    };
                });

            let nominator_stake = Staking::slashable_balance_of(&nominator);
            // a nominator cannot over-spend.
            assert!(
                nominator_stake >= sum,
                "failed: Nominator({}) stake({}) >= sum divided({})",
                nominator,
                nominator_stake,
                sum,
            );

            let diff = nominator_stake - sum;
            assert!(diff < 100);
        });
}

fn assert_is_stash(acc: AccountId) {
    assert!(Staking::bonded(&acc).is_some(), "Not a stash.");
}

fn assert_ledger_consistent(ctrl: AccountId) {
    // ensures ledger.total == ledger.active + sum(ledger.unlocking).
    let ledger = Staking::ledger(ctrl).expect("Not a controller.");
    let real_total: Balance = ledger
        .unlocking
        .iter()
        .fold(ledger.active, |a, c| a + c.value);
    assert_eq!(real_total, ledger.total);
}

pub fn bond_validator(stash: AccountId, ctrl: AccountId, val: Balance) {
    bond_validator_with_intended_count(stash, ctrl, val, None)
}

pub fn bond_validator_with_intended_count(
    stash: AccountId,
    ctrl: AccountId,
    val: Balance,
    i_count: Option<u32>,
) {
    bond(stash, ctrl, val);
    let entity_id = Identity::get_identity(&stash).unwrap();
    if Staking::permissioned_identity(entity_id).is_none() {
        assert_ok!(Staking::add_permissioned_validator(
            frame_system::RawOrigin::Root.into(),
            entity_id,
            i_count
        ));
    }
    assert_ok!(Staking::validate(
        Origin::signed(ctrl),
        ValidatorPrefs::default()
    ));
}

pub fn bond(stash: AccountId, ctrl: AccountId, val: Balance) {
    let _ = Balances::make_free_balance_be(&stash, val);
    let _ = Balances::make_free_balance_be(&ctrl, val);
    provide_did_to_user(stash);
    add_secondary_key(stash, ctrl);
    if Staking::bonded(&stash).is_none() {
        assert_ok!(Staking::bond(
            Origin::signed(stash),
            ctrl,
            val,
            RewardDestination::Controller,
        ));
    }
}

pub(crate) fn bond_nominator(
    stash: AccountId,
    ctrl: AccountId,
    val: Balance,
    target: Vec<AccountId>,
) {
    let _ = Balances::make_free_balance_be(&stash, val);
    let _ = Balances::make_free_balance_be(&ctrl, val);
    assert_ok!(Staking::bond(
        Origin::signed(stash),
        ctrl,
        val,
        RewardDestination::Controller,
    ));
    assert_ok!(Staking::nominate(Origin::signed(ctrl), target));
}

pub fn bond_nominator_cdd(stash: AccountId, ctrl: AccountId, val: Balance, target: Vec<AccountId>) {
    provide_did_to_user(stash);
    add_secondary_key(stash, ctrl);
    bond_nominator(stash, ctrl, val, target);
}

pub fn run_to_block(n: BlockNumber) {
    Staking::on_finalize(System::block_number());
    for b in System::block_number() + 1..=n {
        System::set_block_number(b);
        Session::on_initialize(b);
        Staking::on_initialize(b);
        if b != n {
            Staking::on_finalize(System::block_number());
        }
    }
}

pub fn advance_session() {
    let current_index = Session::current_index();
    start_session(current_index + 1);
}

pub fn start_session(session_index: SessionIndex) {
    assert_eq!(
        <Period as Get<BlockNumber>>::get(),
        1,
        "start_session can only be used with session length 1."
    );
    for i in Session::current_index()..session_index {
        Staking::on_finalize(System::block_number());
        System::set_block_number((i + 1).into());
        Timestamp::set_timestamp(System::block_number() * 1000 + INIT_TIMESTAMP);
        Session::on_initialize(System::block_number());
        Staking::on_initialize(System::block_number());
    }

    assert_eq!(Session::current_index(), session_index);
}

// This start and activate the era given.
// Because the mock use pallet-session which delays session by one, this will be one session after
// the election happened, not the first session after the election has happened.
pub(crate) fn start_era(era_index: EraIndex) {
    start_session((era_index * <SessionsPerEra as Get<u32>>::get()).into());
    assert_eq!(Staking::current_era().unwrap(), era_index);
    assert_eq!(Staking::active_era().unwrap().index, era_index);
}

pub(crate) fn current_total_payout_for_duration(duration: u64) -> Balance {
    inflation::compute_total_payout(
        <Test as Trait>::RewardCurve::get(),
        Staking::eras_total_stake(Staking::active_era().unwrap().index),
        Balances::total_issuance(),
        duration,
        MaxVariableInflationTotalIssuance::get(),
        FixedYearlyReward::get(),
    )
    .0
}

pub fn reward_all_elected() {
    let rewards = <Test as Trait>::SessionInterface::validators()
        .into_iter()
        .map(|v| (v, 1));

    <Module<Test>>::reward_by_ids(rewards)
}

pub(crate) fn validator_controllers() -> Vec<AccountId> {
    Session::validators()
        .into_iter()
        .map(|s| Staking::bonded(&s).expect("no controller for validator"))
        .collect()
}

pub(crate) fn on_offence_in_era(
    offenders: &[OffenceDetails<
        AccountId,
        pallet_session::historical::IdentificationTuple<Test>,
    >],
    slash_fraction: &[Perbill],
    era: EraIndex,
) {
    let bonded_eras = staking::BondedEras::get();
    for &(bonded_era, start_session) in bonded_eras.iter() {
        if bonded_era == era {
            let _ = Staking::on_offence(offenders, slash_fraction, start_session).unwrap();
            return;
        } else if bonded_era > era {
            break;
        }
    }

    if Staking::active_era().unwrap().index == era {
        let _ = Staking::on_offence(
            offenders,
            slash_fraction,
            Staking::eras_start_session_index(era).unwrap(),
        )
        .unwrap();
    } else {
        panic!("cannot slash in era {}", era);
    }
}

pub(crate) fn on_offence_now(
    offenders: &[OffenceDetails<
        AccountId,
        pallet_session::historical::IdentificationTuple<Test>,
    >],
    slash_fraction: &[Perbill],
) {
    let now = Staking::active_era().unwrap().index;
    on_offence_in_era(offenders, slash_fraction, now)
}

pub(crate) fn add_slash(who: &AccountId) {
    on_offence_now(
        &[OffenceDetails {
            offender: (
                who.clone(),
                Staking::eras_stakers(Staking::active_era().unwrap().index, who.clone()),
            ),
            reporters: vec![],
        }],
        &[Perbill::from_percent(10)],
    );
}

// winners will be chosen by simply their unweighted total backing stake. Nominator stake is
// distributed evenly.
pub(crate) fn horrible_phragmen_with_post_processing(
    do_reduce: bool,
) -> (CompactAssignments, Vec<ValidatorIndex>, ElectionScore) {
    let mut backing_stake_of: BTreeMap<AccountId, Balance> = BTreeMap::new();

    // self stake
    <Validators<Test>>::iter().for_each(|(who, _p)| {
        *backing_stake_of.entry(who).or_insert(Zero::zero()) += Staking::slashable_balance_of(&who)
    });

    // add nominator stuff
    <Nominators<Test>>::iter().for_each(|(who, nomination)| {
        nomination.targets.iter().for_each(|v| {
            *backing_stake_of.entry(*v).or_insert(Zero::zero()) +=
                Staking::slashable_balance_of(&who)
        })
    });

    // elect winners
    let mut sorted: Vec<AccountId> = backing_stake_of.keys().cloned().collect();
    sorted.sort_by_key(|x| backing_stake_of.get(x).unwrap());
    let winners: Vec<AccountId> = sorted
        .iter()
        .cloned()
        .take(Staking::validator_count() as usize)
        .collect();

    // create assignments
    let mut staked_assignment: Vec<StakedAssignment<AccountId>> = Vec::new();
    <Nominators<Test>>::iter().for_each(|(who, nomination)| {
        let mut dist: Vec<(AccountId, ExtendedBalance)> = Vec::new();
        nomination.targets.iter().for_each(|v| {
            if winners.iter().find(|w| *w == v).is_some() {
                dist.push((*v, ExtendedBalance::zero()));
            }
        });

        if dist.len() == 0 {
            return;
        }

        // assign real stakes. just split the stake.
        let stake = Staking::slashable_balance_of(&who) as ExtendedBalance;
        let mut sum: ExtendedBalance = Zero::zero();
        let dist_len = dist.len();
        {
            dist.iter_mut().for_each(|(_, w)| {
                let partial = stake / (dist_len as ExtendedBalance);
                *w = partial;
                sum += partial;
            });
        }

        // assign the leftover to last.
        {
            let leftover = stake - sum;
            let last = dist.last_mut().unwrap();
            last.1 += leftover;
        }

        staked_assignment.push(StakedAssignment {
            who,
            distribution: dist,
        });
    });

    // Ensure that this result is worse than seq-phragmen. Otherwise, it should not have been used
    // for testing.
    let score = {
        let (_, _, better_score) = prepare_submission_with(true, 0, |_| {});

        let support = build_support_map::<AccountId>(&winners, &staked_assignment).0;
        let score = evaluate_support(&support);

        assert!(sp_npos_elections::is_score_better::<Perbill>(
            better_score,
            score,
            MinSolutionScoreBump::get(),
        ));

        score
    };

    if do_reduce {
        reduce(&mut staked_assignment);
    }

    let snapshot_validators = Staking::snapshot_validators().unwrap();
    let snapshot_nominators = Staking::snapshot_nominators().unwrap();
    let nominator_index = |a: &AccountId| -> Option<NominatorIndex> {
        snapshot_nominators
            .iter()
            .position(|x| x == a)
            .map(|i| i as NominatorIndex)
    };
    let validator_index = |a: &AccountId| -> Option<ValidatorIndex> {
        snapshot_validators
            .iter()
            .position(|x| x == a)
            .map(|i| i as ValidatorIndex)
    };

    // convert back to ratio assignment. This takes less space.
    let assignments_reduced = sp_npos_elections::assignment_staked_to_ratio::<
        AccountId,
        OffchainAccuracy,
    >(staked_assignment);

    let compact =
        CompactAssignments::from_assignment(assignments_reduced, nominator_index, validator_index)
            .unwrap();

    // winner ids to index
    let winners = winners
        .into_iter()
        .map(|w| validator_index(&w).unwrap())
        .collect::<Vec<_>>();

    (compact, winners, score)
}

// Note: this should always logically reproduce [`offchain_election::prepare_submission`], yet we
// cannot do it since we want to have `tweak` injected into the process.
pub(crate) fn prepare_submission_with(
    do_reduce: bool,
    iterations: usize,
    tweak: impl FnOnce(&mut Vec<StakedAssignment<AccountId>>),
) -> (CompactAssignments, Vec<ValidatorIndex>, ElectionScore) {
    // run election on the default stuff.
    let sp_npos_elections::ElectionResult {
        winners,
        assignments,
    } = Staking::do_phragmen::<OffchainAccuracy>().unwrap();
    let winners = sp_npos_elections::to_without_backing(winners);

    let stake_of = |who: &AccountId| -> VoteWeight {
        <CurrencyToVoteHandler as Convert<Balance, VoteWeight>>::convert(
            Staking::slashable_balance_of(&who),
        )
    };

    let mut staked = sp_npos_elections::assignment_ratio_to_staked(assignments, stake_of);
    let (mut support_map, _) = build_support_map::<AccountId>(&winners, &staked);

    if iterations > 0 {
        sp_npos_elections::balance_solution(
            &mut staked,
            &mut support_map,
            Zero::zero(),
            iterations,
        );
    }

    // apply custom tweaks. awesome for testing.
    tweak(&mut staked);

    if do_reduce {
        reduce(&mut staked);
    }

    // convert back to ratio assignment. This takes less space.
    let snapshot_validators = Staking::snapshot_validators().expect("snapshot not created.");
    let snapshot_nominators = Staking::snapshot_nominators().expect("snapshot not created.");
    let nominator_index = |a: &AccountId| -> Option<NominatorIndex> {
        snapshot_nominators.iter().position(|x| x == a).map_or_else(
            || {
                println!("unable to find nominator index for {:?}", a);
                None
            },
            |i| Some(i as NominatorIndex),
        )
    };
    let validator_index = |a: &AccountId| -> Option<ValidatorIndex> {
        snapshot_validators.iter().position(|x| x == a).map_or_else(
            || {
                println!("unable to find validator index for {:?}", a);
                None
            },
            |i| Some(i as ValidatorIndex),
        )
    };

    let assignments_reduced = sp_npos_elections::assignment_staked_to_ratio(staked);

    // re-compute score by converting, yet again, into staked type
    let score = {
        let staked = sp_npos_elections::assignment_ratio_to_staked(
            assignments_reduced.clone(),
            Staking::slashable_balance_of_vote_weight,
        );

        let (support_map, _) =
            build_support_map::<AccountId>(winners.as_slice(), staked.as_slice());
        evaluate_support::<AccountId>(&support_map)
    };

    let compact =
        CompactAssignments::from_assignment(assignments_reduced, nominator_index, validator_index)
            .map_err(|e| {
                println!("error in compact: {:?}", e);
                e
            })
            .expect("Failed to create compact");

    // winner ids to index
    let winners = winners
        .into_iter()
        .map(|w| validator_index(&w).unwrap())
        .collect::<Vec<_>>();

    (compact, winners, score)
}

/// Make all validator and nominator request their payment
pub(crate) fn make_all_reward_payment(era: EraIndex) {
    let validators_with_reward = ErasRewardPoints::<Test>::get(era)
        .individual
        .keys()
        .cloned()
        .collect::<Vec<_>>();

    // reward validators
    for validator_controller in validators_with_reward.iter().filter_map(Staking::bonded) {
        let ledger = <Ledger<Test>>::get(&validator_controller).unwrap();

        assert_ok!(Staking::payout_stakers(
            Origin::signed(1337),
            ledger.stash,
            era
        ));
    }
}

pub(crate) fn staking_events() -> Vec<Event<Test>> {
    System::events()
        .into_iter()
        .map(|r| r.event)
        .filter_map(|e| {
            if let MetaEvent::staking(inner) = e {
                Some(inner)
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn balances(who: &AccountId) -> (Balance, Balance) {
    (Balances::free_balance(who), Balances::reserved_balance(who))
}

pub fn make_account_with_uid(
    id: AccountId,
) -> Result<(<Test as frame_system::Trait>::Origin, IdentityId), &'static str> {
    make_account_with_balance(id, 1_000_000, None)
}

/// It creates an Account and registers its DID.
pub fn make_account_with_balance(
    id: AccountId,
    balance: <Test as CommonTrait>::Balance,
    expiry: Option<Moment>,
) -> Result<(<Test as frame_system::Trait>::Origin, IdentityId), &'static str> {
    let signed_id = Origin::signed(id.clone());
    Balances::make_free_balance_be(&id, balance);
    let uid = create_investor_uid(id);
    // If we have CDD providers, first of them executes the registration.
    let cdd_providers = Group::get_members();
    let did = match cdd_providers.into_iter().nth(0) {
        Some(cdd_provider) => {
            let cdd_acc = Identity::did_records(&cdd_provider).primary_key;
            let _ = Identity::cdd_register_did(Origin::signed(cdd_acc), id, vec![])
                .map_err(|_| "CDD register DID failed")?;
            let did = Identity::get_identity(&id).unwrap();
            let (cdd_id, _) = create_cdd_id(did, Ticker::default(), uid);
            let cdd_claim = Claim::CustomerDueDiligence(cdd_id);
            Identity::add_claim(Origin::signed(cdd_acc), did, cdd_claim, expiry)
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

pub fn add_trusted_cdd_provider(cdd_sp: IdentityId) {
    let root = Origin::from(frame_system::RawOrigin::Root);
    assert_ok!(Group::add_member(root, cdd_sp));
}

pub fn add_nominator_claim_with_expiry(
    _claim_issuer: IdentityId,
    identity_id: IdentityId,
    claim_issuer_account_id: AccountId,
    expiry: u64,
) {
    let signed_claim_issuer_id = Origin::signed(claim_issuer_account_id);
    let (cdd_id, _) = create_cdd_id_and_investor_uid(identity_id);
    assert_ok!(Identity::add_claim(
        signed_claim_issuer_id,
        identity_id,
        Claim::CustomerDueDiligence(cdd_id),
        Some(expiry.into()),
    ));
}

pub fn create_cdd_id_and_investor_uid(identity_id: IdentityId) -> (CddId, InvestorUid) {
    let uid = create_investor_uid(Identity::did_records(identity_id).primary_key);
    let (cdd_id, _) = create_cdd_id(identity_id, Ticker::default(), uid);
    (cdd_id, uid)
}

pub fn create_investor_uid(acc: AccountId) -> InvestorUid {
    InvestorUid::from(format!("{}", acc).as_str())
}

pub fn add_nominator_claim(
    _claim_issuer: IdentityId,
    identity_id: IdentityId,
    claim_issuer_account_id: AccountId,
) {
    let signed_claim_issuer_id = Origin::signed(claim_issuer_account_id);
    let now = Utc::now();
    let (cdd_id, _) = create_cdd_id_and_investor_uid(identity_id);
    assert_ok!(Identity::add_claim(
        signed_claim_issuer_id,
        identity_id,
        Claim::CustomerDueDiligence(cdd_id),
        Some((now.timestamp() as u64 + 10000_u64).into()),
    ));
}

pub fn bond_nominator_with_expiry(acc: u64, val: u128, claim_expiry: u64, target: Vec<AccountId>) {
    // a = controller
    // a + 1 = stash
    let controller = acc;
    let stash = acc + 1;
    let _ = Balances::make_free_balance_be(&(stash), val);
    assert_ok!(Staking::bond(
        Origin::signed(stash),
        controller,
        val,
        RewardDestination::Controller
    ));
    create_did_and_add_claim_with_expiry(stash, claim_expiry);
    assert_ok!(Staking::nominate(Origin::signed(controller), target));
}

pub fn create_did_and_add_claim_with_expiry(stash: AccountId, expiry: u64) {
    Balances::make_free_balance_be(&1005, 1_000_000);
    assert_ok!(Identity::cdd_register_did(
        Origin::signed(1005),
        stash,
        vec![]
    ));
    let did = Identity::get_identity(&stash).unwrap();
    let (cdd_id, _) = create_cdd_id_and_investor_uid(did);
    assert_ok!(Identity::add_claim(
        Origin::signed(1005),
        did,
        Claim::CustomerDueDiligence(cdd_id),
        Some(expiry.into())
    ));
}

// `iter_prefix_values` has no guarantee that it will iterate in a sequential
// order. However, we need the latest `auth_id`. Which is why we search for the claim
// with the highest `auth_id`.
pub fn get_last_auth(signatory: &Signatory<AccountId>) -> Authorization<AccountId, u64> {
    <identity::Authorizations<Test>>::iter_prefix_values(signatory)
        .into_iter()
        .max_by_key(|x| x.auth_id)
        .expect("there are no authorizations")
}

pub fn get_last_auth_id(signatory: &Signatory<AccountId>) -> u64 {
    get_last_auth(signatory).auth_id
}

pub fn root() -> Origin {
    Origin::from(frame_system::RawOrigin::Root)
}

pub fn run_to_block_scheduler(n: u64) {
    while System::block_number() < n {
        Staking::on_finalize(System::block_number());
        Scheduler::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        Scheduler::on_initialize(System::block_number());
        Session::on_initialize(System::block_number());
        Staking::on_initialize(System::block_number());
        Staking::on_finalize(System::block_number());
    }
}
