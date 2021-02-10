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

//! Shareable types.

#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]
#![feature(bool_to_option)]

use blake2::{Blake2b, Digest};
use confidential_identity::Scalar;
use polymesh_primitives_derive::VecU8StrongTyped;
#[cfg(feature = "std")]
use sp_runtime::{Deserialize, Serialize};
use sp_std::prelude::Vec;

use codec::{Decode, Encode};
use sp_runtime::{
    generic,
    traits::{BlakeTwo256, IdentifyAccount, Verify},
    MultiSignature,
};

/// An index to a block.
/// 32-bits will allow for 136 years of blocks assuming 1 block per second.
pub type BlockNumber = u32;

/// An instant or duration in time.
pub type Moment = u64;

/// Alias to 512-bit hash when used in the context of a signature on the relay chain.
/// Equipped with logic for possibly "unsigned" messages.
pub type Signature = MultiSignature;

/// Alias to an sr25519 or ed25519 key.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The type for looking up accounts. We don't expect more than 4 billion of them.
pub type AccountIndex = u32;

/// Identifier for a chain. 32-bit should be plenty.
pub type ChainId = u32;

/// A hash of some data used by the relay chain.
pub type Hash = sp_core::H256;

/// Index of a transaction in the relay chain. 32-bit should be plenty.
pub type Index = u32;

/// App-specific crypto used for reporting equivocation/misbehavior in BABE and
/// GRANDPA. Any rewards for misbehavior reporting will be paid out to this
/// account.
#[cfg(feature = "std")]
pub mod report {
    use super::{Signature, Verify};
    use frame_system::offchain::AppCrypto;
    use sp_core::crypto::{key_types, KeyTypeId};

    /// Key type for the reporting module. Used for reporting BABE and GRANDPA
    /// equivocations.
    pub const KEY_TYPE: KeyTypeId = key_types::REPORTING;

    mod app {
        use sp_application_crypto::{app_crypto, sr25519};
        app_crypto!(sr25519, super::KEY_TYPE);
    }

    /// Identity of the equivocation/misbehavior reporter.
    pub type ReporterId = app::Public;

    /// An `AppCrypto` type to allow submitting signed transactions using the reporting
    /// application key as signer.
    pub struct ReporterAppCrypto;

    impl AppCrypto<<Signature as Verify>::Signer, Signature> for ReporterAppCrypto {
        type RuntimeAppPublic = ReporterId;
        type GenericSignature = sp_core::sr25519::Signature;
        type GenericPublic = sp_core::sr25519::Public;
    }
}

/// A positive coefficient: a pair of a numerator and a denominator. Defaults to `(1, 1)`.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Decode, Encode, Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PosRatio(pub u32, pub u32);

impl Default for PosRatio {
    fn default() -> Self {
        PosRatio(1, 1)
    }
}

impl From<(u32, u32)> for PosRatio {
    fn from((n, d): (u32, u32)) -> Self {
        PosRatio(n, d)
    }
}

/// It creates a scalar from the blake2_512 hash of `data` parameter.
pub fn scalar_blake2_from_bytes(data: impl AsRef<[u8]>) -> Scalar {
    let mut hash = [0u8; 64];
    hash.copy_from_slice(
        Blake2b::default()
            .chain(data.as_ref())
            .finalize()
            .as_slice(),
    );
    Scalar::from_bytes_mod_order_wide(&hash)
}

/// The balance of an account.
/// 128-bits (or 38 significant decimal figures) will allow for 10m currency (10^7) at a resolution
/// to all for one second's worth of an annualised 50% reward be paid to a unit holder (10^11 unit
/// denomination), or 10^18 total atomic units, to grow at 50%/year for 51 years (10^9 multiplier)
/// for an eventual total of 10^27 units (27 significant decimal figures).
/// We round denomination to 10^12 (12 sdf), and leave the other redundancy at the upper end so
/// that 32 bits may be multiplied with a balance in 128 bits without worrying about overflow.
pub type Balance = u128;

/// Header type.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// Block ID.
pub type BlockId = generic::BlockId<Block>;

/// Opaque, encoded, unchecked extrinsic.
pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

