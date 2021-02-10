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

use pallet_identity as identity;
use pallet_multisig as multisig;
use polymesh_common_utilities::traits::{balances::CheckCdd, identity::Trait as IdentityTrait};
use polymesh_primitives::IdentityId;

pub struct CddChecker<R>(sp_std::marker::PhantomData<R>);

impl<R> CheckCdd<<R as frame_system::Trait>::AccountId> for CddChecker<R>
where
    R: IdentityTrait + multisig::Trait,
{
    fn check_key_cdd(key: &<R as frame_system::Trait>::AccountId) -> bool {
        Self::get_key_cdd_did(key).is_some()
    }

    fn get_key_cdd_did(key: &<R as frame_system::Trait>::AccountId) -> Option<IdentityId> {
        identity::Module::<R>::get_identity(key)
            .filter(|&did| identity::Module::<R>::has_valid_cdd(did))
    }
}
