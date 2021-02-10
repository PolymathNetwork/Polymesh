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

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

pub mod cdd_check;
pub mod impls;

pub use cdd_check::CddChecker;
pub use sp_runtime::{Perbill, Permill};

pub use frame_support::weights::{
    constants::{WEIGHT_PER_MICROS, WEIGHT_PER_MILLIS, WEIGHT_PER_NANOS, WEIGHT_PER_SECOND},
    GetDispatchInfo, Weight,
};
use frame_support::{
    parameter_types,
    traits::Currency,
    weights::{
        RuntimeDbWeight, WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
    },
};
use frame_system::{self as system};
use pallet_balances as balances;
use polymesh_common_utilities::constants::currency::*;
use polymesh_primitives::{Balance, BlockNumber, IdentityId, Moment};
use smallvec::smallvec;

pub use impls::{Author, CurrencyToVoteHandler};

pub type NegativeImbalance<T> =
    <balances::Module<T> as Currency<<T as system::Trait>::AccountId>>::NegativeImbalance;

parameter_types! {
    pub const BlockHashCount: BlockNumber = 250;
    /// We allow for 2 seconds of compute with a 6 second average block time.
    ///
    /// If this is updated, `PipsEnactSnapshotMaximumWeight` needs to be updated accordingly.
    pub const MaximumBlockWeight: Weight = 2 * WEIGHT_PER_SECOND;
    /// Portion of the block available to normal class of dispatches.
    ///
    /// If this is updated, `PipsEnactSnapshotMaximumWeight` needs to be updated accordingly.
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    /// Blocks can be of upto 10 MB in size.
    pub const MaximumBlockLength: u32 = 10 * 1024 * 1024;
    /// 20 ms is needed to create a block.
    pub const BlockExecutionWeight: Weight = 20 * WEIGHT_PER_MILLIS;
    /// 0.65 ms is needed to process an empty extrinsic.
    pub const ExtrinsicBaseWeight: Weight = 650 * WEIGHT_PER_MICROS;
    /// When the read/writes are cached/buffered, they take 25/100 microseconds on NVMe disks.
    /// When they are uncached, they take 250/450 microseconds on NVMe disks.
    /// Most read will be cached and writes will be buffered in production.
    /// We are taking a number slightly higher than what cached suggest to allow for some extra breathing room.
    pub const RocksDbWeight: RuntimeDbWeight = RuntimeDbWeight {
        read: 50 * WEIGHT_PER_MICROS,   // ~100 µs @ 100,000 items
        write: 200 * WEIGHT_PER_MICROS, // ~200 µs @ 100,000 items
    };
    /// This implies a 100 POLYX fee per MB of transaction length
    pub const TransactionByteFee: Balance = 10 * MILLICENTS;
    /// We want the noop transaction to cost 0.03 POLYX
    pub const PolyXBaseFee: Balance = 3 * CENTS;
}

/// Converts Weight to Fee
pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
    type Balance = Balance;
    /// We want a 0.03 POLYX fee per ExtrinsicBaseWeight.
    /// 650_000_000 weight = 30_000 fee => 21_666 weight = 1 fee.
    /// Hence, 1 fee = 0 + 1/21_666 weight.
    /// This implies, coeff_integer = 0 and coeff_frac = 1/21_666.
    fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
        smallvec![WeightToFeeCoefficient {
            degree: 1,
            coeff_frac: Perbill::from_rational_approximation(
                PolyXBaseFee::get().into(),
                ExtrinsicBaseWeight::get() as u128
            ),
            coeff_integer: 0u128, // Coefficient is zero.
            negative: false,
        }]
    }
}

use pallet_group_rpc_runtime_api::Member;
use polymesh_common_utilities::traits::group::InactiveMember;
use sp_std::{convert::From, prelude::*};

/// It merges actives and in-actives members.
pub fn merge_active_and_inactive<Block>(
    active: Vec<IdentityId>,
    inactive: Vec<InactiveMember<Moment>>,
) -> Vec<Member> {
    active
        .into_iter()
        .map(Member::from)
        .chain(inactive.into_iter().map(Member::from))
        .collect()
}