/// Utility byte container where equality comparison are ignored case.
pub mod ignored_case_string;
pub use ignored_case_string::IgnoredCaseString;

/// Asset identifiers.
pub mod asset_identifier;
pub use asset_identifier::AssetIdentifier;

pub mod event_only;
pub use event_only::EventOnly;

/// Polymesh Distributed Identity.
pub mod identity_id;
pub use identity_id::{
    EventDid, IdentityId, PortfolioId, PortfolioKind, PortfolioName, PortfolioNumber,
};

/// Identity information.
/// Each DID is associated with this kind of record.
pub mod identity;
pub use identity::Identity;

/// CDD Identity is an ID to link the encrypted investor UID with one Identity ID.
/// That keeps the privacy of a real investor and its global portfolio split in several Polymesh
/// Identities.
pub mod cdd_id;
pub use cdd_id::{CddId, InvestorUid};

/// Investor Zero Knowledge Proof data
pub mod investor_zkproof_data;
pub use investor_zkproof_data::InvestorZKProofData;

/// Claim information.
/// Each claim is associated with this kind of record.
pub mod identity_claim;
pub use identity_claim::{Claim, ClaimType, IdentityClaim, Scope, ScopeId};

// Defining and enumerating jurisdictions.
pub mod jurisdiction;
pub use jurisdiction::CountryCode;

/// Utilities for storage migration.
pub mod migrate;

/// This module contains entities related with secondary keys.
pub mod secondary_key;
pub use secondary_key::{
    AssetPermissions, ExtrinsicPermissions, PalletPermissions, Permissions, PortfolioPermissions,
    SecondaryKey, Signatory,
};

/// Subset type.
pub mod subset;
pub use subset::{LatticeOrd, LatticeOrdering, SubsetRestriction};

/// Generic authorization data types for all two step processes
pub mod authorization;
/// Pub Traits
pub mod traits;
pub use authorization::AuthIdentifier;
pub use authorization::Authorization;
pub use authorization::AuthorizationData;
pub use authorization::AuthorizationError;
pub use authorization::AuthorizationType;

pub mod ticker;
pub use ticker::Ticker;

/// This module defines types used by smart extensions
pub mod smart_extension;
pub use smart_extension::{
    ExtensionAttributes, MetaDescription, MetaUrl, MetaVersion, SmartExtension, SmartExtensionName,
    SmartExtensionType, TemplateDetails, TemplateMetadata,
};

/// Document hash
pub mod document_hash;
pub use document_hash::{DocumentHash, DocumentHashOld};

/// Document types
pub mod document;
pub use document::{Document, DocumentId, DocumentName, DocumentUri};

/// Rules for claims.
pub mod condition;
pub use condition::{Condition, ConditionType, TargetIdentity, TrustedFor, TrustedIssuer};

/// Predicate calculation for Claims.
pub mod proposition;
pub use proposition::{AndProposition, Context, NotProposition, OrProposition, Proposition};

/// For confidential stuff.
pub mod valid_proof_of_investor;
pub use valid_proof_of_investor::ValidProofOfInvestor;

/// Timekeeping and checkpoints.
pub mod calendar;

/// Runtime crypto tools.
pub mod crypto;

/// Asset type definitions.
pub mod asset;

/// Represents custom transaction errors.
#[repr(u8)]
pub enum TransactionError {
    /// 0-6 are used by substrate. Skipping them to avoid confusion
    ZeroTip = 0,
    /// Transaction needs an Identity associated to an account.
    MissingIdentity = 1,
    /// CDD is required
    CddRequired = 2,
    /// Invalid auth id
    InvalidAuthorization = 3,
}

/// Represents the target identity and the amount requested by a beneficiary.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug)]
pub struct Beneficiary<Balance> {
    /// Beneficiary identity.
    pub id: IdentityId,
    /// Amount requested to this beneficiary.
    pub amount: Balance,
}

/// The name of a pallet.
#[derive(
    Decode, Encode, Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, VecU8StrongTyped,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct PalletName(pub Vec<u8>);

/// The name of a function within a pallet.
#[derive(
    Decode, Encode, Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, VecU8StrongTyped,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct DispatchableName(pub Vec<u8>);

