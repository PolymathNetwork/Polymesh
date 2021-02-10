use crate::*;
use frame_benchmarking::benchmarks;
use frame_support::traits::Get;
use pallet_settlement::{
    benchmarking::{add_transfer_managers, compliance_setup},
    VenueDetails,
};
use polymesh_common_utilities::benchs::{make_asset, User, UserBuilder};
use polymesh_primitives::TrustedIssuer;

const OFFERING_TICKER: Ticker = Ticker::repeating(b'A');
const RAISE_TICKER: Ticker = Ticker::repeating(b'B');

pub type Asset<T> = pallet_asset::Module<T>;
pub type ComplianceManager<T> = pallet_compliance_manager::Module<T>;
pub type Identity<T> = identity::Module<T>;
pub type Timestamp<T> = pallet_timestamp::Module<T>;
pub type Settlement<T> = pallet_settlement::Module<T>;
pub type Sto<T> = crate::Module<T>;

fn create_assets_and_compliance<T: Trait>(
    from: &User<T>,
    to: &User<T>,
    offering_ticker: Ticker,
    raise_ticker: Ticker,
    complexity: u32,
    transfer_managers: u32,
) -> DispatchResult {
    let t_issuer = UserBuilder::<T>::default()
        .generate_did()
        .build("TrustedClaimIssuer");
    let trusted_issuer = TrustedIssuer::from(t_issuer.did());
    let setup = |a: &User<T>,
                 b: &User<T>,
                 ticker: Ticker,
                 complexity: u32,
                 transfer_managers: u32|
     -> DispatchResult {
        make_asset::<T::AssetFn, T, T::Balance, T::AccountId, T::Origin, Ticker>(a, Some(ticker))?;
        compliance_setup::<T>(
            complexity,
            ticker,
            a.origin(),
            a.did(),
            b.did(),
            trusted_issuer.clone(),
        );
        add_transfer_managers::<T>(ticker, a.origin(), a.did(), transfer_managers);
        Ok(())
    };

    setup(from, to, offering_ticker, complexity, transfer_managers)?;
    setup(to, from, raise_ticker, complexity, transfer_managers)?;

    Ok(())
}

fn generate_tiers<T: Trait>(n: u32) -> Vec<PriceTier<T::Balance>> {
    let n = n as usize;
    let mut tiers = Vec::with_capacity(n);
    for i in 0..n {
        tiers.push(PriceTier {
            total: 1u32.into(),
            price: (i as u128 + 1).into(),
        })
    }
    tiers
}

fn create_venue<T: Trait>(user: &User<T>) -> Result<u64, DispatchError> {
    let venue_id = <Settlement<T>>::venue_counter();
    <Settlement<T>>::create_venue(
        user.origin().into(),
        VenueDetails::default(),
        vec![user.account()],
        VenueType::Sto,
    )?;
    Ok(venue_id)
}

struct UserWithPortfolio<T: Trait> {
    user: User<T>,
    portfolio: PortfolioId,
}

fn setup_fundraiser<T: Trait>(
    complexity: u32,
    tiers: u32,
    transfer_managers: u32,
) -> Result<(UserWithPortfolio<T>, UserWithPortfolio<T>), DispatchError> {
    let alice = user::<T>("alice");
    let bob = user::<T>("bob");

    create_assets_and_compliance::<T>(
        &alice.user,
        &bob.user,
        OFFERING_TICKER,
        RAISE_TICKER,
        complexity,
        transfer_managers,
    )?;

    let venue_id = create_venue(&alice.user)?;

    <Sto<T>>::create_fundraiser(
        alice.user.origin().into(),
        alice.portfolio,
        OFFERING_TICKER,
        alice.portfolio,
        RAISE_TICKER,
        generate_tiers::<T>(tiers),
        venue_id,
        None,
        Some(101u32.into()),
        0u32.into(),
        vec![].into(),
    )?;

    Ok((alice, bob))
}

fn user<T: Trait>(name: &'static str) -> UserWithPortfolio<T> {
    let user = <UserBuilder<T>>::default().generate_did().build(name);
    let portfolio = PortfolioId::default_portfolio(user.did());
    UserWithPortfolio { user, portfolio }
}

benchmarks! {
    _ {}

    create_fundraiser {
        // Number of tiers
        let i in 1 .. MAX_TIERS as u32;

        let alice = user::<T>("alice");

        create_assets_and_compliance::<T>(&alice.user, &alice.user, OFFERING_TICKER, RAISE_TICKER, 0, 0)?;

        let venue_id = create_venue(&alice.user)?;
        let tiers = generate_tiers::<T>(i);
    }: _(
            alice.user.origin(),
            alice.portfolio,
            OFFERING_TICKER,
            alice.portfolio,
            RAISE_TICKER,
            tiers,
            venue_id,
            None,
            None,
            0u32.into(),
            vec![].into()
        )
    verify {
        ensure!(FundraiserCount::get(OFFERING_TICKER) > 0, "create_fundraiser");
    }

    invest {
        let (alice, bob) = setup_fundraiser::<T>(T::MaxConditionComplexity::get() as u32, MAX_TIERS as u32, T::MaxTransferManagersPerAsset::get() as u32)?;
    }: _(
            bob.user.origin(),
            bob.portfolio,
            bob.portfolio,
            OFFERING_TICKER,
            0,
            (MAX_TIERS as u128).into(),
            Some(100u32.into()),
            None
        )
    verify {
        ensure!(<Asset<T>>::balance_of(&OFFERING_TICKER, bob.user.did()) > 0u32.into(), "invest");
    }

    freeze_fundraiser {
        let (alice, _) = setup_fundraiser::<T>(0, 1, 0)?;
    }: _(alice.user.origin(), OFFERING_TICKER, 0)
    verify {
        ensure!(<Fundraisers<T>>::get(OFFERING_TICKER, 0).unwrap().status == FundraiserStatus::Frozen, "freeze_fundraiser");
    }

    unfreeze_fundraiser {
        let (alice, _) = setup_fundraiser::<T>(0, 1, 0)?;
        <Sto<T>>::freeze_fundraiser(
            alice.user.origin().into(),
            OFFERING_TICKER,
            0,
        )?;
    }: _(alice.user.origin(), OFFERING_TICKER, 0)
    verify {
        ensure!(<Fundraisers<T>>::get(OFFERING_TICKER, 0).unwrap().status == FundraiserStatus::Live, "unfreeze_fundraiser");
    }

    modify_fundraiser_window {
        let (alice, _) = setup_fundraiser::<T>(0, 1, 0)?;
    }: _(alice.user.origin(), OFFERING_TICKER, 0, 100u32.into(), Some(101u32.into()))
    verify {
        ensure!(<Fundraisers<T>>::get(OFFERING_TICKER, 0).unwrap().end == Some(101u32.into()), "modify_fundraiser_window");
    }

    stop {
        let (alice, _) = setup_fundraiser::<T>(0, 1, 0)?;
    }: _(alice.user.origin(), OFFERING_TICKER, 0)
    verify {
        ensure!(<Fundraisers<T>>::get(OFFERING_TICKER, 0).unwrap().status == FundraiserStatus::Closed, "stop");
    }
}
