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
use polymesh_common_utilities::benchs::{generate_ticker, UserBuilder};
use polymesh_primitives::PortfolioName;
use sp_std::{convert::TryFrom, prelude::*};

benchmarks! {
    _ {}

    create_portfolio {
        // Length of portfolio name
        let i in 1 .. 500;

        let target = UserBuilder::<T>::default().generate_did().build("target");
        let did = target.did();
        let portfolio_name = PortfolioName(vec![65u8; i as usize]);
        let next_portfolio_num = NextPortfolioNumber::get(&did);
    }: _(target.origin, portfolio_name.clone())
    verify {
        assert_eq!(Portfolios::get(&did, &next_portfolio_num), portfolio_name);
    }

    delete_portfolio {
        let target = UserBuilder::<T>::default().generate_did().build("target");
        let did = target.did();
        let portfolio_name = PortfolioName(vec![65u8; 5]);
        let next_portfolio_num = NextPortfolioNumber::get(&did);
        Module::<T>::create_portfolio(target.origin.clone().into(), portfolio_name.clone())?;
        assert_eq!(Portfolios::get(&did, &next_portfolio_num), portfolio_name);
    }: _(target.origin, next_portfolio_num.clone())
    verify {
        assert!(!Portfolios::contains_key(&did, &next_portfolio_num));
    }

    move_portfolio_funds {
        // Number of assets being moved
        let a in 1 .. 500;
        let mut items = Vec::with_capacity(a as usize);
        let target = UserBuilder::<T>::default().generate_did().build("target");
        let first_ticker = Ticker::try_from(generate_ticker(0u64).as_slice()).unwrap();
        let amount = T::Balance::from(10u32);
        let portfolio_name = PortfolioName(vec![65u8; 5]);
        let next_portfolio_num = NextPortfolioNumber::get(&target.did());
        let default_portfolio = PortfolioId::default_portfolio(target.did());
        let user_portfolio = PortfolioId::user_portfolio(target.did(), next_portfolio_num.clone());

        for x in 0..a as u64 {
            let ticker = Ticker::try_from(generate_ticker(x).as_slice()).unwrap();
            items.push(MovePortfolioItem {
                ticker,
                amount: amount,
            });
            <PortfolioAssetBalances<T>>::insert(&default_portfolio, &ticker, amount);
        }

        Module::<T>::create_portfolio(target.origin.clone().into(), portfolio_name.clone())?;

        assert_eq!(<PortfolioAssetBalances<T>>::get(&default_portfolio, &first_ticker), amount);
        assert_eq!(<PortfolioAssetBalances<T>>::get(&user_portfolio, &first_ticker), 0u32.into());
    }: _(target.origin, default_portfolio, user_portfolio, items)
    verify {
        assert_eq!(<PortfolioAssetBalances<T>>::get(&default_portfolio, &first_ticker), 0u32.into());
        assert_eq!(<PortfolioAssetBalances<T>>::get(&user_portfolio, &first_ticker), amount);
    }

    rename_portfolio {
        // Length of portfolio name
        let i in 1 .. 500;

        let target = UserBuilder::<T>::default().generate_did().build("target");
        let did = target.did();
        let portfolio_name = PortfolioName(vec![65u8; i as usize]);
        let next_portfolio_num = NextPortfolioNumber::get(&did);
        Module::<T>::create_portfolio(target.origin.clone().into(), portfolio_name.clone())?;
        assert_eq!(Portfolios::get(&did, &next_portfolio_num), portfolio_name);
        let new_name = PortfolioName(vec![66u8; i as usize]);

    }: _(target.origin, next_portfolio_num.clone(), new_name.clone())
    verify {
        assert_eq!(Portfolios::get(&did, &next_portfolio_num), new_name);
    }
}
