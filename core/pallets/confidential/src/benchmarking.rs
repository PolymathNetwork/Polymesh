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
use polymesh_common_utilities::benchs::{User, UserBuilder};
use polymesh_primitives::asset::AssetType;
use sp_std::convert::TryFrom;

const MAX_TICKER_LENGTH: u8 = 12;
const SECRET_VALUE: u64 = 42;

fn make_ticker<T: Trait>(owner: &User<T>) -> Ticker {
    let ticker = Ticker::try_from(vec![b'A'; MAX_TICKER_LENGTH as usize].as_slice()).unwrap();
    let sc_name = b"TIC".into();
    T::Asset::create_asset(
        owner.origin().into(),
        sc_name,
        ticker.clone(),
        1_000u32.into(),
        true,
        AssetType::default(),
        vec![],
        None,
    )
    .expect("Asset cannot be created");

    ticker
}

benchmarks! {
    _ {}

    add_range_proof {
        let owner = UserBuilder::<T>::default().generate_did().build("owner");
        let prover = UserBuilder::<T>::default().generate_did().build("prover");
        let ticker = make_ticker::<T>(&owner);

    }: _(prover.origin(), owner.did(), ticker.clone(), SECRET_VALUE)
    verify {
        let prover_ticker_key = ProverTickerKey { prover: prover.did(), ticker };
        assert_eq!(RangeProofs::contains_key(owner.did(), prover_ticker_key), true);
    }

    add_verify_range_proof {
        let owner = UserBuilder::<T>::default().generate_did().build("owner");
        let prover = UserBuilder::<T>::default().generate_did().build("prover");
        let verifier = UserBuilder::<T>::default().generate_did().build("verifier");
        let ticker = make_ticker::<T>(&owner);

        Module::<T>::add_range_proof(prover.origin().into(), owner.did(), ticker.clone(), SECRET_VALUE)
            .expect( "Range proof cannot be added");


    }: _(verifier.origin(), owner.did(), prover.did(), ticker.clone())
    verify {
        let k1 = (owner.did(), ticker);
        assert_eq!(RangeProofVerifications::contains_key(k1, verifier.did()), true);
    }
}
