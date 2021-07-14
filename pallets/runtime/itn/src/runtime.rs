#![allow(clippy::not_unsafe_ptr_arg_deref)]

use crate::{constants::time::*, fee_details::CddHandler};
use codec::Encode;
use frame_support::{
    construct_runtime, debug, parameter_types,
    traits::{KeyOwnerProofSystem, Randomness, SplitTwoWays},
    weights::Weight,
};
//use pallet_contracts::weights::WeightInfo;
use pallet_asset::checkpoint as pallet_checkpoint;
use pallet_corporate_actions::ballot as pallet_corporate_ballot;
use pallet_corporate_actions::distribution as pallet_capital_distribution;
use pallet_session::historical as pallet_session_historical;
pub use pallet_transaction_payment::{Multiplier, RuntimeDispatchInfo, TargetedFeeAdjustment};
use polymesh_common_utilities::{constants::currency::*, protocol_fee::ProtocolOp};
use polymesh_primitives::{Balance, BlockNumber, Moment};
use polymesh_runtime_common::{
    impls::Author,
    merge_active_and_inactive,
    runtime::{GovernanceCommittee, VMO},
    AvailableBlockRatio, MaximumBlockWeight, NegativeImbalance,
};
use sp_core::u32_trait::{_1, _4};
use sp_runtime::transaction_validity::TransactionPriority;
use sp_runtime::{
    create_runtime_str,
    curve::PiecewiseLinear,
    traits::{BlakeTwo256, Block as BlockT, Extrinsic, NumberFor, StaticLookup, Verify},
    Perbill, Permill,
};
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

pub use frame_support::StorageValue;
pub use frame_system::Call as SystemCall;
pub use pallet_balances::Call as BalancesCall;
pub use pallet_staking::StakerStatus;
pub use pallet_timestamp::Call as TimestampCall;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

/// Runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("polymesh"),
    impl_name: create_runtime_str!("polymath-polymesh"),
    authoring_version: 1,
    // Per convention: if the runtime behavior changes, increment spec_version
    // and set impl_version to 0. If only runtime
    // implementation changes and behavior does not, then leave spec_version as
    // is and increment impl_version.
    spec_version: 2018,
    impl_version: 0,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 7,
};

parameter_types! {
    /// Assume 10% of weight for average on_initialize calls.
    pub MaximumExtrinsicWeight: Weight = AvailableBlockRatio::get()
        .saturating_sub(Perbill::from_percent(10)) * MaximumBlockWeight::get();
    pub const Version: RuntimeVersion = VERSION;

    // Frame:
    pub const EpochDuration: u64 = EPOCH_DURATION_IN_BLOCKS as u64;
    pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;

    // Base:
    pub const MaxLen: u32 = 2048;

    // Indices:
    pub const IndexDeposit: Balance = DOLLARS;

    // Balances:
    pub const ExistentialDeposit: Balance = 0u128;
    pub const MaxLocks: u32 = 50;

    // Timestamp:
    pub const MinimumPeriod: Moment = SLOT_DURATION / 2;

    // Authorship:
    pub const UncleGenerations: BlockNumber = 0;

    // Session:
    // NOTE: `SessionHandler` and `SessionKeys` are co-dependent:
    // One key will be used for each handler.
    // The number and order of items in `SessionHandler` *MUST* be the same number
    // and order of keys in `SessionKeys`.
    // TODO: Introduce some structure to tie these together to make it a bit less of a footgun.
    // This should be easy, since OneSessionHandler trait provides the `Key` as an associated type. #2858
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);

    /*
    // Contracts:
    pub const NetworkShareInFee: Perbill = Perbill::from_percent(60);
    pub const TombstoneDeposit: Balance = 0;
    pub const RentByteFee: Balance = 0; // Assigning zero to switch off the rent logic in the contracts;
    pub const RentDepositOffset: Balance = 300 * DOLLARS;
    /// Reward that is received by the party whose touch has led
    /// to removal of a contract.
    pub const SurchargeReward: Balance = 150 * DOLLARS;
    */

    // Settlement:
    pub const MaxLegsInInstruction: u32 = 10;

    // Offences:
    pub OffencesWeightSoftLimit: Weight = Perbill::from_percent(60) * MaximumBlockWeight::get();

    // I'm online:
    pub const SessionDuration: BlockNumber = EPOCH_DURATION_IN_SLOTS as _;
    pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();

    // Assets:
    pub const MaxNumberOfTMExtensionForAsset: u32 = 5;
    pub const AssetNameMaxLength: u32 = 128;
    pub const FundingRoundNameMaxLength: u32 = 128;

    // Compliance manager:
    pub const MaxConditionComplexity: u32 = 50;

    // Corporate Actions:
    pub const MaxTargetIds: u32 = 1000;
    pub const MaxDidWhts: u32 = 1000;

    // Statistics:
    pub const MaxTransferManagersPerAsset: u32 = 3;

    // Scheduler:
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * MaximumBlockWeight::get();
    pub const MaxScheduledPerBlock: u32 = 50;

    // Identity:
    pub const InitialPOLYX: Balance = 100_000 * POLY;

    /*
    /// The fraction of the deposit that should be used as rent per block.
    pub RentFraction: Perbill = Perbill::from_rational_approximation(1u32, 30 * DAYS);
    // The lazy deletion runs inside on_initialize.
    pub DeletionWeightLimit: Weight = AVERAGE_ON_INITIALIZE_RATIO *
        RuntimeBlockWeights::get().max_block;
    // The weight needed for decoding the queue should be less or equal than a fifth
    // of the overall weight dedicated to the lazy deletion.
    pub DeletionQueueDepth: u32 = ((DeletionWeightLimit::get() / (
                <Runtime as pallet_contracts::Config>::WeightInfo::on_initialize_per_queue_item(1) -
                <Runtime as pallet_contracts::Config>::WeightInfo::on_initialize_per_queue_item(0)
                )) / 5) as u32;
    */
}

