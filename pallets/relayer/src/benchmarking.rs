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
use polymesh_common_utilities::{
    benchs::{user, AccountIdOf, User},
    traits::{relayer::Config, TestUtilsFn},
};
use sp_std::prelude::*;

type Relayer<T> = crate::Module<T>;

pub(crate) const SEED: u32 = 0;

fn setup_users<T: Config + TestUtilsFn<AccountIdOf<T>>>() -> (User<T>, User<T>) {
    let payer = user::<T>("payer", SEED);
    let user = user::<T>("user", SEED);
    (payer, user)
}

fn setup_paying_key<T: Config + TestUtilsFn<AccountIdOf<T>>>() -> (User<T>, User<T>) {
    let (payer, user) = setup_users::<T>();
    // accept paying key
    <Relayer<T>>::auth_accept_paying_key(
        user.did(),
        payer.did(),
        user.account(),
        payer.account(),
        0u128.into(),
    )
    .unwrap();
    (payer, user)
}

benchmarks! {
    where_clause { where T: Config, T: TestUtilsFn<AccountIdOf<T>> }

    set_paying_key {
        let (payer, user) = setup_users::<T>();
    }: _(payer.origin(), user.account(), 0u128.into())

    accept_paying_key {
        let (payer, user) = setup_users::<T>();
        let limit = 100u128;
        // setup authorization
        let auth_id = <Relayer<T>>::unverified_add_auth_for_paying_key(
            payer.did(), user.account(), payer.account(), limit.into()
        );
    }: _(user.origin(), auth_id)
    verify {
        assert_eq!(Subsidies::<T>::get(user.account()), Some(Subsidy {
            paying_key: payer.account(),
            remaining: limit.into(),
        }));
    }

    remove_paying_key {
        let (payer, user) = setup_paying_key::<T>();
    }: _(payer.origin(), user.account(), payer.account())
    verify {
        assert_eq!(Subsidies::<T>::get(user.account()), None);
    }

    update_polyx_limit {
        let limit = 1_000u128;
        let (payer, user) = setup_paying_key::<T>();
    }: _(payer.origin(), user.account(), limit.into())
    verify {
        assert_eq!(Subsidies::<T>::get(user.account()), Some(Subsidy {
            paying_key: payer.account(),
            remaining: limit.into(),
        }));
    }
}
