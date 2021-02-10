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

use super::*;
use crate::benchmarking::{currency, did_whts, set_ca_targets, setup_ca, SEED};
use crate::{CAKind, CorporateActions};
use frame_benchmarking::benchmarks;
use pallet_compliance_manager::Module as ComplianceManager;
use pallet_portfolio::MovePortfolioItem;
use polymesh_common_utilities::{
    asset::AssetFnTrait,
    benchs::{user, User},
};
use polymesh_primitives::{PortfolioId, PortfolioNumber, Ticker};
const MAX_TARGETS: u32 = 1000;
const MAX_DID_WHT_IDS: u32 = 1000;

fn portfolio<T: Trait>(owner: &User<T>, pnum: PortfolioNumber, ticker: Ticker, amount: T::Balance) {
    let did = owner.did();
    let origin: T::Origin = owner.origin().into();
    <Portfolio<T>>::create_portfolio(origin.clone(), "portfolio".into()).unwrap();
    <Portfolio<T>>::move_portfolio_funds(
        origin,
        PortfolioId::default_portfolio(did),
        PortfolioId::user_portfolio(did, pnum),
        vec![MovePortfolioItem { ticker, amount }],
    )
    .unwrap();
}

fn dist<T: Trait>(target_ids: u32) -> (User<T>, CAId, Ticker) {
    let (owner, ca_id) = setup_ca::<T>(CAKind::UnpredictableBenefit);

    let currency = currency::<T>(&owner);
    let amount = 1000u32.into();
    let pnum = 1u64.into();
    portfolio::<T>(&owner, pnum, currency, amount);

    <Module<T>>::distribute(
        owner.origin().into(),
        ca_id,
        Some(pnum),
        currency,
        amount,
        3000,
        Some(4000),
    )
    .unwrap();

    set_ca_targets::<T>(ca_id, target_ids);

    (owner, ca_id, currency)
}

fn prepare_transfer<T: Trait + pallet_compliance_manager::Trait>(
    target_ids: u32,
    did_whts_num: u32,
) -> (User<T>, User<T>, CAId) {
    let (owner, ca_id, currency) = dist::<T>(target_ids);

    CorporateActions::mutate(ca_id.ticker, ca_id.local_id, |ca| {
        let mut whts = did_whts::<T>(did_whts_num);
        whts.sort_by_key(|(did, _)| *did);
        ca.as_mut().unwrap().withholding_tax = whts;
    });

    <pallet_timestamp::Now<T>>::set(3000u32.into());

    let holder = user::<T>("holder", SEED);
    <T as pallet_compliance_manager::Trait>::Asset::add_investor_uniqueness_claim(
        owner.did(),
        currency,
    );
    <T as pallet_compliance_manager::Trait>::Asset::add_investor_uniqueness_claim(
        holder.did(),
        currency,
    );
    <ComplianceManager<T>>::add_compliance_requirement(
        owner.origin().into(),
        currency,
        vec![],
        vec![],
    )
    .unwrap();

    (owner, holder, ca_id)
}

benchmarks! {
    where_clause { where T: pallet_compliance_manager::Trait }

    _ {}

    distribute {
        let (owner, ca_id) = setup_ca::<T>(CAKind::UnpredictableBenefit);
        let currency = currency::<T>(&owner);
        let amount = 1000u32.into();
        let pnum =1u64.into();
        portfolio::<T>(&owner, pnum, currency, amount);
    }: _(owner.origin(), ca_id, Some(pnum), currency, amount, 3000, Some(4000))
    verify {
        ensure!(<Distributions<T>>::get(ca_id).is_some(), "distribution not created");
    }

    claim {
        let t in 0..MAX_TARGETS;
        let w in 0..MAX_DID_WHT_IDS;

        let (_, holder, ca_id) = prepare_transfer::<T>(t, w);
    }: _(holder.origin(), ca_id)
    verify {
        ensure!(HolderPaid::get((ca_id, holder.did())), "not paid");
    }

    push_benefit {
        let t in 0..MAX_TARGETS;
        let w in 0..MAX_DID_WHT_IDS;

        let (owner, holder, ca_id) = prepare_transfer::<T>(t, w);
    }: _(owner.origin(), ca_id, holder.did())
    verify {
        ensure!(HolderPaid::get((ca_id, holder.did())), "not paid");
    }

    reclaim {
        let (owner, ca_id, currency) = dist::<T>(0);

        <pallet_timestamp::Now<T>>::set(5000u32.into());
    }: _(owner.origin(), ca_id)
    verify {
        ensure!(<Distributions<T>>::get(ca_id).unwrap().reclaimed, "not reclaimed");
    }

    remove_distribution {
        let (owner, ca_id, currency) = dist::<T>(0);
    }: _(owner.origin(), ca_id)
    verify {
        ensure!(<Distributions<T>>::get(ca_id).is_none(), "not removed");
    }
}
