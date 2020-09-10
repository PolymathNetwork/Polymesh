use codec::{Decode, Encode};
use std::convert::TryInto;

#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq)]
pub enum Identifier {
    CUSIP([u8; 9]),
    CINS([u8; 9]),
    ISIN([u8; 12]),
    LEI([u8; 20]),
    EMPTY
}

impl Default for Identifier {
    fn default() -> Self {
        Identifier::EMPTY
    }
}

impl Identifier {
    pub fn cusip(bytes: [u8; 9]) -> Option<Identifier> {
        if cusip_checksum(&bytes[..8]) == bytes[8] - b'0' {
            return Some(Identifier::CUSIP(bytes));
        }
        None
    }

    pub fn cins(bytes: [u8; 9]) -> Option<Identifier> {
        if cusip_checksum(&bytes[..8]) == bytes[8] - b'0' {
            return Some(Identifier::CINS(bytes));
        }
        None
    }

    pub fn isin(bytes: [u8; 12]) -> Option<Identifier> {
        let s: String = bytes
            .iter()
            .map(|b| byte_value(*b))
            .map(|b| b.to_string())
            .collect();

        let mut s1 = 0;
        let mut s2 = 0;
        for (i, c) in s.chars().rev().enumerate() {
            let digit = c.to_digit(10)?;
            if i % 2 == 0 {
                s1 += digit;
            } else {
                s2 += 2 * digit;
                if digit >= 5 {
                    s2 -= 9;
                }
            }
        }

        if (s1 + s2) % 10 == 0 {
            return Some(Identifier::ISIN(bytes));
        }

        None
    }

    pub fn lei(bytes: [u8; 20]) -> Option<Identifier> {
        if lei_checksum(bytes[..18].try_into().ok()?)?
            == (bytes[18] - b'0') * 10 + (bytes[19] - b'0')
        {
            return Some(Identifier::LEI(bytes));
        }
        None
    }
}

fn cusip_checksum(bytes: &[u8]) -> u8 {
    let mut v = 0;
    let total = bytes.iter().enumerate().fold(0, |total, (i, c)| {
        v = byte_value(*c);
        if i % 2 != 0 {
            v *= 2
        }
        total + (v / 10) + v % 10
    });
    (10 - (total % 10)) % 10
}

fn lei_checksum(bytes: [u8; 18]) -> Option<u8> {
    let mut s = bytes
        .iter()
        .map(|b| byte_value(*b))
        .map(|b| b.to_string())
        .collect::<String>();
    s.push_str("00");
    Some(98 - (s.parse::<u128>().ok()? % 97) as u8)
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
            Identifier::cusip(*b"037833100"),
            Some(Identifier::CUSIP(*b"037833100"))
        );
        assert_eq!(
            Identifier::cusip(*b"17275R102"),
            Some(Identifier::CUSIP(*b"17275R102"))
        );
        assert_eq!(
            Identifier::cusip(*b"38259P508"),
            Some(Identifier::CUSIP(*b"38259P508"))
        );
        assert_eq!(
            Identifier::cusip(*b"594918104"),
            Some(Identifier::CUSIP(*b"594918104"))
        );
        assert_eq!(Identifier::cusip(*b"68389X106"), None);
        assert_eq!(
            Identifier::cusip(*b"68389X105"),
            Some(Identifier::CUSIP(*b"68389X105"))
        );
    }

    #[test]
    fn cins() {
        assert_eq!(
            Identifier::cins(*b"S08000AA9"),
            Some(Identifier::CINS(*b"S08000AA9"))
        );
        assert_eq!(Identifier::cins(*b"S08000AA4"), None);
    }

    #[test]
    fn isin() {
        assert_eq!(
            Identifier::isin(*b"US0378331005"),
            Some(Identifier::ISIN(*b"US0378331005"))
        );
        assert_eq!(
            Identifier::isin(*b"US0004026250"),
            Some(Identifier::ISIN(*b"US0004026250"))
        );
        assert_eq!(
            Identifier::isin(*b"AU0000XVGZA3"),
            Some(Identifier::ISIN(*b"AU0000XVGZA3"))
        );
        assert_eq!(
            Identifier::isin(*b"AU0000VXGZA3"),
            Some(Identifier::ISIN(*b"AU0000VXGZA3"))
        );
        assert_eq!(
            Identifier::isin(*b"FR0000988040"),
            Some(Identifier::ISIN(*b"FR0000988040"))
        );
        assert_eq!(Identifier::isin(*b"US0373831005"), None);
    }

    #[test]
    fn lei() {
        assert_eq!(
            Identifier::lei(*b"YZ83GD8L7GG84979J516"),
            Some(Identifier::LEI(*b"YZ83GD8L7GG84979J516"))
        );
        assert_eq!(
            Identifier::lei(*b"815600306702171A6844"),
            Some(Identifier::LEI(*b"815600306702171A6844"))
        );
        assert_eq!(
            Identifier::lei(*b"549300GFX6WN7JDUSN34"),
            Some(Identifier::LEI(*b"549300GFX6WN7JDUSN34"))
        );
        assert_eq!(Identifier::lei(*b"549300GFXDSN7JDUSN34"), None);
        assert!(lei_checksum(*b"ZZZZZZZZZZZZZZZZZZ").is_some());
    }
}
