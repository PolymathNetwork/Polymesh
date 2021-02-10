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

use crate::EventOnly;
use codec::{Decode, Encode};
use core::fmt::{Display, Formatter};
use core::str;
use polymesh_primitives_derive::VecU8StrongTyped;
#[cfg(feature = "std")]
use polymesh_primitives_derive::{DeserializeU8StrongTyped, SerializeU8StrongTyped};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::Printable;
use sp_std::prelude::Vec;

const _POLY_DID_PREFIX: &str = "did:poly:";
const POLY_DID_PREFIX_LEN: usize = 9; // _POLY_DID_PREFIX.len(); // CI does not support: #![feature(const_str_len)]
const POLY_DID_LEN: usize = POLY_DID_PREFIX_LEN + UUID_LEN * 2;
const UUID_LEN: usize = 32usize;

/// Polymesh Identifier ID.
/// It is stored internally as an `u128` but it can be load from string with the following format:
/// "did:poly:<32 Hex characters>".
///
/// # From str
/// The current implementation of `TryFrom<&str>` requires exactly 32 hexadecimal characters for
/// code part of DID.
/// Valid examples are the following:
///  - "did:poly:ab01cd12ef34ab01cd12ef34ab01cd12"
/// Invalid examples:
///  - "did:poly:ab01"
///  - "did:poly:1"
///  - "DID:poly:..."
#[derive(Encode, Decode, Default, PartialOrd, Ord, PartialEq, Eq, Clone, Copy, Hash)]
#[cfg_attr(
    feature = "std",
    derive(SerializeU8StrongTyped, DeserializeU8StrongTyped)
)]
pub struct IdentityId(pub [u8; UUID_LEN]);

/// Alias for `EventOnly<IdentityId>`.
// Exists because schema checks don't know how to handle `EventOnly`.
pub type EventDid = EventOnly<IdentityId>;

impl IdentityId {
    /// Protect the DID as only for use in events.
    #[inline]
    pub fn for_event(self) -> EventDid {
        EventDid::new(self)
    }

    /// Returns a byte slice of this IdentityId's contents
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0[..]
    }

    /// Extracts a reference to the byte array containing the entire fixed id.
    #[inline]
    pub fn as_fixed_bytes(&self) -> &[u8; UUID_LEN] {
        &self.0
    }

    /// Transform this IdentityId into raw bytes.
    #[inline]
    pub fn to_bytes(self) -> [u8; UUID_LEN] {
        self.0
    }

    /// Returns an iterator over the slice.
    #[inline]
    pub fn iter(&self) -> sp_std::slice::Iter<'_, u8> {
        self.0.iter()
    }

    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "did:poly:")?;
        for byte in &self.0 {
            f.write_fmt(format_args!("{:02x}", byte))?;
        }
        Ok(())
    }
}

impl Display for IdentityId {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        self.fmt(f)
    }
}

impl sp_std::fmt::Debug for IdentityId {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        self.fmt(f)
    }
}

impl From<u128> for IdentityId {
    fn from(id: u128) -> Self {
        let encoded_id = id.encode();
        let mut did = [0; 32];
        for (i, n) in encoded_id.into_iter().enumerate() {
            did[i] = n;
        }

        Self(did)
    }
}

use frame_support::ensure;
use sp_std::convert::TryFrom;

impl TryFrom<&str> for IdentityId {
    type Error = &'static str;

    fn try_from(did: &str) -> Result<Self, Self::Error> {
        ensure!(did.len() == POLY_DID_LEN, "Invalid length of IdentityId");

        // Check prefix
        let prefix = &did[..POLY_DID_PREFIX_LEN];
        ensure!(prefix == _POLY_DID_PREFIX, "Missing 'did:poly:' prefix");

        // Check hex code
        let did_code = (POLY_DID_PREFIX_LEN..POLY_DID_LEN)
            .step_by(2)
            .map(|idx| u8::from_str_radix(&did[idx..idx + 2], 16))
            .collect::<Result<Vec<u8>, _>>()
            .map_err(|_| "DID code is not a valid hex")?;

        IdentityId::try_from(did_code.as_slice())
    }
}

impl TryFrom<&[u8]> for IdentityId {
    type Error = &'static str;

    fn try_from(did: &[u8]) -> Result<Self, Self::Error> {
        if did.len() <= UUID_LEN {
            // case where a 256 bit hash is being converted
            let mut fixed = [0; 32];
            fixed[(UUID_LEN - did.len())..].copy_from_slice(&did);
            Ok(Self(fixed))
        } else {
            // case where a string represented as u8 is being converted
            let did_str = str::from_utf8(did).map_err(|_| "DID is not valid UTF-8")?;
            IdentityId::try_from(did_str)
        }
    }
}

