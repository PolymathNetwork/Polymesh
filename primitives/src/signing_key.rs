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

use crate::{IdentityId, PortfolioNumber, Subset, Ticker};
use codec::{Decode, Encode};
#[cfg(feature = "std")]
use sp_runtime::{Deserialize, Serialize};
use sp_std::{
    cmp::{Ord, Ordering, PartialOrd},
    prelude::Vec,
};

/// Asset permissions.
pub type AssetPermissions = Subset<Ticker>;

/// Extrinsic permissions.
///
/// FIXME: correct the type of extrinsic.
pub type ExtrinsicPermissions = Subset<Vec<u8>>;

/// Portfolio permissions.
pub type PortfolioPermissions = Subset<PortfolioNumber>;

/// Signing key permissions.
#[derive(Encode, Decode, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Permissions {
    /// The subset of assets under management.
    pub asset: AssetPermissions,
    /// The subset of callable extrinsics.
    pub extrinsic: ExtrinsicPermissions,
    /// The subset of portfolios management.
    pub portfolio: PortfolioPermissions,
}

impl Permissions {
    /// The empty permissions.
    pub fn empty() -> Self {
        Self {
            asset: Subset::empty(),
            extrinsic: Subset::empty(),
            portfolio: Subset::empty(),
        }
    }
}

/// It supports different elements as a signer.
#[allow(missing_docs)]
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Signatory<AccountId> {
    Identity(IdentityId),
    Account(AccountId),
}

impl<AccountId> Default for Signatory<AccountId> {
    fn default() -> Self {
        Signatory::Identity(IdentityId::default())
    }
}

impl<AccountId> From<IdentityId> for Signatory<AccountId> {
    fn from(v: IdentityId) -> Self {
        Signatory::Identity(v)
    }
}

impl<AccountId> PartialEq<IdentityId> for Signatory<AccountId> {
    fn eq(&self, other: &IdentityId) -> bool {
        match self {
            Signatory::Identity(ref id) => id == other,
            _ => false,
        }
    }
}

impl<AccountId> Signatory<AccountId>
where
    AccountId: PartialEq,
{
    /// Checks if Signatory is either a particular Identity or a particular key
    pub fn eq_either(&self, other_identity: &IdentityId, other_key: &AccountId) -> bool {
        match self {
            Signatory::Account(ref key) => key == other_key,
            Signatory::Identity(ref id) => id == other_identity,
        }
    }

    /// This signatory as `IdentityId` or None.
    pub fn as_identity(&self) -> Option<&IdentityId> {
        match self {
            Signatory::Identity(id) => Some(id),
            _ => None,
        }
    }

    /// This signatory as `AccountId` or None.
    pub fn as_account(&self) -> Option<&AccountId> {
        match self {
            Signatory::Account(key) => Some(key),
            _ => None,
        }
    }
}

impl<AccountId> PartialOrd for Signatory<AccountId>
where
    AccountId: Ord,
{
    /// Any key is less than any Identity.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<AccountId> Ord for Signatory<AccountId>
where
    AccountId: Ord,
{
    fn cmp(&self, other: &Self) -> Ordering {
        match self {
            Signatory::Identity(id) => match other {
                Signatory::Identity(other_id) => id.cmp(other_id),
                Signatory::Account(..) => Ordering::Greater,
            },
            Signatory::Account(key) => match other {
                Signatory::Account(other_key) => key.cmp(other_key),
                Signatory::Identity(..) => Ordering::Less,
            },
        }
    }
}

/// A signing key of an identity.
#[allow(missing_docs)]
#[derive(Encode, Decode, Default, Clone, Eq, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct SigningKey<AccountId> {
    /// The signatory of the signing key.
    pub signer: Signatory<AccountId>,
    /// The access permissions of the signing key.
    pub permissions: Permissions,
}

impl<AccountId> SigningKey<AccountId> {
    /// Creates a [`SigningKey`].
    pub fn new(signer: Signatory<AccountId>, permissions: Permissions) -> Self {
        Self {
            signer,
            permissions,
        }
    }

    /// Creates a [`SigningKey`] from an `AccountId` with empty permissions.
    pub fn from_account_id(s: AccountId) -> Self {
        Self {
            signer: Signatory::Account(s),
            permissions: Permissions::empty(),
        }
    }

    /// Checks if the given key has permission to access the given asset.
    pub fn has_asset_permission(&self, asset: Ticker) -> bool {
        self.permissions.asset.ge(&Subset::elem(asset))
    }

    /// Checks if the given key has permission to call the given extrinsic.
    ///
    /// FIXME: replace the placeholder type of extrinsic with a correct one.
    pub fn has_extrinsic_permission(&self, extrinsic: Vec<u8>) -> bool {
        self.permissions.extrinsic.ge(&Subset::elem(extrinsic))
    }

