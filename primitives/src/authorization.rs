// This file is part of the Polymesh distribution (https://github.com/PolymathNetwork/Polymesh).
// Copyright (c) 2020 Polymath

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

use crate::{
    agent::AgentGroup, identity_id::IdentityId, secondary_key::Permissions, Balance, PortfolioId,
    Ticker,
};
use codec::{Decode, Encode};
use frame_support::dispatch::DispatchError;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::prelude::*;

/// Authorization data for two step processes.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AuthorizationData<AccountId> {
    // TODO: Remove Custom type and move NoData at the start
    /// CDD provider's attestation to change primary key
    AttestPrimaryKeyRotation(IdentityId),
    /// Authorization to change primary key
    RotatePrimaryKey(IdentityId),
    /// Authorization to transfer a ticker
    /// Must be issued by the current owner of the ticker
    TransferTicker(Ticker),
    /// Not in use anymore.
    TransferPrimaryIssuanceAgent(Ticker),
    /// Add a signer to multisig
    /// Must be issued to the identity that created the ms (and therefore owns it permanently)
    AddMultiSigSigner(AccountId),
    /// Authorization to transfer a token's ownership
    /// Must be issued by the current owner of the asset
    TransferAssetOwnership(Ticker),
    /// Authorization to join an Identity
    /// Must be issued by the identity which is being joined
    JoinIdentity(Permissions),
    /// Authorization to take custody of a portfolio
    PortfolioCustody(PortfolioId),
    /// Any other authorization
    Custom(Ticker),
    /// No authorization data
    NoData,
    /// Not in use anymore.
    TransferCorporateActionAgent(Ticker),
    /// Authorization to become an agent of the `Ticker` with the `AgentGroup`.
    BecomeAgent(Ticker, AgentGroup),
    /// Add Relayer paying key to user key
    /// Must be issued by the paying key.
    /// `AddRelayerPayingKey(user_key, paying_key, polyx_limit)`
    AddRelayerPayingKey(AccountId, AccountId, Balance),
}

impl<AccountId> AuthorizationData<AccountId> {
    /// Returns the `AuthorizationType` of this auth data.
    pub fn auth_type(&self) -> AuthorizationType {
        match self {
            Self::AttestPrimaryKeyRotation(..) => AuthorizationType::AttestPrimaryKeyRotation,
            Self::RotatePrimaryKey(..) => AuthorizationType::RotatePrimaryKey,
            Self::TransferTicker(..) => AuthorizationType::TransferTicker,
            Self::TransferPrimaryIssuanceAgent(..) => AuthorizationType::NoData,
            Self::TransferCorporateActionAgent(..) => AuthorizationType::NoData,
            Self::BecomeAgent(..) => AuthorizationType::BecomeAgent,
            Self::AddMultiSigSigner(..) => AuthorizationType::AddMultiSigSigner,
            Self::TransferAssetOwnership(..) => AuthorizationType::TransferAssetOwnership,
            Self::JoinIdentity(..) => AuthorizationType::JoinIdentity,
            Self::PortfolioCustody(..) => AuthorizationType::PortfolioCustody,
            Self::AddRelayerPayingKey(..) => AuthorizationType::AddRelayerPayingKey,
            Self::Custom(..) => AuthorizationType::Custom,
            Self::NoData => AuthorizationType::NoData,
        }
    }
}

/// Type of authorization.
#[derive(Eq, PartialEq, Encode, Decode, Clone)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub enum AuthorizationType {
    // TODO: Remove Custom type and move NoData at the start
    /// CDD Authorization to rotate primary key.
    AttestPrimaryKeyRotation,
    /// Authorization to rotate primary key.
    RotatePrimaryKey,
    /// Authorization to transfer a ticker.
    TransferTicker,
    /// Authorization to add some key int a multi signer.
    AddMultiSigSigner,
    /// Authorization to transfer the asset ownership to other identity.
    TransferAssetOwnership,
    /// Join Identity authorization.
    JoinIdentity,
    /// Accept custody of a portfolio
    PortfolioCustody,
    /// Customized authorization.
    Custom,
    /// Undefined authorization.
    NoData,
    /// Authorization to become an agent of a ticker.
    BecomeAgent,
    /// Authorization to add a Relayer paying key.
    AddRelayerPayingKey,
}

impl<AccountId> Default for AuthorizationData<AccountId> {
    fn default() -> Self {
        AuthorizationData::NoData
    }
}

/// Status of an Authorization after consume is called on it.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum AuthorizationError {
    /// Auth identified by an `auth_id` for a given `target` does not exist.
    /// The `target` might be wrong or the `auth_id` was never created at all.
    Invalid,
    /// Caller not authorized or the identity who created
    /// this authorization is not authorized to create this authorization.
    Unauthorized,
    /// Auth expired already.
    Expired,
    /// The extrinsic expected a different `AuthorizationType`
    /// than what the `data.auth_type()` is.
    BadType,
}

impl From<AuthorizationError> for DispatchError {
    fn from(error: AuthorizationError) -> DispatchError {
        match error {
            AuthorizationError::Invalid => DispatchError::Other("Authorization does not exist"),
            AuthorizationError::Unauthorized => {
                DispatchError::Other("Illegal use of Authorization")
            }
            AuthorizationError::Expired => DispatchError::Other("Authorization expired"),
            AuthorizationError::BadType => DispatchError::Other("Authorization type is wrong"),
        }
    }
}

/// Authorization struct
#[derive(Encode, Decode, Default, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Authorization<AccountId, Moment> {
    /// Enum that contains authorization type and data
    pub authorization_data: AuthorizationData<AccountId>,

    /// Identity of the organization/individual that added this authorization
    pub authorized_by: IdentityId,

    /// time when this authorization expires. optional.
    pub expiry: Option<Moment>,

    /// Authorization id of this authorization
    pub auth_id: u64,
}

/// Extract the authoriation variant's data, or bail.
#[macro_export]
macro_rules! extract_auth {
    ($data:expr, $variant:ident ( $($f:ident),*) ) => {
        match $data {
            $crate::authorization::AuthorizationData::$variant($($f),*) => ($($f),*),
            _ => frame_support::fail!($crate::authorization::AuthorizationError::BadType),
        }
    }
}