impl From<[u8; UUID_LEN]> for IdentityId {
    fn from(s: [u8; UUID_LEN]) -> Self {
        Self(s)
    }
}

impl AsRef<[u8]> for IdentityId {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Printable for IdentityId {
    fn print(&self) {
        sp_io::misc::print_utf8(b"did:poly:");
        sp_io::misc::print_hex(&self.0);
    }
}

/// A wrapper for a portfolio name. It is used for non-default (aka "user") portfolios only since
/// default ones are nameless.
#[derive(
    Decode, Encode, Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, VecU8StrongTyped,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct PortfolioName(pub Vec<u8>);

/// The unique ID of a non-default portfolio.
#[derive(Decode, Encode, Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct PortfolioNumber(pub u64);

impl Default for PortfolioNumber {
    fn default() -> Self {
        Self(1)
    }
}

impl From<u64> for PortfolioNumber {
    fn from(num: u64) -> Self {
        Self(num)
    }
}

/// TBD
#[derive(Decode, Encode, Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum PortfolioKind {
    /// The default portfolio of a DID.
    Default,
    /// A user-defined portfolio of a DID.
    User(PortfolioNumber),
}

impl Default for PortfolioKind {
    fn default() -> Self {
        Self::Default
    }
}

impl From<Option<PortfolioNumber>> for PortfolioKind {
    fn from(num: Option<PortfolioNumber>) -> Self {
        num.map_or(Self::Default, Self::User)
    }
}

/// The ID of a portfolio.
#[derive(Decode, Encode, Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct PortfolioId {
    /// The DID of the portfolio.
    pub did: IdentityId,
    /// The kind of the portfolio: either default or user.
    pub kind: PortfolioKind,
}

impl Printable for PortfolioId {
    fn print(&self) {
        self.did.print();
        sp_io::misc::print_utf8(b"/");
        match self.kind {
            PortfolioKind::Default => {
                sp_io::misc::print_utf8(b"default");
            }
            PortfolioKind::User(num) => {
                sp_io::misc::print_num(num.0);
            }
        }
    }
}

impl PortfolioId {
    /// Returns the default portfolio of `did`.
    pub fn default_portfolio(did: IdentityId) -> Self {
        Self {
            did,
            kind: PortfolioKind::Default,
        }
    }

    /// Returns the user portfolio `num` of `did`.
    pub fn user_portfolio(did: IdentityId, num: PortfolioNumber) -> Self {
        Self {
            did,
            kind: PortfolioKind::User(num),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::assert_err;
    use std::convert::TryFrom;

    #[test]
    fn serialize_deserialize_identity() {
        let identity = IdentityId::from(999);
        println!("Print the un-serialize value: {:?}", identity);
        let serialize = serde_json::to_string(&identity).unwrap();
        let serialize_data =
            "\"0xe703000000000000000000000000000000000000000000000000000000000000\"";
        println!("Print the serialize data {:?}", serialize);
        assert_eq!(serialize_data, serialize);
        let deserialize = serde_json::from_str::<IdentityId>(&serialize).unwrap();
        println!("Print the deserialize data {:?}", deserialize);
        assert_eq!(identity, deserialize);
    }

    #[test]
    fn build_test() {
        assert_eq!(IdentityId::default().0, [0; 32]);
        let valid_did =
            hex::decode("f1d273950ddaf693db228084d63ef18282e00f91997ae9df4f173f09e86d0976")
                .expect("Decoding failed");
        let mut valid_did_without_prefix = [0; 32];
        valid_did_without_prefix.copy_from_slice(&valid_did);

        assert!(IdentityId::try_from(valid_did_without_prefix).is_ok());

        assert!(IdentityId::try_from(
            "did:poly:f1d273950ddaf693db228084d63ef18282e00f91997ae9df4f173f09e86d0976"
        )
        .is_ok());

        assert_err!(
            IdentityId::try_from(
                "did:OOLY:f1d273950ddaf693db228084d63ef18282e00f91997ae9df4f173f09e86d0976"
                    .as_bytes()
            ),
            "Missing 'did:poly:' prefix"
        );
        assert_err!(
            IdentityId::try_from("did:poly:a4a7"),
            "Invalid length of IdentityId"
        );

        assert_err!(
            IdentityId::try_from(
                "did:poly:f1d273950ddaf693db228084d63ef18282e00f91997ae9df4f173f09e86d097X"
            ),
            "DID code is not a valid hex"
        );

        let mut non_utf8: Vec<u8> =
            b"did:poly:f1d273950ddaf693db228084d63ef18282e00f91997ae9df4f173f09e86d".to_vec();
        non_utf8.append(&mut [0, 159, 146, 150].to_vec());
        assert_err!(
            IdentityId::try_from(non_utf8.as_slice()),
            "DID is not valid UTF-8"
        );
    }
}
