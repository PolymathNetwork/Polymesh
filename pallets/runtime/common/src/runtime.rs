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
pub type VMO<Instance> =
    pallet_committee::EnsureThresholdMet<polymesh_primitives::AccountId, Instance>;

pub type GovernanceCommittee = pallet_committee::Instance1;

#[macro_export]
macro_rules! misc2 {
    () => {
        impl pallet_authority_discovery::Trait for Runtime {}

        impl pallet_finality_tracker::Trait for Runtime {
            type OnFinalizationStalled = ();
            type WindowSize = WindowSize;
            type ReportLatency = ReportLatency;
        }

        impl pallet_sudo::Trait for Runtime {
            type Event = Event;
            type Call = Call;
        }

        impl pallet_multisig::Trait for Runtime {
            type Event = Event;
            type Scheduler = Scheduler;
            type SchedulerCall = Call;
            type WeightInfo = polymesh_weights::pallet_multisig::WeightInfo;
        }

        impl pallet_bridge::Trait for Runtime {
            type Event = Event;
            type Proposal = Call;
            type Scheduler = Scheduler;
        }

        impl pallet_portfolio::Trait for Runtime {
            type Event = Event;
            type WeightInfo = polymesh_weights::pallet_portfolio::WeightInfo;
        }

        impl polymesh_common_utilities::traits::identity::Trait for Runtime {
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
            type CorporateAction = CorporateAction;
            type IdentityFn = pallet_identity::Module<Runtime>;
            type SchedulerOrigin = OriginCaller;
        }

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
            type DetermineContractAddress =
                polymesh_contracts::NonceBasedAddressDeterminer<Runtime>;
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

        impl pallet_offences::Trait for Runtime {
            type Event = Event;
            type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
            type OnOffenceHandler = Staking;
            type WeightSoftLimit = OffencesWeightSoftLimit;
        }

        type GrandpaKey = (sp_core::crypto::KeyTypeId, GrandpaId);

        impl pallet_im_online::Trait for Runtime {
            type AuthorityId = pallet_im_online::sr25519::AuthorityId;
            type Event = Event;
            type UnsignedPriority = ImOnlineUnsignedPriority;
            type ReportUnresponsiveness = Offences;
            type SessionDuration = SessionDuration;
            type WeightInfo = polymesh_weights::pallet_im_online::WeightInfo;
        }

        impl pallet_grandpa::Trait for Runtime {
            type WeightInfo = polymesh_weights::pallet_grandpa::WeightInfo;
            type Event = Event;
            type Call = Call;

            type KeyOwnerProofSystem = Historical;

            type KeyOwnerProof =
                <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<GrandpaKey>>::Proof;

            type KeyOwnerIdentification =
                <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<GrandpaKey>>::IdentificationTuple;

            type HandleEquivocation =
                pallet_grandpa::EquivocationHandler<Self::KeyOwnerIdentification, Offences>;
        }

        impl pallet_treasury::Trait for Runtime {
            type Event = Event;
            type Currency = Balances;
            type WeightInfo = polymesh_weights::pallet_treasury::WeightInfo;
        }

        impl pallet_settlement::Trait for Runtime {
            type Event = Event;
            type MaxLegsInInstruction = MaxLegsInInstruction;
            type Scheduler = Scheduler;
            type WeightInfo = polymesh_weights::pallet_settlement::WeightInfo;
        }

        impl pallet_sto::Trait for Runtime {
            type Event = Event;
            type WeightInfo = polymesh_weights::pallet_sto::WeightInfo;
        }

        impl PermissionChecker for Runtime {
            type Call = Call;
            type Checker = Identity;
        }

        impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
        where
            Call: From<LocalCall>,
        {
            fn create_transaction<
                C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>,
            >(
                call: Call,
                public: <Signature as Verify>::Signer,
                account: AccountId,
                nonce: Index,
            ) -> Option<(Call, <UncheckedExtrinsic as Extrinsic>::SignaturePayload)> {
                // take the biggest period possible.
                let period = BlockHashCount::get()
                    .checked_next_power_of_two()
                    .map(|c| c / 2)
                    .unwrap_or(2) as u64;
                let current_block = System::block_number()
                    .saturated_into::<u64>()
                    // The `System::block_number` is initialized with `n+1`,
                    // so the actual block number is `n`.
                    .saturating_sub(1);
                let tip = 0;
                let extra: SignedExtra = (
                    frame_system::CheckSpecVersion::new(),
                    frame_system::CheckTxVersion::new(),
                    frame_system::CheckGenesis::new(),
                    frame_system::CheckEra::from(generic::Era::mortal(period, current_block)),
                    frame_system::CheckNonce::from(nonce),
                    frame_system::CheckWeight::new(),
                    pallet_transaction_payment::ChargeTransactionPayment::from(tip),
                    pallet_permissions::StoreCallMetadata::new(),
                );
                let raw_payload = SignedPayload::new(call, extra)
                    .map_err(|e| {
                        debug::warn!("Unable to create signed payload: {:?}", e);
                    })
                    .ok()?;
                let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
                let address = Indices::unlookup(account);
                let (call, extra, _) = raw_payload.deconstruct();
                Some((call, (address, signature, extra)))
            }
        }

        impl frame_system::offchain::SigningTypes for Runtime {
            type Public = <Signature as Verify>::Signer;
            type Signature = Signature;
        }

        impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
        where
            Call: From<C>,
        {
            type Extrinsic = UncheckedExtrinsic;
            type OverarchingCall = Call;
        }
    };
}

