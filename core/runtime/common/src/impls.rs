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

// This file is part of Substrate.

// Copyright (C) 2019-2020 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License

//! Some configurable implementations as associated type for the substrate runtime.
//! Auxillary struct/enums

use crate::NegativeImbalance;
use frame_support::traits::{Currency, OnUnbalanced};
use frame_system as system;
use pallet_authorship as authorship;
use pallet_balances as balances;
use polymesh_primitives::Balance;
use sp_runtime::traits::Convert;

pub struct Author<R>(sp_std::marker::PhantomData<R>);

impl<R> OnUnbalanced<NegativeImbalance<R>> for Author<R>
where
    R: balances::Trait + authorship::Trait,
    <R as system::Trait>::AccountId: From<polymesh_primitives::AccountId>,
    <R as system::Trait>::AccountId: Into<polymesh_primitives::AccountId>,
{
    fn on_nonzero_unbalanced(amount: NegativeImbalance<R>) {
        <balances::Module<R>>::resolve_creating(&<authorship::Module<R>>::author(), amount);
    }
}

/// Struct that handles the conversion of Balance -> `u128`. This is used for staking's election
/// calculation.
pub struct CurrencyToVoteHandler<R>(sp_std::marker::PhantomData<R>);

impl<R> CurrencyToVoteHandler<R>
where
    R: balances::Trait,
    R::Balance: Into<Balance>,
{
    fn factor() -> Balance {
        let issuance: Balance = <balances::Module<R>>::total_issuance().into();
        (issuance / u64::max_value() as Balance).max(1)
    }
}

impl<R> Convert<Balance, u64> for CurrencyToVoteHandler<R>
where
    R: balances::Trait,
    R::Balance: Into<Balance>,
{
    fn convert(x: Balance) -> u64 {
        (x / Self::factor()) as u64
    }
}

impl<R> Convert<u128, Balance> for CurrencyToVoteHandler<R>
where
    R: balances::Trait,
    R::Balance: Into<Balance>,
{
    fn convert(x: u128) -> Balance {
        x * Self::factor()
    }
}
