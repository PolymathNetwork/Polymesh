#[macro_export]
macro_rules! misc1 {
    () => {
        impl frame_system::Trait for Runtime {
            /// The basic call filter to use in dispatchable.
            type BaseCallFilter = ();
            /// The identifier used to distinguish between accounts.
            type AccountId = polymesh_primitives::AccountId;
            /// The aggregated dispatch type that is available for extrinsics.
            type Call = Call;
            /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
            type Lookup = Indices;
            /// The index type for storing how many extrinsics an account has signed.
            type Index = polymesh_primitives::Index;
            /// The index type for blocks.
            type BlockNumber = polymesh_primitives::BlockNumber;
            /// The type for hashing blocks and tries.
            type Hash = polymesh_primitives::Hash;
            /// The hashing algorithm used.
            type Hashing = sp_runtime::traits::BlakeTwo256;
            /// The header type.
            type Header =
                sp_runtime::generic::Header<polymesh_primitives::BlockNumber, BlakeTwo256>;
            /// The ubiquitous event type.
            type Event = Event;
            /// The ubiquitous origin type.
            type Origin = Origin;
            /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
            type BlockHashCount = polymesh_runtime_common::BlockHashCount;
            /// Maximum weight of each block.
            type MaximumBlockWeight = polymesh_runtime_common::MaximumBlockWeight;
            /// The weight of database operations that the runtime can invoke.
            type DbWeight = polymesh_runtime_common::RocksDbWeight;
            /// The weight of the overhead invoked on the block import process, independent of the
            /// extrinsics included in that block.
            type BlockExecutionWeight = polymesh_runtime_common::BlockExecutionWeight;
            /// The base weight of any extrinsic processed by the runtime, independent of the
            /// logic of that extrinsic. (Signature verification, nonce increment, fee, etc...)
            type ExtrinsicBaseWeight = polymesh_runtime_common::ExtrinsicBaseWeight;
            /// The maximum weight that a single extrinsic of `Normal` dispatch class can have,
            /// idependent of the logic of that extrinsics. (Roughly max block weight - average on
            /// initialize cost).
            type MaximumExtrinsicWeight = MaximumExtrinsicWeight;
            /// Maximum size of all encoded transactions (in bytes) that are allowed in one block.
            type MaximumBlockLength = polymesh_runtime_common::MaximumBlockLength;
            /// Portion of the block weight that is available to all normal transactions.
            type AvailableBlockRatio = polymesh_runtime_common::AvailableBlockRatio;
            /// Version of the runtime.
            type Version = Version;
            /// Converts a module to the index of the module in `construct_runtime!`.
            ///
            /// This type is being generated by `construct_runtime!`.
            type PalletInfo = PalletInfo;
            /// What to do if a new account is created.
            type OnNewAccount = ();
            /// What to do if an account is fully reaped from the system.
            type OnKilledAccount = ();
            /// The data to be stored in an account.
            type AccountData = polymesh_common_utilities::traits::balances::AccountData<
                polymesh_primitives::Balance,
            >;
            type SystemWeightInfo = polymesh_weights::frame_system::WeightInfo;
        }

        impl pallet_babe::Trait for Runtime {
            type WeightInfo = polymesh_weights::pallet_babe::WeightInfo;
            type EpochDuration = EpochDuration;
            type ExpectedBlockTime = ExpectedBlockTime;
            type EpochChangeTrigger = pallet_babe::ExternalTrigger;

            type KeyOwnerProofSystem = Historical;

            type KeyOwnerProof = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
                sp_core::crypto::KeyTypeId,
                pallet_babe::AuthorityId,
            )>>::Proof;

            type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
                sp_core::crypto::KeyTypeId,
                pallet_babe::AuthorityId,
            )>>::IdentificationTuple;

            type HandleEquivocation =
                pallet_babe::EquivocationHandler<Self::KeyOwnerIdentification, Offences>;
        }

        impl pallet_indices::Trait for Runtime {
            type AccountIndex = polymesh_primitives::AccountIndex;
            type Currency = Balances;
            type Deposit = IndexDeposit;
            type Event = Event;
            type WeightInfo = polymesh_weights::pallet_indices::WeightInfo;
        }

        impl pallet_transaction_payment::Trait for Runtime {
            type Currency = Balances;
            type OnTransactionPayment = DealWithFees;
            type TransactionByteFee = TransactionByteFee;
            type WeightToFee = WeightToFee;
            type FeeMultiplierUpdate = ();
            type CddHandler = CddHandler;
        }

        impl polymesh_common_utilities::traits::CommonTrait for Runtime {
            type Balance = polymesh_primitives::Balance;
            type AssetSubTraitTarget = Asset;
            type BlockRewardsReserve = balances::Module<Runtime>;
        }

        impl pallet_balances::Trait for Runtime {
            type MaxLocks = MaxLocks;
            type DustRemoval = ();
            type Event = Event;
            type ExistentialDeposit = ExistentialDeposit;
            type AccountStore = frame_system::Module<Runtime>;
            type CddChecker = CddChecker<Runtime>;
            type WeightInfo = polymesh_weights::pallet_balances::WeightInfo;
        }

        impl pallet_protocol_fee::Trait for Runtime {
            type Event = Event;
            type Currency = Balances;
            type OnProtocolFeePayment = DealWithFees;
            type WeightInfo = polymesh_weights::pallet_protocol_fee::WeightInfo;
        }

        impl pallet_timestamp::Trait for Runtime {
            type Moment = polymesh_primitives::Moment;
            type OnTimestampSet = Babe;
            type MinimumPeriod = MinimumPeriod;
            type WeightInfo = polymesh_weights::pallet_timestamp::WeightInfo;
        }

        // TODO: substrate#2986 implement this properly
        impl pallet_authorship::Trait for Runtime {
            type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
            type UncleGenerations = UncleGenerations;
            type FilterUncle = ();
            type EventHandler = (Staking, ImOnline);
        }

        impl_opaque_keys! {
            pub struct SessionKeys {
                pub grandpa: Grandpa,
                pub babe: Babe,
                pub im_online: ImOnline,
                pub authority_discovery: AuthorityDiscovery,
            }
        }

        impl pallet_session::Trait for Runtime {
            type Event = Event;
            type ValidatorId = <Self as frame_system::Trait>::AccountId;
            type ValidatorIdOf = pallet_staking::StashOf<Self>;
            type ShouldEndSession = Babe;
            type NextSessionRotation = Babe;
            type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
            type SessionHandler =
                <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
            type Keys = SessionKeys;
            type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
            type WeightInfo = polymesh_weights::pallet_session::WeightInfo;
        }

        impl pallet_session::historical::Trait for Runtime {
            type FullIdentification = pallet_staking::Exposure<
                polymesh_primitives::AccountId,
                polymesh_primitives::Balance,
            >;
            type FullIdentificationOf = pallet_staking::ExposureOf<Runtime>;
        }

        impl pallet_staking::Trait for Runtime {
            type Currency = Balances;
            type UnixTime = Timestamp;
            type CurrencyToVote = CurrencyToVoteHandler<Self>;
            type RewardRemainder = ();
            type Event = Event;
            type Slash = Treasury; // send the slashed funds to the treasury.
            type Reward = (); // rewards are minted from the void
            type SessionsPerEra = SessionsPerEra;
            type BondingDuration = BondingDuration;
            type SlashDeferDuration = SlashDeferDuration;
            type SlashCancelOrigin = frame_system::EnsureRoot<polymesh_primitives::AccountId>;
            type SessionInterface = Self;
            type RewardCurve = RewardCurve;
            type NextNewSession = Session;
            type ElectionLookahead = ElectionLookahead;
            type Call = Call;
            type MaxIterations = MaxIterations;
            type MinSolutionScoreBump = MinSolutionScoreBump;
            type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
            type UnsignedPriority = StakingUnsignedPriority;
            type WeightInfo = ();
            type RequiredAddOrigin = Self::SlashCancelOrigin;
            type RequiredRemoveOrigin = Self::SlashCancelOrigin;
            type RequiredComplianceOrigin = Self::SlashCancelOrigin;
            type RequiredCommissionOrigin = Self::SlashCancelOrigin;
            type RequiredChangeHistoryDepthOrigin = Self::SlashCancelOrigin;
            type RewardScheduler = Scheduler;
            type MaxValidatorPerIdentity = MaxValidatorPerIdentity;
            type MaxVariableInflationTotalIssuance = MaxVariableInflationTotalIssuance;
            type FixedYearlyReward = FixedYearlyReward;
            type PalletsOrigin = OriginCaller;
        }
    };
}

