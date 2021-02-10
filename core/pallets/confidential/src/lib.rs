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

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use cryptography_core::{
    asset_proofs::range_proof::{prove_within_range, verify_within_range, InRangeProof},
    bulletproofs::RangeProof,
    CompressedRistretto, Scalar,
};
use frame_support::{
    debug, decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult,
    weights::Weight,
};
use pallet_identity as identity;
use polymesh_common_utilities::{asset::AssetFnTrait, identity::Trait as IdentityTrait};
use polymesh_primitives::{IdentityId, Ticker};
use polymesh_primitives_derive::{SliceU8StrongTyped, VecU8StrongTyped};
use sp_std::prelude::*;

pub mod rng;
pub use rng::native_rng;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, SliceU8StrongTyped)]
pub struct RangeProofInitialMessageWrapper(pub [u8; 32]);

#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, VecU8StrongTyped)]
pub struct RangeProofFinalResponseWrapper(pub Vec<u8>);

#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq)]
pub struct TickerRangeProof {
    // pub proof: RangeProofWrapper,
    pub initial_message: RangeProofInitialMessageWrapper,
    // pub committed_value: CompressedRistrettoWrapper,
    pub final_response: RangeProofFinalResponseWrapper,
    pub max_two_exp: u32,
}

pub trait WeightInfo {
    fn add_range_proof() -> Weight;
    fn add_verify_range_proof() -> Weight;
}

pub trait Trait: frame_system::Trait + IdentityTrait {
    type Event: From<Event> + Into<<Self as frame_system::Trait>::Event>;

    type Asset: AssetFnTrait<Self::Balance, Self::AccountId, Self::Origin>;
    type WeightInfo: WeightInfo;
}

type Identity<T> = identity::Module<T>;

#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq)]
pub struct ProverTickerKey {
    prover: IdentityId,
    ticker: Ticker,
}

decl_storage! {
    trait Store for Module<T: Trait> as Confidential {
        /// Number of investor per asset.
        pub RangeProofs get(fn range_proof): double_map hasher(identity) IdentityId, hasher(blake2_128_concat) ProverTickerKey => Option<TickerRangeProof>;

        pub RangeProofVerifications get(fn range_proof_verification): double_map hasher(blake2_128_concat) (IdentityId, Ticker), hasher(identity) IdentityId => bool;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        #[weight = <T as Trait>::WeightInfo::add_range_proof()]
        pub fn add_range_proof(origin, target_id: IdentityId, ticker: Ticker, secret_value: u64) {
            let prover = Identity::<T>::ensure_perms(origin)?;

            // Create proof
            let mut rng = rng::Rng::default();
            let rand_blind = Scalar::random(&mut rng);
            let in_range_proof = prove_within_range( secret_value, rand_blind, 32, &mut rng)
                .map_err(|e| {
                    debug::error!("Confidential error: {:?}", e);
                    Error::<T>::InvalidRangeProof
                })?;

            let ticker_range_proof = TickerRangeProof {
                initial_message: RangeProofInitialMessageWrapper::from(in_range_proof.init.as_bytes().as_ref()),
                final_response: in_range_proof.response.to_bytes().into(),
                max_two_exp: 32,
            };
            let prover_ticker_key = ProverTickerKey { prover, ticker };
            RangeProofs::insert(&target_id, &prover_ticker_key, ticker_range_proof);
        }

        #[weight = <T as Trait>::WeightInfo::add_verify_range_proof()]
        pub fn add_verify_range_proof(origin, target: IdentityId, prover: IdentityId, ticker: Ticker) {
            let verifier_id = Identity::<T>::ensure_perms(origin)?;
            Self::verify_range_proof(target, prover, ticker)?;
            RangeProofVerifications::insert((target, ticker), verifier_id, true);
        }
    }
}

decl_event! {
    pub enum Event {
        RangeProofAdded(IdentityId, Ticker, TickerRangeProof),
        RangeProofVerified(IdentityId, IdentityId, Ticker),
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        ///
        MissingRangeProof,
        ///
        InvalidRangeProof,
    }
}

impl<T: Trait> Module<T> {
    pub fn verify_range_proof(
        target: IdentityId,
        prover: IdentityId,
        ticker: Ticker,
    ) -> DispatchResult {
        let prover_ticker_key = ProverTickerKey { prover, ticker };
        let mut rng = rng::Rng::default();

        let trp = Self::range_proof(&target, &prover_ticker_key)
            .ok_or_else(|| Error::<T>::MissingRangeProof)?;

        let init = CompressedRistretto::from_slice(trp.initial_message.as_slice());
        let response = RangeProof::from_bytes(trp.final_response.as_slice())
            .map_err(|_| Error::<T>::InvalidRangeProof)?;
        let proof = InRangeProof {
            init,
            response,
            range: 32,
        };

        verify_within_range(&proof, &mut rng).map_err(|_| Error::<T>::InvalidRangeProof.into())
    }
}
