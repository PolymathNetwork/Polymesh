use crate::{
    benchs::User,
    constants::currency::POLY,
    traits::{asset::AssetFnTrait, identity::Trait as IdentityTrait},
};

use polymesh_primitives::{
    asset::{AssetName, AssetType},
    Ticker,
};

use frame_system::RawOrigin;
use sp_std::{convert::TryFrom, prelude::*};

pub type ResultTicker = Result<Ticker, &'static str>;

/// Create a ticker and register it.
pub fn make_ticker<Asset, Balance, Acc, O, N>(owner: O, opt_name: Option<N>) -> ResultTicker
where
    Asset: AssetFnTrait<Balance, Acc, O>,
    N: AsRef<[u8]>,
{
    let ticker = match &opt_name {
        Some(name) => Ticker::try_from(name.as_ref()).map_err(|_| "Invalid ticker name")?,
        _ => Ticker::repeating(b'A'),
    };
    Asset::register_ticker(owner, ticker).map_err(|_| "Ticker cannot be registered")?;

    Ok(ticker)
}

pub fn make_asset<Asset, Identity, Balance, Acc, Origin, N>(
    owner: &User<Identity>,
    name: Option<N>,
) -> ResultTicker
where
    Asset: AssetFnTrait<Balance, Acc, Origin>,
    Identity: IdentityTrait,
    Origin: From<RawOrigin<<Identity as frame_system::Trait>::AccountId>>,
    Balance: From<u128>,
    N: AsRef<[u8]>,
{
    make_base_asset::<Asset, Identity, Balance, Acc, Origin, N>(owner, true, name)
}

pub fn make_indivisible_asset<Asset, Identity, Balance, Acc, Origin, N>(
    owner: &User<Identity>,
    name: Option<N>,
) -> ResultTicker
where
    Asset: AssetFnTrait<Balance, Acc, Origin>,
    Identity: IdentityTrait,
    Origin: From<RawOrigin<<Identity as frame_system::Trait>::AccountId>>,
    Balance: From<u128>,
    N: AsRef<[u8]>,
{
    make_base_asset::<Asset, Identity, Balance, Acc, Origin, N>(owner, false, name)
}

fn make_base_asset<Asset, Identity, Balance, Acc, Origin, N>(
    owner: &User<Identity>,
    divisible: bool,
    name: Option<N>,
) -> ResultTicker
where
    Asset: AssetFnTrait<Balance, Acc, Origin>,
    Identity: IdentityTrait,
    Origin: From<RawOrigin<<Identity as frame_system::Trait>::AccountId>>,
    Balance: From<u128>,
    N: AsRef<[u8]>,
{
    let ticker = make_ticker::<Asset, _, _, _, _>(owner.origin().into(), name)?;
    let name: AssetName = ticker.as_slice().into();
    let total_supply: Balance = (1_000_000 * POLY).into();

    Asset::create_asset(
        owner.origin().into(),
        name,
        ticker,
        total_supply,
        divisible,
        AssetType::default(),
        vec![],
        None,
    )
    .map_err(|_| "Asset cannot be created")?;

    Ok(ticker)
}

/// Given a number, this function generates a ticker with
/// A-Z, least number of characters in Lexicographic order
pub fn generate_ticker(n: u64) -> Vec<u8> {
    fn calc_base26(n: u64, base_26: &mut Vec<u8>) {
        if n >= 26 {
            // Subtracting 1 is not required and shouldn't be done for a proper base_26 conversion
            // However, without this hack, B will be the first char after a bump in number of chars.
            // i.e. the sequence will go A,B...Z,BA,BB...ZZ,BAA. We want the sequence to start with A.
            // Subtracting 1 here means we are doing 1 indexing rather than 0.
            // i.e. A = 1, B = 2 instead of A = 0, B = 1
            calc_base26((n / 26) - 1, base_26);
        }
        let character = n % 26 + 65;
        base_26.push(character as u8);
    }
    let mut base_26 = Vec::new();
    calc_base26(n, &mut base_26);
    base_26
}