/// Voting majority origin for `Instance`.
pub type VMO<Instance> = pallet_committee::EnsureThresholdMet<polymesh_primitives::AccountId, Instance>;

pub type GovernanceCommittee = pallet_committee::Instance1;

#[macro_export]
macro_rules! misc2 {
    () => {
        impl pallet_asset::Trait for Runtime {
            type Event = Event;
            type Currency = Balances;
            type ComplianceManager = pallet_compliance_manager::Module<Runtime>;
            type MaxNumberOfTMExtensionForAsset = MaxNumberOfTMExtensionForAsset;
            type UnixTime = pallet_timestamp::Module<Runtime>;
            type AssetNameMaxLength = AssetNameMaxLength;
            type FundingRoundNameMaxLength = FundingRoundNameMaxLength;
            type AssetFn = Asset;
            type AllowedGasLimit = AllowedGasLimit;
            type WeightInfo = polymesh_weights::pallet_asset::WeightInfo;
            type CPWeightInfo = polymesh_weights::pallet_checkpoint::WeightInfo;
        }

        impl polymesh_contracts::Trait for Runtime {
            type Event = Event;
            type NetworkShareInFee = NetworkShareInFee;
            type WeightInfo = polymesh_weights::polymesh_contracts::WeightInfo;
        }
        impl pallet_contracts::Trait for Runtime {
            type Time = Timestamp;
            type Randomness = RandomnessCollectiveFlip;
            type Currency = Balances;
            type Event = Event;
            type DetermineContractAddress = polymesh_contracts::NonceBasedAddressDeterminer<Runtime>;
            type TrieIdGenerator = pallet_contracts::TrieIdFromParentCounter<Runtime>;
            type RentPayment = ();
            type SignedClaimHandicap = pallet_contracts::DefaultSignedClaimHandicap;
            type TombstoneDeposit = TombstoneDeposit;
            type StorageSizeOffset = pallet_contracts::DefaultStorageSizeOffset;
            type RentByteFee = RentByteFee;
            type RentDepositOffset = RentDepositOffset;
            type SurchargeReward = SurchargeReward;
            type MaxDepth = pallet_contracts::DefaultMaxDepth;
            type MaxValueSize = pallet_contracts::DefaultMaxValueSize;
            type WeightPrice = pallet_transaction_payment::Module<Self>;
        }

        impl pallet_compliance_manager::Trait for Runtime {
            type Event = Event;
            type Asset = Asset;
            type WeightInfo = polymesh_weights::pallet_compliance_manager::WeightInfo;
            type MaxConditionComplexity = MaxConditionComplexity;
        }

        impl pallet_corporate_actions::Trait for Runtime {
            type Event = Event;
            type MaxTargetIds = MaxTargetIds;
            type MaxDidWhts = MaxDidWhts;
            type WeightInfo = polymesh_weights::pallet_corporate_actions::WeightInfo;
            type BallotWeightInfo = polymesh_weights::pallet_corporate_ballot::WeightInfo;
            type DistWeightInfo = polymesh_weights::pallet_capital_distribution::WeightInfo;
        }

        impl pallet_statistics::Trait for Runtime {
            type Event = Event;
            type Asset = Asset;
            type MaxTransferManagersPerAsset = MaxTransferManagersPerAsset;
            type WeightInfo = polymesh_weights::pallet_statistics::WeightInfo;
        }

        impl pallet_utility::Trait for Runtime {
            type Event = Event;
            type Call = Call;
            type WeightInfo = polymesh_weights::pallet_utility::WeightInfo;
        }

        impl pallet_scheduler::Trait for Runtime {
            type Event = Event;
            type Origin = Origin;
            type PalletsOrigin = OriginCaller;
            type Call = Call;
            type MaximumWeight = MaximumSchedulerWeight;
            type ScheduleOrigin = frame_system::EnsureRoot<polymesh_primitives::AccountId>;
            type MaxScheduledPerBlock = MaxScheduledPerBlock;
            type WeightInfo = polymesh_weights::pallet_scheduler::WeightInfo;
        }
    }
}
