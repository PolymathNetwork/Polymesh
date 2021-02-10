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

use crate::migrate::{Empty, Migrate};

use codec::{Decode, Encode};
use polymesh_primitives_derive::VecU8StrongTyped;
use sp_std::{
    convert::{TryFrom, TryInto},
    ops::Deref,
    vec::Vec,
};

/// Previous version of `DocumentHash`.
/// It is only used during the migration.
#[derive(
    Decode, Encode, Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, VecU8StrongTyped,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct DocumentHashOld(pub Vec<u8>);

impl Migrate for DocumentHashOld {
    type Into = DocumentHash;
    type Context = Empty;

    fn migrate(self, _: Self::Context) -> Option<Self::Into> {
        DocumentHash::try_from(self.0.as_slice())
            .ok()
            .or_else(|| Some(DocumentHash::None))
    }
}

/// A wrapper for a document hash.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Encode, Decode)]
pub enum DocumentHash {
    /// No hash
    None,
    /// 512 bits output: Blake2b, SHA-512, SHA-3, Whirlpool
    H512([u8; 64]),
    /// 384 bits output: SHA-384, SHA-3
    H384([u8; 48]),
    /// 320 bits output: RIPEMD-320
    H320([u8; 40]),
    /// 256 bits output: Blake2s, HAVAL, PANAMA, RIPEMD-256, SHA-256, SHA-3
    H256([u8; 32]),
    /// 224 bits output: SHA-224, SHA-3, HAVAL,
    H224([u8; 28]),
    /// 192 bits output: HAVAL, Tiger-192
    H192([u8; 24]),
    /// 160 bits output: HAVAL, RIPEMD-160, SHA-0, SHA-1, Tiger-160
    H160([u8; 20]),
    /// 128 bits output: HAVAL, MD2, MD4, MD5, RIPEMD-128, Tiger-128
    H128([u8; 16]),
}

impl DocumentHash {
    /// Len of the hash in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.as_ref().len()
    }
}
impl Default for DocumentHash {
    fn default() -> Self {
        DocumentHash::None
    }
}

macro_rules! array_to_hash {
    ($array:ty, $hash:ident, $raw:expr) => {{
        <$array>::try_from($raw)
            .map(|array| Self::$hash(array))
            .map_err(|_| "unreachable")
    }};
}

impl TryFrom<&[u8]> for DocumentHash {
    type Error = &'static str;

    fn try_from(raw: &[u8]) -> Result<Self, Self::Error> {
        match raw.len() {
            64 => array_to_hash!([u8; 64], H512, raw),
            48 => array_to_hash!([u8; 48], H384, raw),
            40 => array_to_hash!([u8; 40], H320, raw),
            32 => array_to_hash!([u8; 32], H256, raw),
            28 => array_to_hash!([u8; 28], H224, raw),
            24 => array_to_hash!([u8; 24], H192, raw),
            20 => array_to_hash!([u8; 20], H160, raw),
            16 => array_to_hash!([u8; 16], H128, raw),
            0 => Ok(Self::None),
            _ => Err("Unsupported hash len"),
        }
    }
}

impl TryFrom<Vec<u8>> for DocumentHash {
    type Error = &'static str;

    #[inline]
    fn try_from(raw: Vec<u8>) -> Result<Self, Self::Error> {
        raw.as_slice().try_into()
    }
}

impl Deref for DocumentHash {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl AsRef<[u8]> for DocumentHash {
    fn as_ref(&self) -> &[u8] {
        match &self {
            Self::H512(raw) => &raw[..],
            Self::H384(raw) => &raw[..],
            Self::H320(raw) => &raw[..],
            Self::H256(raw) => &raw[..],
            Self::H224(raw) => &raw[..],
            Self::H192(raw) => &raw[..],
            Self::H160(raw) => &raw[..],
            Self::H128(raw) => &raw[..],
            Self::None => &[],
        }
    }
}

// Serde support
// ======================

#[cfg(feature = "std")]
use serde::{de::Error as SerdeError, Deserialize, Deserializer, Serialize, Serializer};

#[cfg(feature = "std")]
impl Serialize for DocumentHash {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.as_ref().serialize(serializer)
    }
}

#[cfg(feature = "std")]
impl<'de> Deserialize<'de> for DocumentHash {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = <Vec<u8>>::deserialize(deserializer)?;
        raw.try_into().map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn make_hashes() -> Vec<DocumentHash> {
        vec![
            DocumentHash::H512([1u8; 64]),
            DocumentHash::H384([2u8; 48]),
            DocumentHash::H320([3u8; 40]),
            DocumentHash::H256([4u8; 32]),
            DocumentHash::H224([5u8; 28]),
            DocumentHash::H192([6u8; 24]),
            DocumentHash::H160([7u8; 20]),
            DocumentHash::H128([8u8; 16]),
            DocumentHash::None,
        ]
    }

    #[test]
    fn build_hash() {
        let hashes = make_hashes();

        // Verify `try_from` a slice
        for h in hashes.into_iter() {
            let raw = h.as_ref();
            let new_h = DocumentHash::try_from(raw);
            assert_eq!(Ok(h), new_h);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn serde_tests() -> Result<(), serde_json::Error> {
        let hashes = make_hashes();

        for h in hashes.into_iter() {
            let encoded = serde_json::to_string(&h)?;
            let raw_encoded = serde_json::to_string(h.as_ref())?;
            assert_eq!(encoded, raw_encoded);

            let decoded: DocumentHash = serde_json::from_str(&encoded)?;
            assert_eq!(h, decoded);
        }

        Ok(())
    }
}
