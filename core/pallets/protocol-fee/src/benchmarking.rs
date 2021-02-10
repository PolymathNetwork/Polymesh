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

use crate::*;
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use polymesh_common_utilities::protocol_fee::ProtocolOp;
use polymesh_primitives::PosRatio;
use sp_std::prelude::*;

benchmarks! {
    _ {}

    change_coefficient {
        let n in 0 .. u32::MAX;
        let d in 0 .. u32::MAX;

        let origin = RawOrigin::Root;
        let coefficient = PosRatio::from((n, d));
    }: _(origin, coefficient)

    change_base_fee {
        let b in 0 .. u32::MAX;

        let origin = RawOrigin::Root;
        let op = ProtocolOp::AssetRegisterTicker;
        let base_fee = b.into();
    }: _(origin, op, base_fee)
}
