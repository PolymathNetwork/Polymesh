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

//! Runtime API definition for the protocol fee module.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use polymesh_common_utilities::protocol_fee::ProtocolOp;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{SaturatedConversion, UniqueSaturatedInto};

/// A capped version of `Balance` which is normally a `u128`, fit into `u64` which is a serializable
/// type unlike `u128`. There are no fees that would not fit into `u64`.
#[derive(Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct CappedFee(pub u64);

impl<T: UniqueSaturatedInto<u64>> From<T> for CappedFee {
    fn from(fee: T) -> CappedFee {
        CappedFee(fee.saturated_into())
    }
}

sp_api::decl_runtime_apis! {
    pub trait ProtocolFeeApi {
        fn compute_fee(op: ProtocolOp) -> CappedFee;
    }
}
