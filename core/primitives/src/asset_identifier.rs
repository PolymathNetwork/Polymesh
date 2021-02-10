use codec::{Decode, Encode};
use core::convert::TryInto;

/// Implementation of common asset identifiers
/// https://www.cusip.com/identifiers.html
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq)]
pub enum AssetIdentifier {
    /// Universally recognized identifier for financial instruments
    /// Example: Amazon.com Inc - Common Stock
    /// ISSUER ISSUE CHECK CUSIP
    /// 023135 10    6     023135106
    CUSIP([u8; 9]),
    /// The CUSIP International Numbering System
    /// Example: Abingdon Capital PLC - Shares
    /// COUNTRY CODE ISSUER ISSUE CHECK CINS
    /// G            0052B  10    5     G0052B105
    CINS([u8; 9]),
    /// The International Securities Identification Number
    /// Example:
    /// COUNTRY CODE LOCAL IDENTIFIER CHECK ISIN
    /// CA           008911703        4     CA0089117034
    ISIN([u8; 12]),
    /// The Legal Entity Identifier
    /// Example: Philadelphia Cheesesteak Company
    /// LOU PREFIX ENTITY INDENTIFIER VERIFICATION ID LEI
    /// 5493       00SAMIRN1R27UP     42              549300SAMIRN1R27UP42
    LEI([u8; 20]),
}

impl AssetIdentifier {
    /// Validate `bytes` is a valid CUSIP identifier, returns an instance of `Identifier` if successful
    pub fn cusip(bytes: [u8; 9]) -> Option<AssetIdentifier> {
        (cusip_checksum(&bytes[..8]) == bytes[8] - b'0').then_some(AssetIdentifier::CUSIP(bytes))
    }

    /// Validate `bytes` is a valid CINS identifier, returns an instance of `Identifier` if successful
    pub fn cins(bytes: [u8; 9]) -> Option<AssetIdentifier> {
        Self::cusip(bytes).map(|_| AssetIdentifier::CINS(bytes))
    }

    /// Validate `bytes` is a valid ISIN identifier, returns an instance of `Identifier` if successful
    pub fn isin(bytes: [u8; 12]) -> Option<AssetIdentifier> {
        enum UpToTwo {
            Zero,
            One(u8),
            Two(u8, u8),
        }
        impl Iterator for UpToTwo {
            type Item = u8;
            fn next(&mut self) -> Option<Self::Item> {
                let (this, next) = match self {
                    Self::Zero => return None,
                    Self::One(x) => (Self::Zero, Some(*x)),
                    Self::Two(x, y) => (Self::One(*y), Some(*x)),
                };
                *self = this;
                next
            }
        }
        impl core::iter::DoubleEndedIterator for UpToTwo {
            fn next_back(&mut self) -> Option<Self::Item> {
                let (this, next) = match self {
                    Self::Zero => return None,
                    Self::One(x) => (Self::Zero, Some(*x)),
                    Self::Two(x, y) => (Self::One(*x), Some(*y)),
                };
                *self = this;
                next
            }
        }

        let (s1, s2) = bytes
            .iter()
            .copied()
            .map(byte_value)
            .flat_map(|b| {
                if b > 9 {
                    UpToTwo::Two(b / 10, b % 10)
                } else {
                    UpToTwo::One(b)
                }
            })
            .rev()
            .enumerate()
            .fold((0u8, 0u8), |(mut s1, mut s2), (i, digit)| {
                if i % 2 == 0 {
                    s1 = s1.wrapping_add(digit);
                } else {
                    s2 = s2.wrapping_add(2u8.wrapping_mul(digit));
                    if digit >= 5 {
                        s2 = s2.wrapping_sub(9);
                    }
                }
                (s1, s2)
            });
        (s1.wrapping_add(s2) % 10 == 0).then_some(AssetIdentifier::ISIN(bytes))
    }

    /// Validate `bytes` is a valid LEI identifier, returns an instance of `Identifier` if successful
    pub fn lei(bytes: [u8; 20]) -> Option<AssetIdentifier> {
        bytes[..18]
            .try_into()
            .ok()
            .map(lei_checksum)
            .filter(|hash| {
                *hash
                    == (bytes[18].wrapping_sub(b'0'))
                        .wrapping_mul(10)
                        .wrapping_add(bytes[19].wrapping_sub(b'0'))
            })
            .map(|_| AssetIdentifier::LEI(bytes))
    }