#[macro_export]
macro_rules! runtime_apis {
    ($($extra:item)*) => {
        /// The address format for describing accounts.
        pub type Address = <Indices as StaticLookup>::Source;
        /// Block header type as expected by this runtime.
        pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
        /// Block type as expected by this runtime.
        pub type Block = generic::Block<Header, UncheckedExtrinsic>;
        /// A Block signed with a Justification
        pub type SignedBlock = generic::SignedBlock<Block>;
        /// BlockId type as expected by this runtime.
        pub type BlockId = generic::BlockId<Block>;
        /// The SignedExtension to the basic transaction logic.
        pub type SignedExtra = (
            frame_system::CheckSpecVersion<Runtime>,
            frame_system::CheckTxVersion<Runtime>,
            frame_system::CheckGenesis<Runtime>,
            frame_system::CheckEra<Runtime>,
            frame_system::CheckNonce<Runtime>,
            polymesh_extensions::CheckWeight<Runtime>
            pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
            pallet_permissions::StoreCallMetadata<Runtime>,
        );
        /// Unchecked extrinsic type as expected by this runtime.
        pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
        /// The payload being signed in transactions.
        pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;
        /// Extrinsic type that has already been checked.
        pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;
        /// Executive: handles dispatch to the various modules.
        pub type Executive = pallet_executive::Executive<
            Runtime,
            Block,
            frame_system::ChainContext<Runtime>,
            Runtime,
            AllModules,
        >;

        impl_runtime_apis! {
            impl sp_api::Core<Block> for Runtime {
                fn version() -> RuntimeVersion {
                    VERSION
                }

                fn execute_block(block: Block) {
                    Executive::execute_block(block)
                }

                fn initialize_block(header: &<Block as BlockT>::Header) {
                    Executive::initialize_block(header)
                }
            }

            impl sp_api::Metadata<Block> for Runtime {
                fn metadata() -> OpaqueMetadata {
                    Runtime::metadata().into()
                }
            }

            impl sp_block_builder::BlockBuilder<Block> for Runtime {
                fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
                    Executive::apply_extrinsic(extrinsic)
                }

                fn finalize_block() -> <Block as BlockT>::Header {
                    Executive::finalize_block()
                }

                fn inherent_extrinsics(data: InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
                    data.create_extrinsics()
                }

                fn check_inherents(block: Block, data: InherentData) -> CheckInherentsResult {
                    data.check_extrinsics(&block)
                }

                fn random_seed() -> <Block as BlockT>::Hash {
                    RandomnessCollectiveFlip::random_seed()
                }
            }

            impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
                fn validate_transaction(
                    source: TransactionSource,
                    tx: <Block as BlockT>::Extrinsic,
                ) -> TransactionValidity {
                    Executive::validate_transaction(source, tx)
                }
            }

            impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
                fn offchain_worker(header: &<Block as BlockT>::Header) {
                    Executive::offchain_worker(header)
                }
            }

            impl fg_primitives::GrandpaApi<Block> for Runtime {
                fn grandpa_authorities() -> GrandpaAuthorityList {
                    Grandpa::grandpa_authorities()
                }

                fn submit_report_equivocation_unsigned_extrinsic(
                    equivocation_proof: fg_primitives::EquivocationProof<
                        <Block as BlockT>::Hash,
                        NumberFor<Block>,
                    >,
                    key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
                ) -> Option<()> {
                    let key_owner_proof = key_owner_proof.decode()?;

                    Grandpa::submit_unsigned_equivocation_report(
                        equivocation_proof,
                        key_owner_proof,
                    )
                }

                fn generate_key_ownership_proof(
                    _set_id: fg_primitives::SetId,
                    authority_id: GrandpaId,
                ) -> Option<fg_primitives::OpaqueKeyOwnershipProof> {
                    use codec::Encode;

                    Historical::prove((fg_primitives::KEY_TYPE, authority_id))
                        .map(|p| p.encode())
                        .map(fg_primitives::OpaqueKeyOwnershipProof::new)
                }
            }

            impl sp_consensus_babe::BabeApi<Block> for Runtime {
                fn configuration() -> sp_consensus_babe::BabeGenesisConfiguration {
                    // The choice of `c` parameter (where `1 - c` represents the
                    // probability of a slot being empty), is done in accordance to the
                    // slot duration and expected target block time, for safely
                    // resisting network delays of maximum two seconds.
                    // <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
                    sp_consensus_babe::BabeGenesisConfiguration {
                        slot_duration: Babe::slot_duration(),
                        epoch_length: EpochDuration::get(),
                        c: PRIMARY_PROBABILITY,
                        genesis_authorities: Babe::authorities(),
                        randomness: Babe::randomness(),
                        allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryPlainSlots,
                    }
                }

                fn current_epoch_start() -> sp_consensus_babe::SlotNumber {
                    Babe::current_epoch_start()
                }

                fn generate_key_ownership_proof(
                    _slot_number: sp_consensus_babe::SlotNumber,
                    authority_id: sp_consensus_babe::AuthorityId,
                ) -> Option<sp_consensus_babe::OpaqueKeyOwnershipProof> {
                    use codec::Encode;

                    Historical::prove((sp_consensus_babe::KEY_TYPE, authority_id))
                        .map(|p| p.encode())
                        .map(sp_consensus_babe::OpaqueKeyOwnershipProof::new)
                }

                fn submit_report_equivocation_unsigned_extrinsic(
                    equivocation_proof: sp_consensus_babe::EquivocationProof<<Block as BlockT>::Header>,
                    key_owner_proof: sp_consensus_babe::OpaqueKeyOwnershipProof,
                ) -> Option<()> {
                    let key_owner_proof = key_owner_proof.decode()?;

                    Babe::submit_unsigned_equivocation_report(
                        equivocation_proof,
                        key_owner_proof,
                    )
                }
            }

            impl sp_authority_discovery::AuthorityDiscoveryApi<Block> for Runtime {
                fn authorities() -> Vec<AuthorityDiscoveryId> {
                    AuthorityDiscovery::authorities()
                }
            }

            impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
                fn account_nonce(account: AccountId) -> Index {
                    System::account_nonce(account)
                }
            }

            impl pallet_contracts_rpc_runtime_api::ContractsApi<Block, AccountId, Balance, BlockNumber>
                for Runtime
            {
                fn call(
                    origin: AccountId,
                    dest: AccountId,
                    value: Balance,
                    gas_limit: u64,
                    input_data: Vec<u8>,
                ) -> ContractExecResult {
                    let (exec_result, gas_consumed) =
                    BaseContracts::bare_call(origin, dest.into(), value, gas_limit, input_data);
                    match exec_result {
                        Ok(v) => ContractExecResult::Success {
                            flags: v.flags.bits(),
                            data: v.data,
                            gas_consumed: gas_consumed,
                        },
                        Err(_) => ContractExecResult::Error,
                    }
                }

                fn get_storage(
                    address: AccountId,
                    key: [u8; 32],
                ) -> pallet_contracts_primitives::GetStorageResult {
                    BaseContracts::get_storage(address, key)
                }

                fn rent_projection(
                    address: AccountId,
                ) -> pallet_contracts_primitives::RentProjectionResult<BlockNumber> {
                    BaseContracts::rent_projection(address)
                }
            }

            impl node_rpc_runtime_api::transaction_payment::TransactionPaymentApi<
                Block,
                Balance,
                UncheckedExtrinsic,
            > for Runtime {
                fn query_info(uxt: UncheckedExtrinsic, len: u32) -> RuntimeDispatchInfo<Balance> {
                    TransactionPayment::query_info(uxt, len)
                }
            }

            impl sp_session::SessionKeys<Block> for Runtime {
                fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
                    SessionKeys::generate(seed)
                }

                fn decode_session_keys(
                    encoded: Vec<u8>,
                ) -> Option<Vec<(Vec<u8>, sp_core::crypto::KeyTypeId)>> {
                    SessionKeys::decode_into_raw_public_keys(&encoded)
                }
            }

            impl pallet_staking_rpc_runtime_api::StakingApi<Block> for Runtime {
                fn get_curve() -> Vec<(Perbill, Perbill)> {
                    Staking::get_curve()
                }
            }

            impl node_rpc_runtime_api::pips::PipsApi<Block, AccountId, Balance>
            for Runtime
            {
                /// Get vote count for a given proposal index
                fn get_votes(index: u32) -> VoteCount<Balance> {
                    Pips::get_votes(index)
                }

                /// Proposals voted by `address`
                fn proposed_by(address: AccountId) -> Vec<u32> {
                    Pips::proposed_by(pallet_pips::Proposer::Community(address))
                }

                /// Proposals `address` voted on
                fn voted_on(address: AccountId) -> Vec<u32> {
                    Pips::voted_on(address)
                }

                /// Retrieve PIPs voted on information by `address` account.
                fn voting_history_by_address(address: AccountId) -> HistoricalVotingByAddress<Vote<Balance>> {
                    Pips::voting_history_by_address(address)

                }

                /// Retrieve PIPs voted on information by `id` identity (and its secondary items).
                fn voting_history_by_id(id: IdentityId) -> HistoricalVotingById<AccountId, Vote<Balance>> {
                    Pips::voting_history_by_id(id)
                }
            }

            impl pallet_protocol_fee_rpc_runtime_api::ProtocolFeeApi<
                Block,
            > for Runtime {
                fn compute_fee(op: ProtocolOp) -> CappedFee {
                    ProtocolFee::compute_fee(&[op]).into()
                }
            }

            impl
                node_rpc_runtime_api::identity::IdentityApi<
                    Block,
                    IdentityId,
                    Ticker,
                    AccountId,
                    SecondaryKey<AccountId>,
                    Signatory<AccountId>,
                    Moment
                > for Runtime
            {
                /// RPC call to know whether the given did has valid cdd claim or not
                fn is_identity_has_valid_cdd(did: IdentityId, leeway: Option<u64>) -> CddStatus {
                    Identity::fetch_cdd(did, leeway.unwrap_or_default())
                        .ok_or_else(|| "Either cdd claim is expired or not yet provided to give identity".into())
                }

                /// RPC call to query the given ticker did
                fn get_asset_did(ticker: Ticker) -> AssetDidResult {
                    Identity::get_asset_did(ticker)
                        .map_err(|_| "Error in computing the given ticker error".into())
                }

                /// Retrieve primary key and secondary keys for a given IdentityId
                fn get_did_records(did: IdentityId) -> DidRecords<AccountId, SecondaryKey<AccountId>> {
                    Identity::get_did_records(did)
                }

                /// Retrieve the status of the DIDs
                fn get_did_status(dids: Vec<IdentityId>) -> Vec<DidStatus> {
                    Identity::get_did_status(dids)
                }

                fn get_key_identity_data(acc: AccountId) -> Option<KeyIdentityData<IdentityId>> {
                    Identity::get_key_identity_data(acc)
                }

                /// Retrieve list of a authorization for a given signatory
                fn get_filtered_authorizations(
                    signatory: Signatory<AccountId>,
                    allow_expired: bool,
                    auth_type: Option<AuthorizationType>
                ) -> Vec<Authorization<AccountId, Moment>> {
                    Identity::get_filtered_authorizations(signatory, allow_expired, auth_type)
                }
            }

            impl node_rpc_runtime_api::asset::AssetApi<Block, AccountId> for Runtime {
                #[inline]
                fn can_transfer(
                    _sender: AccountId,
                    from_custodian: Option<IdentityId>,
                    from_portfolio: PortfolioId,
                    to_custodian: Option<IdentityId>,
                    to_portfolio: PortfolioId,
                    ticker: &Ticker,
                    value: Balance) -> node_rpc_runtime_api::asset::CanTransferResult
                {
                    Asset::unsafe_can_transfer(from_custodian, from_portfolio, to_custodian, to_portfolio, ticker, value)
                        .map_err(|msg| msg.as_bytes().to_vec())
                }
            }

            impl node_rpc_runtime_api::compliance_manager::ComplianceManagerApi<Block, AccountId, Balance>
                for Runtime
            {
                #[inline]
                fn can_transfer(
                    ticker: Ticker,
                    from_did: Option<IdentityId>,
                    to_did: Option<IdentityId>,
                ) -> AssetComplianceResult
                {
                    ComplianceManager::granular_verify_restriction(&ticker, from_did, to_did)
                }
            }

            impl pallet_group_rpc_runtime_api::GroupApi<Block> for Runtime {
                fn get_cdd_valid_members() -> Vec<pallet_group_rpc_runtime_api::Member> {
                    merge_active_and_inactive::<Block>(
                        CddServiceProviders::active_members(),
                        CddServiceProviders::inactive_members())
                }

                fn get_gc_valid_members() -> Vec<pallet_group_rpc_runtime_api::Member> {
                    merge_active_and_inactive::<Block>(
                        CommitteeMembership::active_members(),
                        CommitteeMembership::inactive_members())
                }
            }

            $($extra)*
        }
    }
}
