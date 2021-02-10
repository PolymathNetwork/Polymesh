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
use polymesh_primitives_derive::VecU8StrongTyped;
use sp_std::prelude::Vec;

/// A wrapper for a token name.
#[derive(
    Decode, Encode, Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, VecU8StrongTyped,
)]
pub struct AssetName(pub Vec<u8>);

/// The type of security represented by a token.
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq)]
pub enum AssetType {
    /// Common stock - a security that represents ownership in a corporation.
    EquityCommon,
    /// Preferred stock. Preferred stockholders have a higher claim to dividends or asset
    /// distribution than common stockholders.
    EquityPreferred,
    /// Commodity - a basic good used in commerce that is interchangeable with other commodities of
    /// the same type.
    Commodity,
    /// Fixed income security - an investment that provides a return in the form of fixed periodic
    /// interest payments and the eventual return of principal at maturity. Examples: bonds,
    /// treasury bills, certificates of deposit.
    FixedIncome,
    /// Real estate investment trust - a company that owns, operates, or finances income-producing
    /// properties.
    REIT,
    /// Investment fund - a supply of capital belonging to numerous investors used to collectively
    /// purchase securities while each investor retains ownership and control of his own shares.
    Fund,
    /// Revenue share partnership agreement - a document signed by all partners in a partnership
    /// that has procedures when distributing business profits or losses.
    RevenueShareAgreement,
    /// Structured product, aka market-linked investment - a pre-packaged structured finance
    /// investment strategy based on a single security, a basket of securities, options, indices,
    /// commodities, debt issuance or foreign currencies, and to a lesser extent, derivatives.
    StructuredProduct,
    /// Derivative contract - a contract between two parties for buying or selling a security at a
    /// predetermined price within a specific time period. Examples: forwards, futures, options or
    /// swaps.
    Derivative,
    /// Anything else.
    Custom(Vec<u8>),
}

impl Default for AssetType {
    fn default() -> Self {
        Self::Custom(b"undefined".to_vec())
    }
}

/// A wrapper for a funding round name.
#[derive(
    Decode, Encode, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Default, VecU8StrongTyped,
)]
pub struct FundingRoundName(pub Vec<u8>);