    /// Checks if the given key has permission to access the given portfolio.
    pub fn has_portfolio_permission(&self, portfolio: PortfolioNumber) -> bool {
        self.permissions.portfolio.ge(&Subset::elem(portfolio))
    }

    /// Checks if the signing key has the given permissions.
    pub fn has_permissions(&self, them: &Permissions) -> bool {
        let us = &self.permissions;
        us.asset.ge(&them.asset)
            && us.extrinsic.ge(&them.extrinsic)
            && us.portfolio.ge(&them.portfolio)
    }
}

impl<AccountId> From<IdentityId> for SigningKey<AccountId> {
    fn from(id: IdentityId) -> Self {
        Self {
            signer: Signatory::Identity(id),
            permissions: Permissions::empty(),
        }
    }
}

impl<AccountId> PartialEq for SigningKey<AccountId>
where
    AccountId: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.signer == other.signer && self.permissions == other.permissions
    }
}

impl<AccountId> PartialEq<IdentityId> for SigningKey<AccountId> {
    fn eq(&self, other: &IdentityId) -> bool {
        if let Signatory::Identity(id) = self.signer {
            id == *other
        } else {
            false
        }
    }
}

impl<AccountId> PartialOrd for SigningKey<AccountId>
where
    AccountId: Ord,
{
    /// Any key is less than any Identity.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.signer.partial_cmp(&other.signer)
    }
}

impl<AccountId> Ord for SigningKey<AccountId>
where
    AccountId: Ord,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.signer.cmp(&other.signer)
    }
}

#[cfg(test)]
mod tests {
    use super::{Permissions, Signatory, SigningKey, Subset};
    use crate::{IdentityId, Ticker};
    use sp_core::sr25519::Public;
    use std::convert::{From, TryFrom};

    #[test]
    fn build_test() {
        let key = Public::from_raw([b'A'; 32]);
        let rk1 = SigningKey::new(Signatory::Account(key.clone()), Permissions::empty());
        let rk2 = SigningKey::from_account_id(key.clone());
        assert_eq!(rk1, rk2);

        let rk3_permissions = Permissions {
            asset: Subset::Elems(vec![Ticker::try_from(&[1][..]).unwrap()]),
            extrinsic: Subset::All,
            portfolio: Subset::Elems(vec![1]),
        };
        let rk3 = SigningKey::new(Signatory::Account(key.clone()), rk3_permissions.clone());
        assert_ne!(rk1, rk3);

        let mut rk4 = SigningKey::from_account_id(key);
        rk4.permissions = rk3_permissions;
        assert_eq!(rk3, rk4);

        let si1 = SigningKey::from(IdentityId::from(1u128));
        let si2 = SigningKey::from(IdentityId::from(1u128));
        assert_eq!(si1, si2);

        let si3 = SigningKey::from(IdentityId::from(2u128));
        assert_ne!(si1, si3);

        assert_ne!(si1, rk1);
    }

    #[test]
    fn has_permission_test() {
        let key = Public::from_raw([b'A'; 32]);
        let ticker1 = Ticker::try_from(&[1][..]).unwrap();
        let ticker2 = Ticker::try_from(&[2][..]).unwrap();
        let permissions = Permissions {
            asset: Subset::Elems(vec![ticker1]),
            extrinsic: Subset::All,
            portfolio: Subset::Elems(vec![1]),
        };
        let free_key = SigningKey::new(Signatory::Account(key.clone()), Permissions::default());
        let restricted_key = SigningKey::new(Signatory::Account(key), permissions.clone());
        assert!(free_key.has_asset_permission(ticker2));
        assert!(free_key.has_extrinsic_permission(vec![2]));
        assert!(free_key.has_portfolio_permission(2));
        assert!(!restricted_key.has_asset_permission(ticker2));
        assert!(restricted_key.has_extrinsic_permission(vec![2]));
        assert!(!restricted_key.has_portfolio_permission(2));
        assert!(free_key.has_permissions(&permissions));
        assert!(restricted_key.has_permissions(&permissions));
        let extended_permissions = Permissions {
            asset: Subset::Elems(vec![ticker1]),
            extrinsic: Subset::All,
            portfolio: Subset::Elems(vec![1, 2]),
        };
        assert!(free_key.has_permissions(&extended_permissions));
        assert!(!restricted_key.has_permissions(&extended_permissions));
    }

    #[test]
    fn signer_build_and_eq_tests() {
        let key = Public::from_raw([b'A'; 32]);
        let iden = IdentityId::try_from(
            "did:poly:f1d273950ddaf693db228084d63ef18282e00f91997ae9df4f173f09e86d0976",
        )
        .unwrap();
        let iden_sig: Signatory<sp_core::sr25519::Public> = Signatory::from(iden);
        assert_ne!(Signatory::Account(key), iden_sig);
        assert_eq!(iden_sig, iden);
        assert_ne!(iden_sig, Signatory::Account(key));
    }
}