/// Create a `Version` struct with an upper limit.
#[macro_export]
macro_rules! storage_migration_ver {
    ($ver:literal) => {
        #[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub struct Version(u8);

        impl Version {
            const MAX: u8 = $ver;

            /// Constructor as `const function` which is interpreted by the compiler at
            /// compile-time.
            const fn new(ver: u8) -> Option<Self> {
                if ver <= Self::MAX {
                    Some(Self(ver))
                } else {
                    None
                }
            }
        }

        impl Default for Version {
            fn default() -> Self {
                Version(0)
            }
        }

        impl sp_std::convert::TryFrom<u8> for Version {
            type Error = &'static str;

            fn try_from(ver: u8) -> Result<Self, Self::Error> {
                Self::new(ver).ok_or("Unsupported version")
            }
        }
    };
}

/// Helper macro which execute the `$body` if `$curr` is less than version `$ver`.
/// It also updates `StorageVersion` in the current pallet to `$ver`.
#[macro_export]
macro_rules! storage_migrate_on {
    ($curr: expr, $ver:literal, $([$($targ:ty),*])? $body: block) => {{
        const TARGET_VERSION: Version = Version::new($ver).unwrap();
        if $curr < TARGET_VERSION {
            $body;
            StorageVersion::< $($($targ),*)? >::put(TARGET_VERSION);
        }
    }};
}

#[cfg(test)]
mod tests {
    use polymesh_primitives_derive::{SliceU8StrongTyped, VecU8StrongTyped};

    use codec::{Decode, Encode};
    use sp_std::convert::TryInto;

    #[derive(VecU8StrongTyped)]
    struct A(Vec<u8>);
    #[derive(VecU8StrongTyped)]
    struct B(Vec<u8>);

    #[derive(Default, SliceU8StrongTyped)]
    struct C([u8; 16]);
    #[derive(Default, SliceU8StrongTyped)]
    struct D([u8; 16]);

    #[test]
    fn vec_strong_typed() {
        let text1 = &b"Lorem Ipsum ";
        let text2 = &b"dolor sit amet";
        // let text3 = &b"Lorem Ipsum dolor sit amet";

        // From traits.
        let mut a1 = A::from(text1);
        let a2: A = text1.into();
        let _b1 = B::from(text2.to_vec());
        let _b2: B = text2.to_vec().into();

        // Deref & DerefMut
        assert_eq!(*a1, text1[..]);
        a1[6] = b'i';
        assert_eq!(a1.as_slice(), &b"Lorem ipsum "[..]);

        // Other methods.
        assert_eq!(a2.len(), text1.len());
        assert_eq!(a2.as_slice(), &text1[..]);
        assert_eq!(a2.as_vec().clone(), text1.to_vec());

        // Strong types are not equal.
        // The below line does NOT compile.
        // let a3 :A = _b1;
    }

    #[test]
    fn slice_strong_typed() {
        let text1 = &b"lorem Ipsum ";
        let text2 = &b"dolor sit amet";
        // let text3 = &b"Lorem Ipsum dolor sit amet";

        // From traits.
        let mut c1 = C::from(text1.as_ref());
        let c2: C = C::from(text1.as_ref());
        let _d1 = D::from(text2.as_ref());

        // Deref & DerefMut
        let text1_with_zeros = &b"lorem Ipsum \0\0\0\0";
        assert_eq!(*c1, text1_with_zeros[..]);
        c1[6] = b'i';
        assert_eq!(c1.as_slice(), &b"lorem ipsum \0\0\0\0"[..]);

        // Other methods.
        assert_eq!(c2.len(), text1_with_zeros.len());
        assert_eq!(c2.as_slice(), &text1_with_zeros[..]);

        // Strong types are not equal.
        // The below line does NOT compile.
        // let c3 :C = _d1;
    }

    #[test]
    fn storage_migration_ver_test_1() {
        storage_migration_ver!(3);

        assert!(Version::new(2).is_some());
        assert!(Version::new(4).is_none());

        let v: Result<Version, _> = 3u8.try_into();
        assert!(v.is_ok());

        let v: Result<Version, _> = 5u8.try_into();
        assert!(v.is_err());
    }
}
