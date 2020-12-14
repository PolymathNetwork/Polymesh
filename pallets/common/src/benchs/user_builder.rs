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
use crate::{
    benchs::{SecretKey, User},
    traits::{
        group::GroupTrait,
        identity::{IdentityFnTrait, Trait},
    },
};
use codec::Encode;
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use polymesh_primitives::{IdentityId, InvestorUid};
use sp_std::{convert::TryInto, prelude::*};

#[cfg(not(feature = "std"))]
use frame_benchmarking::account;

#[cfg(feature = "std")]
use codec::Decode;
#[cfg(feature = "std")]
use sp_core::{crypto::Pair as TPair, sr25519::Pair};
#[cfg(feature = "std")]
use sp_io::hashing::blake2_256;

pub fn uid_from_name_and_idx(name: &'static str, u: u32) -> InvestorUid {
    InvestorUid::from((name, u).encode().as_slice())
}

pub struct UserBuilder<T: Trait> {
    account: Option<T::AccountId>,
    uid: Option<InvestorUid>,
    did: Option<IdentityId>,
    balance: T::Balance,
    seed: u32,
    generate_did: bool,
    as_cdd_provider: bool,
}

macro_rules! self_update {
    ($self:ident, $member:ident, $value:expr) => {{
        let mut new = $self;
        new.$member = $value;
        new
    }};
}

impl<T: Trait> UserBuilder<T> {
    /// Create an account based on the builder configuration.
    pub fn build(self, name: &'static str) -> User<T> {
        let (account, secret) = self.account.clone().map_or_else(
            || Self::make_key_pair(name, self.seed),
            |acc| (acc, [0u8; 64]),
        );
        let origin = RawOrigin::Signed(account.clone());
        let amount: u32 = self.balance.try_into().unwrap_or_default() as u32;
        let _ = T::Balances::make_free_balance_be(&account, amount.into());

        // Generate DID or use the one given.
        let uid = self
            .uid
            .unwrap_or_else(|| uid_from_name_and_idx(name, self.seed));
        let did = self.did.or_else(|| {
            self.generate_did
                .then(|| Self::make_did(account.clone(), uid.clone()))
        });

        // Become a CDD provider.
        self.as_cdd_provider.then(|| {
            T::CddServiceProviders::add_member(did.clone().unwrap())
                .expect("User cannot be added as CDD provider")
        });

        User {
            account,
            secret,
            origin,
            did,
            uid: Some(uid),
        }
    }

    pub fn generate_did(self) -> Self {
        assert!(self.did.is_none());
        self_update!(self, generate_did, true)
    }

    pub fn become_cdd_provider(self) -> Self {
        assert!(self.generate_did == true || self.did.is_some());
        self_update!(self, as_cdd_provider, true)
    }

    pub fn uid(self, u: InvestorUid) -> Self {
        self_update!(self, uid, Some(u))
    }

    pub fn did(self, did: IdentityId) -> Self {
        assert!(self.generate_did == false);
        let mut new = self;
        new.did = Some(did);
        new.generate_did = false;
        new
    }

    pub fn seed(self, s: u32) -> Self {
        self_update!(self, seed, s)
    }

    pub fn account<ACC: Into<T::AccountId>>(self, acc: ACC) -> Self {
        self_update!(self, account, Some(acc.into()))
    }

    pub fn balance<B: Into<T::Balance>>(self, b: B) -> Self {
        self_update!(self, balance, b.into())
    }

    #[cfg(not(feature = "std"))]
    fn make_key_pair(name: &'static str, u: u32) -> (T::AccountId, SecretKey) {
        let public: T::AccountId = account(name, u, 0);
        let secret = [0u8; 64];

        (public, secret)
    }

    #[cfg(feature = "std")]
    fn make_key_pair(name: &'static str, u: u32) -> (T::AccountId, SecretKey) {
        let seed = (name, u).using_encoded(blake2_256);
        let pair = Pair::from_seed(&seed);
        let keypair = pair.as_ref();

        let secret = keypair.secret.to_bytes();
        let public = keypair.public.to_bytes();
        let id = T::AccountId::decode(&mut &public[..]).unwrap();

        (id, secret)
    }

    /// Create a DID for account `acc` using the specified investor ID.
    fn make_did(acc: T::AccountId, uid: InvestorUid) -> IdentityId {
        let _ = T::IdentityFn::register_did(acc.clone(), uid, vec![]);
        T::IdentityFn::get_identity(&acc).unwrap()
    }
}

// Derive macro from `Default` is not supported due to trait T.
impl<T: Trait> Default for UserBuilder<T> {
    fn default() -> Self {
        Self {
            account: None,
            uid: None,
            did: None,
            balance: 5_000_000u128.into(),
            seed: 0,
            generate_did: false,
            as_cdd_provider: false,
        }
    }
}