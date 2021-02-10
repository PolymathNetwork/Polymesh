//! Runtime API definition for Identity module.

use codec::{Decode, Encode};
use polymesh_primitives::{ClaimType, IdentityId, Permissions, Scope, SecondaryKey};
use sp_std::{prelude::*, vec::Vec};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub type Error = Vec<u8>;
pub type CddStatus = Result<IdentityId, Error>;
pub type AssetDidResult = Result<IdentityId, Error>;

/// A result of execution of get_votes.
#[derive(Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum DidRecords<AccountId, SecondaryKey> {
    /// Id was found and has the following primary key and secondary keys.
    Success {
        primary_key: AccountId,
        secondary_keys: Vec<SecondaryKey>,
    },
    /// Error.
    IdNotFound,
}

#[derive(Encode, Decode, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub enum DidStatus {
    Unknown,
    Exists,
    CddVerified,
}

/// Aggregate information about an `AccountId` in relation to an `IdentityId`.
#[derive(Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct KeyIdentityData<IdentityId> {
    /// The identity of the provided `AccountId`.
    pub identity: IdentityId,
    /// What permissions does the `AccountId` have within the `identity`?
    /// If `None`, then this is a primary key.
    pub permissions: Option<Permissions>,
}

/// Result of a successful call permission check.
#[derive(Clone, Eq, PartialEq)]
pub struct PermissionedCallOriginData<AccountId: Encode + Decode> {
    /// The origin account.
    pub sender: AccountId,
    /// The primary identity associated with the call.
    pub primary_did: IdentityId,
    /// The secondary account or identity associated with the call, if the caller is a secondary identity
    /// of `primary_did`. None if the caller is the primary key of `primary_did`. This field can be used when
    /// checking asset and portfolio permissions. It is`Some(did)` if the current identity (the identity that
    /// the call is made from) is a secondary identity `did` of `primary_did`. Otherwise it will be `Some(key)`.
    pub secondary_key: Option<SecondaryKey<AccountId>>,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Claim1stKey {
    pub target: IdentityId,
    pub claim_type: ClaimType,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Claim2ndKey {
    pub issuer: IdentityId,
    pub scope: Option<Scope>,
}
