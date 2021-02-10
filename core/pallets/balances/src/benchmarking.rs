// This file is part of Substrate.

// Copyright (C) 2020 Parity Technologies (UK) Ltd.
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
// limitations under the License.

//! Balances pallet benchmarking.

use super::*;
use crate::Module as Balances;
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use polymesh_common_utilities::benchs::UserBuilder;

fn make_worst_memo() -> Option<Memo> {
    Some(Memo([7u8; 32]))
}

benchmarks! {
    _ { }

    // Benchmark `transfer` extrinsic with the worst possible conditions:
    // * Transfer will create the recipient account.
    transfer {
        let amount = T::Balance::from(500u32);
        let caller = UserBuilder::<T>::default().balance(1200u32).generate_did().build("caller");
        let recipient = UserBuilder::<T>::default().balance(0u32).generate_did().build( "recipient");
    }: _(caller.origin(), recipient.lookup(), amount)
    verify {
        assert_eq!(Balances::<T>::free_balance(&caller.account), (1200u32-500).into());
        assert_eq!(Balances::<T>::free_balance(&recipient.account), amount);
    }

    transfer_with_memo {
        let caller = UserBuilder::<T>::default().balance(1000u32).generate_did().build("caller");
        let recipient = UserBuilder::<T>::default().balance(0u32).generate_did().build("recipient");
        let amount = 42u32.into();
        let memo = make_worst_memo();

    }: _(caller.origin(), recipient.lookup(), amount, memo)
    verify {
        assert_eq!(Balances::<T>::free_balance(&caller.account), (1000u32-42).into());
        assert_eq!(Balances::<T>::free_balance(&recipient.account), amount);
    }

    deposit_block_reward_reserve_balance {
        let caller = UserBuilder::<T>::default().balance(1000u32).generate_did().build("caller");
        let amount = 500u32.into();
    }: _(caller.origin(), amount)
    verify {
        assert_eq!(Balances::<T>::free_balance(&caller.account), (1000u32-500).into());
        assert_eq!(Balances::<T>::block_rewards_reserve_balance(), amount);
    }

    set_balance {
        let caller = UserBuilder::<T>::default().balance(1000u32).generate_did().build("caller");
        let free_balance :T::Balance = 1_000_000u32.into();
        let reserved_balance :T::Balance = 100u32.into();
    }: _(RawOrigin::Root, caller.lookup(), free_balance.clone(), reserved_balance)
    verify {
        assert_eq!(Balances::<T>::free_balance(&caller.account), free_balance);
        assert_eq!(Balances::<T>::reserved_balance(&caller.account), reserved_balance);
    }

    force_transfer {
        let source = UserBuilder::<T>::default().balance(1000u32).generate_did().build("source");
        let dest = UserBuilder::<T>::default().balance(1u32).generate_did().build("dest");
        let amount = 500u32.into();
    }: _(RawOrigin::Root, source.lookup(), dest.lookup(), amount)
    verify {
        assert_eq!(Balances::<T>::free_balance(&source.account), (1000u32-500).into());
        assert_eq!(Balances::<T>::free_balance(&dest.account), (1u32+500).into());
    }

    burn_account_balance {
        let caller = UserBuilder::<T>::default().balance(1000u32).generate_did().build("caller");
        let amount = 500u32.into();
    }: _(caller.origin(), amount)
    verify {
        assert_eq!(Balances::<T>::free_balance(&caller.account), (1000u32-500).into());
    }
}