/// Splits fees 80/20 between treasury and block author.
pub type DealWithFees = SplitTwoWays<
    Balance,
    NegativeImbalance<Runtime>,
    _4,
    Treasury, // 4 parts (80%) goes to the treasury.
    _1,
    Author<Runtime>, // 1 part (20%) goes to the block author.
>;

// Staking:
pallet_staking_reward_curve::build! {
    const REWARD_CURVE: PiecewiseLinear<'_> = curve!(
        min_inflation: 0_025_000,
        max_inflation: 0_140_000,
        ideal_stake: 0_700_000,
        falloff: 0_050_000,
        max_piece_count: 40,
        test_precision: 0_005_000,
    );
}
parameter_types! {
    pub const SessionsPerEra: sp_staking::SessionIndex = 6;
    pub const BondingDuration: pallet_staking::EraIndex = 28;
    pub const SlashDeferDuration: pallet_staking::EraIndex = 14; // 1/4 the bonding duration.
    pub const RewardCurve: &'static PiecewiseLinear<'static> = &REWARD_CURVE;
    pub const MaxNominatorRewardedPerValidator: u32 = 2048;
    pub const ElectionLookahead: BlockNumber = EPOCH_DURATION_IN_BLOCKS / 4;
    pub const MaxIterations: u32 = 10;
    pub const MaxValidatorPerIdentity: Permill = Permill::from_percent(33);
    // 0.05%. The higher the value, the more strict solution acceptance becomes.
    pub MinSolutionScoreBump: Perbill = Perbill::from_rational_approximation(5u32, 10_000);
    pub const MaxVariableInflationTotalIssuance: Balance = 1_000_000_000 * POLY;
    pub const FixedYearlyReward: Balance = 140_000_000 * POLY;
    pub const MinimumBond: Balance = 1 * POLY;
    /// We prioritize im-online heartbeats over election solution submission.
    pub const StakingUnsignedPriority: TransactionPriority = TransactionPriority::max_value() / 2;

    pub const ReportLongevity: u64 =
        BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
}

polymesh_runtime_common::misc_pallet_impls!();

impl polymesh_common_utilities::traits::identity::Config for Runtime {
    type Event = Event;
    type Proposal = Call;
    type MultiSig = MultiSig;
    type Portfolio = Portfolio;
    type CddServiceProviders = CddServiceProviders;
    type Balances = pallet_balances::Module<Runtime>;
    type ChargeTxFeeTarget = TransactionPayment;
    type CddHandler = CddHandler;
    type Public = <MultiSignature as Verify>::Signer;
    type OffChainSignature = MultiSignature;
    type ProtocolFee = pallet_protocol_fee::Module<Runtime>;
    type GCVotingMajorityOrigin = VMO<GovernanceCommittee>;
    type WeightInfo = polymesh_weights::pallet_identity::WeightInfo;
    type IdentityFn = pallet_identity::Module<Runtime>;
    type SchedulerOrigin = OriginCaller;
    type InitialPOLYX = InitialPOLYX;
}

