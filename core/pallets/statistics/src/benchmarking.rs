use crate::*;
use core::convert::TryFrom;
use frame_benchmarking::benchmarks;
use polymesh_common_utilities::benchs::{User, UserBuilder};
use polymesh_primitives::asset::AssetType;
use sp_std::prelude::*;

/// Create a new token with name `name` on behalf of `owner`.
/// The new token is a _divisible_ one with 1_000_000 units.
pub fn make_token<T: Trait>(owner: &User<T>, name: Vec<u8>) -> Ticker {
    let ticker = Ticker::try_from(name.as_slice()).unwrap();
    T::Asset::create_asset(
        owner.origin.clone().into(),
        name.into(),
        ticker.clone(),
        1_000_000u32.into(),
        true,
        AssetType::default(),
        Vec::new(),
        None,
    )
    .expect("Cannot create an asset");

    ticker
}

fn init_ticker<T: Trait>() -> (User<T>, Ticker) {
    let owner = UserBuilder::<T>::default().generate_did().build("OWNER");
    let ticker = make_token::<T>(&owner, b"1".to_vec());
    (owner, ticker)
}

fn init_ctm<T: Trait>(
    max_transfer_manager_per_asset: u32,
) -> (User<T>, Ticker, Vec<TransferManager>) {
    let (owner, ticker) = init_ticker::<T>();
    let tms = (0..max_transfer_manager_per_asset)
        .map(|x| TransferManager::CountTransferManager(x.into()))
        .collect::<Vec<_>>();
    ActiveTransferManagers::insert(ticker, tms.clone());
    (owner, ticker, tms)
}

#[cfg(feature = "running-ci")]
mod limits {
    pub const MAX_EXEMPTED_IDENTITIES: u32 = 10;
}

#[cfg(not(feature = "running-ci"))]
mod limits {
    pub const MAX_EXEMPTED_IDENTITIES: u32 = 1000;
}

benchmarks! {
    _ {}

    add_transfer_manager {
        let max_tm = T::MaxTransferManagersPerAsset::get().saturating_sub(1);
        let (owner, ticker, mut tms) = init_ctm::<T>(max_tm);

        let last_tm = TransferManager::CountTransferManager(420);
        tms.push(last_tm.clone());
    }: _(owner.origin, ticker, last_tm)
    verify {
        assert!(Module::<T>::transfer_managers(ticker) == tms);
    }

    remove_transfer_manager {
        let (owner, ticker, mut tms) = init_ctm::<T>(T::MaxTransferManagersPerAsset::get());
        let last_tm = tms.pop().expect("MaxTransferManagersPerAsset should be greater than zero");
    }: _(owner.origin, ticker, last_tm)
    verify {
        assert!(Module::<T>::transfer_managers(ticker) == tms);
    }

    add_exempted_entities {
        // Length of the vector of Exempted identities being added.
        let i in 0 .. limits::MAX_EXEMPTED_IDENTITIES;

        let (owner, ticker) = init_ticker::<T>();
        let scope_ids = (0..i as u128).map(IdentityId::from).collect::<Vec<_>>();
        let tm = TransferManager::CountTransferManager(420);
        let ephemeral_tm = tm.clone();
    }: _(owner.origin, ticker, ephemeral_tm, scope_ids)
    verify {
        assert!(Module::<T>::entity_exempt((ticker, tm), IdentityId::from(0u128)) == (i != 0));
    }

    remove_exempted_entities {
        // Length of the vector of Exempted identities being removed.
        let i in 0 .. limits::MAX_EXEMPTED_IDENTITIES;

        let (owner, ticker) = init_ticker::<T>();
        let tm = TransferManager::CountTransferManager(420);
        let scope_ids = (0..i).map(|x| {
            let scope_id = IdentityId::from(x as u128);
            ExemptEntities::insert((ticker, tm.clone()), scope_id.clone(), true);
            scope_id
        }).collect::<Vec<_>>();
        let ephemeral_tm = tm.clone();
    }: _(owner.origin, ticker, ephemeral_tm, scope_ids)
    verify {
        assert!(!Module::<T>::entity_exempt((ticker, tm), IdentityId::from(0u128)));
    }

    #[extra]
    verify_tm_restrictions {
        let t in 0 .. T::MaxTransferManagersPerAsset::get();

        let (owner, ticker) = init_ticker::<T>();
        let owner_did = owner.did.unwrap();
        let tms = (0..t).map(|x| {
            let tm = TransferManager::CountTransferManager(x.into());
            ExemptEntities::insert((ticker, tm.clone()), owner_did, true);
            tm
        }).collect::<Vec<_>>();
        ActiveTransferManagers::insert(ticker, tms.clone());
        InvestorCountPerAsset::insert(ticker, 1337);
    }: {
        // This will trigger the worse case (exemption)
        Module::<T>::verify_tm_restrictions(
            &ticker,
            owner_did,
            owner_did,
            100u32.into(),
            200u32.into(),
            0u32.into(),
            500u32.into(),
        )?;
    }
}
