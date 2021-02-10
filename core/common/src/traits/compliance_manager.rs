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

use core::result::Result;
use frame_support::{dispatch::DispatchError, weights::Weight};
use polymesh_primitives::{IdentityId, Ticker};

pub trait Trait<Balance> {
    fn verify_restriction(
        ticker: &Ticker,
        from_id: Option<IdentityId>,
        to_id: Option<IdentityId>,
        _value: Balance,
        primary_issuance_agent: IdentityId,
    ) -> Result<u8, DispatchError>;
}

pub trait WeightInfo {
    fn add_compliance_requirement(s: u32, r: u32) -> Weight;
    fn remove_compliance_requirement() -> Weight;
    fn pause_asset_compliance() -> Weight;
    fn resume_asset_compliance() -> Weight;
    fn add_default_trusted_claim_issuer() -> Weight;
    fn remove_default_trusted_claim_issuer() -> Weight;
    fn change_compliance_requirement(s: u32, r: u32) -> Weight;
    fn replace_asset_compliance(c: u32) -> Weight;
    fn reset_asset_compliance() -> Weight;
}
