use sp_runtime::ModuleId;
/// Money matters.
pub mod currency {
    use polymesh_primitives::Balance;
    // TODO: Define proper units. These are placeholders.
    pub const POLY: Balance = 1_000_000;
    pub const DOLLARS: Balance = POLY;
    pub const CENTS: Balance = DOLLARS / 100;
    pub const MILLICENTS: Balance = CENTS / 1_000;
    pub const ONE_UNIT: Balance = 1_000_000;
    pub const MAX_SUPPLY: Balance = ONE_UNIT * 1_000_000_000_000;
}

/// DID-related.
pub mod did {
    /// prefix for user dids
    pub const USER: &[u8; 5] = b"USER:";
    /// prefix for security token dids
    pub const SECURITY_TOKEN: &[u8; 15] = b"SECURITY_TOKEN:";

    /// Governance Committee DID. It is used in systematic CDD claim for Governance Committee members.
    pub const GOVERNANCE_COMMITTEE_DID: &[u8; 32] = b"system:governance_committee\0\0\0\0\0";
    /// CDD Providers DID. It is used in systematic CDD claim for CDD Providers.
    pub const CDD_PROVIDERS_DID: &[u8; 32] = b"system:customer_due_diligence\0\0\0";
    /// Treasury module DID. It is used in systematic CDD claim for the Treasury module.
    pub const TREASURY_DID: &[u8; 32] = b"system:treasury_module_did\0\0\0\0\0\0";
    /// Block Reward Reserve DID.
    pub const BLOCK_REWARD_RESERVE_DID: &[u8; 32] = b"system:block_reward_reserve_did\0";
    /// Settlement module DID
    pub const SETTLEMENT_MODULE_DID: &[u8; 32] = b"system:settlement_module_did\0\0\0\0";
    /// Polymath Classic / Ethereum migration DID.
    pub const CLASSIC_MIGRATION_DID: &[u8; 32] = b"system:polymath_classic_mig\0\0\0\0\0";
    /// Fiat Currency Reservation DID
    pub const FIAT_TICKERS_RESERVATION_DID: &[u8; 32] = b"system:fiat_tickers_reservation\0";
}

/// Priorities for the task that get scheduled.
pub mod queue_priority {
    use frame_support::traits::schedule::Priority;

    /// Queue priority for the settlement instruction execution.
    pub const SETTLEMENT_INSTRUCTION_EXECUTION_PRIORITY: Priority = 100;
    /// Queue priority for multisig action execution.
    pub const MULTISIG_PROPOSAL_EXECUTION_PRIORITY: Priority = 101;
}

/// Prefixes for scheduled actions
pub mod schedule_name_prefix {
    pub const SETTLEMENT_INSTRUCTION_EXECUTION: [u8; 27] = *b"SETTLEMENT_INSTRUCTION_EXEC";
    pub const MULTISIG_PROPOSAL_EXECUTION: [u8; 22] = *b"MULTISIG_PROPOSAL_EXEC";
    pub const PIP_EXECUTION: [u8; 8] = *b"PIP_EXEC";
    pub const PIP_EXPIRY: [u8; 10] = *b"PIP_EXPIRY";
}

// ERC1400 transfer status codes
pub const ERC1400_TRANSFER_FAILURE: u8 = 0x50;
pub const ERC1400_TRANSFER_SUCCESS: u8 = 0x51;
pub const ERC1400_INSUFFICIENT_BALANCE: u8 = 0x52;
pub const ERC1400_INSUFFICIENT_ALLOWANCE: u8 = 0x53;
pub const ERC1400_TRANSFERS_HALTED: u8 = 0x54;
pub const ERC1400_FUNDS_LOCKED: u8 = 0x55;
pub const ERC1400_INVALID_SENDER: u8 = 0x56;
pub const ERC1400_INVALID_RECEIVER: u8 = 0x57;
pub const ERC1400_INVALID_OPERATOR: u8 = 0x58;

// Application-specific status codes
pub const INVALID_SENDER_DID: u8 = 0xa0;
pub const INVALID_RECEIVER_DID: u8 = 0xa1;
pub const COMPLIANCE_MANAGER_FAILURE: u8 = 0xa2;
pub const SMART_EXTENSION_FAILURE: u8 = 0xa3;
pub const INVALID_GRANULARITY: u8 = 0xa4;
pub const APP_TX_VOLUME_LIMIT_REACHED: u8 = 0xa5;
pub const APP_BLOCKED_TX: u8 = 0xa6;
pub const APP_FUNDS_LOCKED: u8 = 0xa7;
pub const APP_FUNDS_LIMIT_REACHED: u8 = 0xa8;
pub const PORTFOLIO_FAILURE: u8 = 0xa9;
pub const CUSTODIAN_ERROR: u8 = 0xaa;
pub const SCOPE_CLAIM_MISSING: u8 = 0xab;
pub const TRANSFER_MANAGER_FAILURE: u8 = 0xac;

// PIP pallet constants.
pub const PIP_MAX_REPORTING_SIZE: usize = 1024;

/// Module ids, used for deriving sovereign account IDs for modules.
pub const TREASURY_MODULE_ID: ModuleId = ModuleId(*b"pm/trsry");
pub const BRR_MODULE_ID: ModuleId = ModuleId(*b"pm/blrwr");
pub const GC_MODULE_ID: ModuleId = ModuleId(*b"pm/govcm");
pub const CDD_MODULE_ID: ModuleId = ModuleId(*b"pm/cusdd");
pub const SETTLEMENT_MODULE_ID: ModuleId = ModuleId(*b"pm/setmn");
pub const CLASSIC_MIGRATION_MODULE_ID: ModuleId = ModuleId(*b"pm/ehmig");
pub const FIAT_TICKERS_RESERVATION_MODULE_ID: ModuleId = ModuleId(*b"pm/ftres");