impl pallet_committee::Config<GovernanceCommittee> for Runtime {
    type CommitteeOrigin = VMO<GovernanceCommittee>;
    type VoteThresholdOrigin = Self::CommitteeOrigin;
    type Event = Event;
    type WeightInfo = polymesh_weights::pallet_committee::WeightInfo;
}

/// PolymeshCommittee as an instance of group
impl pallet_group::Config<pallet_group::Instance1> for Runtime {
    type Event = Event;
    type LimitOrigin = polymesh_primitives::EnsureRoot;
    type AddOrigin = Self::LimitOrigin;
    type RemoveOrigin = Self::LimitOrigin;
    type SwapOrigin = Self::LimitOrigin;
    type ResetOrigin = Self::LimitOrigin;
    type MembershipInitialized = PolymeshCommittee;
    type MembershipChanged = PolymeshCommittee;
    type WeightInfo = polymesh_weights::pallet_group::WeightInfo;
}

macro_rules! committee_config {
    ($committee:ident, $instance:ident) => {
        impl pallet_committee::Config<pallet_committee::$instance> for Runtime {
            // Can act upon itself.
            type CommitteeOrigin = VMO<pallet_committee::$instance>;
            type VoteThresholdOrigin = Self::CommitteeOrigin;
            type Event = Event;
            type WeightInfo = polymesh_weights::pallet_committee::WeightInfo;
        }
        impl pallet_group::Config<pallet_group::$instance> for Runtime {
            type Event = Event;
            // Committee cannot alter its own active membership limit.
            type LimitOrigin = polymesh_primitives::EnsureRoot;
            // Can manage its own addition, deletion, and swapping of membership...
            type AddOrigin = VMO<pallet_committee::$instance>;
            type RemoveOrigin = Self::AddOrigin;
            type SwapOrigin = Self::AddOrigin;
            // ...but it cannot reset its own membership; GC needs to do that.
            type ResetOrigin = VMO<GovernanceCommittee>;
            type MembershipInitialized = $committee;
            type MembershipChanged = $committee;
            type WeightInfo = polymesh_weights::pallet_group::WeightInfo;
        }
    };
}

committee_config!(TechnicalCommittee, Instance3);
committee_config!(UpgradeCommittee, Instance4);

impl pallet_pips::Config for Runtime {
    type Currency = Balances;
    type VotingMajorityOrigin = VMO<GovernanceCommittee>;
    type GovernanceCommittee = PolymeshCommittee;
    type TechnicalCommitteeVMO = VMO<pallet_committee::Instance3>;
    type UpgradeCommitteeVMO = VMO<pallet_committee::Instance4>;
    type Event = Event;
    type WeightInfo = polymesh_weights::pallet_pips::WeightInfo;
    type Scheduler = Scheduler;
}

/// CddProviders instance of group
impl pallet_group::Config<pallet_group::Instance2> for Runtime {
    type Event = Event;
    type LimitOrigin = polymesh_primitives::EnsureRoot;
    type AddOrigin = polymesh_primitives::EnsureRoot;
    type RemoveOrigin = polymesh_primitives::EnsureRoot;
    type SwapOrigin = polymesh_primitives::EnsureRoot;
    type ResetOrigin = polymesh_primitives::EnsureRoot;
    type MembershipInitialized = Identity;
    type MembershipChanged = Identity;
    type WeightInfo = polymesh_weights::pallet_group::WeightInfo;
}

construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = polymesh_primitives::Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>} = 0,
        Babe: pallet_babe::{Module, Call, Storage, Config, ValidateUnsigned} = 1,
        Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent} = 2,
        Indices: pallet_indices::{Module, Call, Storage, Config<T>, Event<T>} = 3,
        Authorship: pallet_authorship::{Module, Call, Storage, Inherent} = 4,

        // Balance: Genesis config dependencies: System.
        Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>} = 5,

        // TransactionPayment: Genesis config dependencies: Balance.
        TransactionPayment: pallet_transaction_payment::{Module, Storage} = 6,

        // Identity: Genesis config deps: Timestamp.
        Identity: pallet_identity::{Module, Call, Storage, Event<T>, Config<T>} = 7,

        // Polymesh Committees
        // CddServiceProviders: Genesis config deps: Identity
        CddServiceProviders: pallet_group::<Instance2>::{Module, Call, Storage, Event<T>, Config<T>} = 8,

        PolymeshCommittee: pallet_committee::<Instance1>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>} = 10,
        // CommitteeMembership: Genesis config deps: PolymeshCommittee, Identity.
        CommitteeMembership: pallet_group::<Instance1>::{Module, Call, Storage, Event<T>, Config<T>} = 9,
        TechnicalCommittee: pallet_committee::<Instance3>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>} = 11,
        // TechnicalCommitteeMembership: Genesis config deps: TechnicalCommittee, Identity
        TechnicalCommitteeMembership: pallet_group::<Instance3>::{Module, Call, Storage, Event<T>, Config<T>} = 12,
        UpgradeCommittee: pallet_committee::<Instance4>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>} = 13,
        // UpgradeCommitteeMembership: Genesis config deps: UpgradeCommittee, Identity
        UpgradeCommitteeMembership: pallet_group::<Instance4>::{Module, Call, Storage, Event<T>, Config<T>} = 14,

        MultiSig: pallet_multisig::{Module, Call, Config, Storage, Event<T>} = 15,
        // Bridge: Genesis config deps: Multisig, Identity, Committees
        Bridge: pallet_bridge::{Module, Call, Storage, Config<T>, Event<T>} = 16,

        // Staking: Genesis config deps: Bridge, Balances, Indices, Identity, Babe, Timestamp, Committees
        Staking: pallet_staking::{Module, Call, Config<T>, Storage, Event<T>, ValidateUnsigned} = 17,
        Offences: pallet_offences::{Module, Call, Storage, Event} = 18,

        // Session: Genesis config deps: System.
        Session: pallet_session::{Module, Call, Storage, Event, Config<T>} = 19,
        AuthorityDiscovery: pallet_authority_discovery::{Module, Call, Config} = 20,
        Grandpa: pallet_grandpa::{Module, Call, Storage, Config, Event} = 22,
        Historical: pallet_session_historical::{Module} = 23,
        ImOnline: pallet_im_online::{Module, Call, Storage, Event<T>, ValidateUnsigned, Config<T>} = 24,
        RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Module, Call, Storage} = 25,

        // Sudo. Usable initially.
        Sudo: pallet_sudo::{Module, Call, Config<T>, Storage, Event<T>} = 26,

        /*
        // Contracts
        BaseContracts: pallet_contracts::{Module, Config<T>, Storage, Event<T>} = 27,
        Contracts: polymesh_contracts::{Module, Call, Storage, Event<T>} = 28,
        */

        // Asset: Genesis config deps: Timestamp,
        Asset: pallet_asset::{Module, Call, Storage, Config<T>, Event<T>} = 29,
        CapitalDistribution: pallet_capital_distribution::{Module, Call, Storage, Event<T>} = 30,
        Checkpoint: pallet_checkpoint::{Module, Call, Storage, Event<T>, Config} = 31,
        ComplianceManager: pallet_compliance_manager::{Module, Call, Storage, Event} = 32,
        CorporateAction: pallet_corporate_actions::{Module, Call, Storage, Event, Config} = 33,
        CorporateBallot: pallet_corporate_ballot::{Module, Call, Storage, Event<T>} = 34,
        Permissions: pallet_permissions::{Module} = 35,
        Pips: pallet_pips::{Module, Call, Storage, Event<T>, Config<T>} = 36,
        Portfolio: pallet_portfolio::{Module, Call, Storage, Event<T>} = 37,
        ProtocolFee: pallet_protocol_fee::{Module, Call, Storage, Event<T>, Config<T>} = 38,
        Scheduler: pallet_scheduler::{Module, Call, Storage, Event<T>} = 39,
        Settlement: pallet_settlement::{Module, Call, Storage, Event<T>, Config} = 40,
        Statistics: pallet_statistics::{Module, Call, Storage, Event} = 41,
        Sto: pallet_sto::{Module, Call, Storage, Event<T>} = 42,
        Treasury: pallet_treasury::{Module, Call, Event<T>} = 43,
        Utility: pallet_utility::{Module, Call, Storage, Event} = 44,
        Base: pallet_base::{Module, Call, Event} = 45,
        ExternalAgents: pallet_external_agents::{Module, Call, Storage, Event} = 46,
        Relayer: pallet_relayer::{Module, Call, Storage, Event<T>} = 47,
    }
);

polymesh_runtime_common::runtime_apis! {}
