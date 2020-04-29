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
    pub const GOVERNANCE_COMMITTEE_ID: &[u8; 32] = b"system:governance_committee\0\0\0\0\0";
    /// CDD Providers DID. It is used in systematic CDD claim for CDD Providers.
    pub const CDD_PROVIDERS_ID: &[u8; 32] = b"system:customer_due_diligence\0\0\0";
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
pub const APP_NOT_AFFECTED: u8 = 0xa0;
pub const APP_SUCCESS: u8 = 0xa1;
pub const APP_MAX_HOLDERS_REACHED: u8 = 0xa2;
pub const APP_MANUAL_APPROVAL_EXPIRED: u8 = 0xa3;
pub const APP_FUNDS_LIMIT_REACHED: u8 = 0xa4;
pub const APP_TX_VOLUME_LIMIT_REACHED: u8 = 0xa5;
pub const APP_BLACKLISTED_TX: u8 = 0xa6;
pub const APP_FUNDS_LOCKED: u8 = 0xa7;
pub const APP_INVALID_GRANULARITY: u8 = 0xa8;

// PIP pallet constants.
pub const PIP_MAX_REPORTING_SIZE: usize = 1024;
