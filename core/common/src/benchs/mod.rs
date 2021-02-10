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

mod user;
use crate::traits::identity::Trait;
pub use user::{PublicKey, SecretKey, User};

mod user_builder;
pub use user_builder::{uid_from_name_and_idx, UserBuilder};

mod asset;
pub use asset::{generate_ticker, make_asset, make_indivisible_asset, make_ticker, ResultTicker};

pub fn user<T: Trait>(prefix: &'static str, u: u32) -> User<T> {
    UserBuilder::<T>::default()
        .generate_did()
        .seed(u)
        .build(prefix)
}
