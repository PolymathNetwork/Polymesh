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

use codec::{Decode, Encode};
use sp_std::prelude::Vec;

/// A string container where `PartialEq` uses ignore case sensitive.
#[derive(Encode, Decode, Default, Clone, Debug)]
pub struct IgnoredCaseString(Vec<u8>);

impl IgnoredCaseString {
    /// It returns a reference to internal data.
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl PartialEq for IgnoredCaseString {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_slice().eq_ignore_ascii_case(other.0.as_slice())
    }
}

impl PartialEq<&[u8]> for IgnoredCaseString {
    fn eq(&self, other: &&[u8]) -> bool {
        self.0.as_slice().eq_ignore_ascii_case(*other)
    }
}

impl PartialEq<&str> for IgnoredCaseString {
    fn eq(&self, other: &&str) -> bool {
        self.0.as_slice().eq_ignore_ascii_case(other.as_bytes())
    }
}

impl From<&[u8]> for IgnoredCaseString {
    fn from(s: &[u8]) -> Self {
        IgnoredCaseString(s.to_ascii_uppercase())
    }
}

impl From<&str> for IgnoredCaseString {
    fn from(s: &str) -> Self {
        IgnoredCaseString(s.to_ascii_uppercase().as_bytes().to_vec())
    }
}

impl From<Vec<u8>> for IgnoredCaseString {
    fn from(v: Vec<u8>) -> Self {
        IgnoredCaseString(v)
    }
}

#[cfg(test)]
mod tests {
    use super::IgnoredCaseString;

    #[test]
    fn from_with_upper_test() {
        let ics_from_str = IgnoredCaseString::from("lower case str");
        assert_eq!(ics_from_str.as_slice(), b"LOWER CASE STR");

        let ics_from_u8 = IgnoredCaseString::from("lower case u8".as_bytes());
        assert_eq!(ics_from_u8.as_slice(), "LOWER CASE U8".as_bytes());
    }

    #[test]
    fn eq_test() {
        let ics = IgnoredCaseString::from("Grüße, Jürgen ❤");
        assert_eq!(ics, "Grüße, JürGEN ❤");
        assert_eq!(ics, "Grüße, JürGEN ❤".as_bytes());

        let other_ics = IgnoredCaseString::from("Grüße, JüRGEN ❤".as_bytes().to_vec());
        assert_eq!(ics, other_ics);

        let disc_ics = IgnoredCaseString::from("Other");
        assert_ne!(ics, disc_ics);
    }
}
