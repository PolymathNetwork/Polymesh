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

//! Ticker symbol
use codec::{Decode, Encode, Error, Input};
#[cfg(feature = "std")]
use polymesh_primitives_derive::{DeserializeU8StrongTyped, SerializeU8StrongTyped};

use sp_std::convert::TryFrom;

/// Ticker length.
pub const TICKER_LEN: usize = 12;

/// Ticker symbol.
///
/// This type stores fixed-length case-sensitive byte strings. Any value of this type that is
/// received by a Substrate module call method has to be converted to canonical uppercase
/// representation using [`Ticker::canonize`].
#[derive(Encode, Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(
    feature = "std",
    derive(SerializeU8StrongTyped, DeserializeU8StrongTyped)
)]
pub struct Ticker([u8; TICKER_LEN]);

impl Default for Ticker {
    fn default() -> Self {
        Ticker([0u8; TICKER_LEN])
    }
}

impl AsRef<[u8]> for Ticker {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl TryFrom<&[u8]> for Ticker {
    type Error = Error;

    fn try_from(s: &[u8]) -> Result<Self, Self::Error> {
        let len = s.len();
        if len > TICKER_LEN {
            return Err("ticker too long".into());
        }
        let mut inner = [0u8; TICKER_LEN];
        inner[..len].copy_from_slice(s);
        inner.make_ascii_uppercase();
        // Check whether the given ticker contains no lowercase characters and return an error
        // otherwise.
        if &inner[..len] == s {
            Ok(Ticker(inner))
        } else {
            Err("lowercase ticker".into())
        }
    }
}

/// It custom decoder enforces to upper case.
impl Decode for Ticker {
    fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
        let inner = <[u8; TICKER_LEN]>::decode(input)?;
        Self::try_from(&inner[..])
    }
}

impl Ticker {
    /// Computes the effective length of the ticker, that is, the length of the minimal prefix after
    /// which only zeros appear.
    pub fn len(&self) -> usize {
        for i in (0..TICKER_LEN).rev() {
            if self.0[i] != 0 {
                return i + 1;
            }
        }
        0
    }
    /// Returns `true` if the ticker is empty, that is, if it has no prefix of characters other than
    /// `0u8`.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0[0] == 0
    }

    /// Returns the ticker as a slice.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Returns the ticker as a fixed-size array.
    #[inline]
    pub fn as_bytes(&self) -> &[u8; TICKER_LEN] {
        &(self.0)
    }

    /// Returns an iterator over the ticker.
    #[inline]
    pub fn iter(&self) -> sp_std::slice::Iter<'_, u8> {
        self.0.iter()
    }
}

#[cfg(feature = "runtime-benchmarks")]
impl Ticker {
    /// Create ticker by repeating `b` for `TICKER_LEN`
    pub const fn repeating(b: u8) -> Ticker {
        // TODO: replace with u8::to_ascii_uppercase when it's const
        const fn to_ascii_uppercase(b: u8) -> u8 {
            b & !((b.is_ascii_lowercase() as u8) << 5)
        }
        Self([to_ascii_uppercase(b); TICKER_LEN])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialization_deserialization_test() {
        let ticker_name: Vec<u8> = (vec![0x45, 0x32, 0x43]).into();
        let ticker = Ticker::try_from(ticker_name.as_slice()).unwrap();
        let serialize = serde_json::to_string(&ticker).unwrap();
        let serialize_data = "\"0x453243000000000000000000\"";
        assert_eq!(serialize_data, serialize);
        let deserialize = serde_json::from_str::<Ticker>(&serialize).unwrap();
        assert_eq!(ticker, deserialize);
    }

    #[test]
    fn ticker_test() {
        // 1. Happy path.
        let s1 = b"ABCDABCDABCD";
        let t1 = Ticker::try_from(&s1[..]).unwrap();
        assert_eq!(t1.len(), 12);
        assert_eq!(t1.as_slice(), b"ABCDABCDABCD");

        // 2. More characters than expected.
        let s2 = b"abcdabcdabcdabcd";
        let t2 = Ticker::try_from(&s2[..]);
        assert_eq!(t2, Err("ticker too long".into()));

        // 3. Lowercase characters.
        let s3 = b"abcd";
        let t3 = Ticker::try_from(&s3[..]);
        assert_eq!(t3, Err("lowercase ticker".into()));
    }

    #[test]
    fn parity_scale_codec() {
        let s = b"ACME";
        let t = Ticker::try_from(&s[..]).unwrap();

        let t_encoded = t.encode();
        let t2 = Ticker::decode(&mut &t_encoded[..]).unwrap();

        assert_eq!(t, t2);
    }
}