    /// Ensures the identifier is valid.
    /// Mainly used for validating manual constructions of the enum (user input).
    pub fn validate(self) -> Option<Self> {
        match self {
            AssetIdentifier::CUSIP(bytes) => Self::cusip(bytes),
            AssetIdentifier::CINS(bytes) => Self::cins(bytes),
            AssetIdentifier::ISIN(bytes) => Self::isin(bytes),
            AssetIdentifier::LEI(bytes) => Self::lei(bytes),
        }
    }
}

fn cusip_checksum(bytes: &[u8]) -> u8 {
    let total: usize = bytes
        .iter()
        .copied()
        .map(byte_value)
        .enumerate()
        .map(|(i, v)| v << (i % 2))
        .map(|v| (v / 10).wrapping_add(v % 10))
        .map(|x| x as usize)
        .sum();
    ((10 - (total % 10)) % 10) as u8
}

fn lei_checksum(bytes: [u8; 18]) -> u8 {
    let mut i = 0;
    let total = bytes
        .iter()
        .copied()
        .rev()
        .map(byte_value)
        .map(|x| x as u128)
        .fold(0u128, |total, b| {
            let total = total.wrapping_add(b.wrapping_mul(10u128.wrapping_pow(i as u32)));
            i += if b > 9 { 2 } else { 1 };
            total
        });
    (98 - (total.wrapping_mul(100) % 97)) as u8
}

fn byte_value(b: u8) -> u8 {
    match b {
        b'*' => 36,
        b'@' => 37,
        b'#' => 38,
        b'0'..=b'9' => b - b'0',
        b'A'..=b'Z' => b - b'A' + 1 + 9,
        b'a'..=b'z' => b - 0x20 - b'A' + 1 + 9,
        _ => b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cusip() {
        assert_eq!(
            AssetIdentifier::cusip(*b"037833100"),
            Some(AssetIdentifier::CUSIP(*b"037833100"))
        );
        assert_eq!(
            AssetIdentifier::cusip(*b"17275R102"),
            Some(AssetIdentifier::CUSIP(*b"17275R102"))
        );
        assert_eq!(
            AssetIdentifier::cusip(*b"38259P508"),
            Some(AssetIdentifier::CUSIP(*b"38259P508"))
        );
        assert_eq!(
            AssetIdentifier::cusip(*b"594918104"),
            Some(AssetIdentifier::CUSIP(*b"594918104"))
        );
        assert_eq!(AssetIdentifier::cusip(*b"68389X106"), None);
        assert_eq!(
            AssetIdentifier::cusip(*b"68389X105"),
            Some(AssetIdentifier::CUSIP(*b"68389X105"))
        );
    }

    #[test]
    fn cins() {
        assert_eq!(
            AssetIdentifier::cins(*b"S08000AA9"),
            Some(AssetIdentifier::CINS(*b"S08000AA9"))
        );
        assert_eq!(AssetIdentifier::cins(*b"S08000AA4"), None);
    }

    #[test]
    fn isin() {
        assert_eq!(
            AssetIdentifier::isin(*b"US0378331005"),
            Some(AssetIdentifier::ISIN(*b"US0378331005"))
        );
        assert_eq!(
            AssetIdentifier::isin(*b"US0004026250"),
            Some(AssetIdentifier::ISIN(*b"US0004026250"))
        );
        assert_eq!(
            AssetIdentifier::isin(*b"AU0000XVGZA3"),
            Some(AssetIdentifier::ISIN(*b"AU0000XVGZA3"))
        );
        assert_eq!(
            AssetIdentifier::isin(*b"AU0000VXGZA3"),
            Some(AssetIdentifier::ISIN(*b"AU0000VXGZA3"))
        );
        assert_eq!(
            AssetIdentifier::isin(*b"FR0000988040"),
            Some(AssetIdentifier::ISIN(*b"FR0000988040"))
        );
        assert_eq!(AssetIdentifier::isin(*b"US0373831005"), None);
    }

    #[test]
    fn lei() {
        assert_eq!(
            AssetIdentifier::lei(*b"YZ83GD8L7GG84979J516"),
            Some(AssetIdentifier::LEI(*b"YZ83GD8L7GG84979J516"))
        );
        assert_eq!(
            AssetIdentifier::lei(*b"815600306702171A6844"),
            Some(AssetIdentifier::LEI(*b"815600306702171A6844"))
        );
        assert_eq!(
            AssetIdentifier::lei(*b"549300GFX6WN7JDUSN34"),
            Some(AssetIdentifier::LEI(*b"549300GFX6WN7JDUSN34"))
        );
        assert_eq!(AssetIdentifier::lei(*b"549300GFXDSN7JDUSN34"), None);
    }
}
