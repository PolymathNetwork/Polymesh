//! Runtime API definition for group module.
#![cfg_attr(not(feature = "std"), no_std)]

use polymesh_common_utilities::traits::group::InactiveMember;
use polymesh_primitives::{IdentityId, Moment};

use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::{prelude::*, vec::Vec};

#[derive(Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct Member {
    pub id: IdentityId,
    pub expiry_at: Option<Moment>,
    pub inactive_from: Option<Moment>,
}

impl From<IdentityId> for Member {
    fn from(id: IdentityId) -> Self {
        Member {
            id,
            expiry_at: None,
            inactive_from: None,
        }
    }
}

impl From<InactiveMember<Moment>> for Member {
    fn from(m: InactiveMember<Moment>) -> Self {
        Member {
            id: m.id,
            expiry_at: m.expiry,
            inactive_from: Some(m.deactivated_at),
        }
    }
}

sp_api::decl_runtime_apis! {
    /// The API to interact with Group governance.
    pub trait GroupApi
    {
        fn get_cdd_valid_members() -> Vec<Member>;
        fn get_gc_valid_members() -> Vec<Member>;
    }
}
