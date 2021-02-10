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

use crate::identity::Trait as IdentityTrait;

use polymesh_primitives::IdentityId;

use codec::{Decode, Encode};
use frame_support::{
    decl_event,
    dispatch::DispatchResult,
    traits::{ChangeMembers, EnsureOrigin, InitializeMembers},
    weights::Weight,
};
use sp_std::{
    cmp::{Eq, Ordering, PartialEq},
    vec::Vec,
};

/// The number of group members.
pub type MemberCount = u32;

#[derive(Encode, Decode, Default, Clone, PartialEq, Eq, Debug)]
pub struct InactiveMember<Moment> {
    pub id: IdentityId,
    pub deactivated_at: Moment,
    pub expiry: Option<Moment>,
}

impl<M> PartialOrd for InactiveMember<M>
where
    M: Eq,
{
    fn partial_cmp(&self, other: &InactiveMember<M>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<M> PartialOrd<IdentityId> for InactiveMember<M>
where
    M: Eq,
{
    fn partial_cmp(&self, other: &IdentityId) -> Option<Ordering> {
        Some(self.id.cmp(other))
    }
}

impl<M> Ord for InactiveMember<M>
where
    M: Eq,
{
    fn cmp(&self, other: &InactiveMember<M>) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl<M> PartialEq<IdentityId> for InactiveMember<M> {
    fn eq(&self, other: &IdentityId) -> bool {
        self.id.eq(other)
    }
}

impl<M: Default> From<IdentityId> for InactiveMember<M> {
    fn from(id: IdentityId) -> Self {
        InactiveMember {
            id,
            ..Default::default()
        }
    }
}

pub trait WeightInfo {
    fn set_active_members_limit() -> Weight;
    fn add_member() -> Weight;
    fn remove_member() -> Weight;
    fn disable_member() -> Weight;
    fn swap_member() -> Weight;
    fn reset_members(new_members_len: u32) -> Weight;
    fn abdicate_membership() -> Weight;
}

pub trait Trait<I>: frame_system::Trait + pallet_timestamp::Trait + IdentityTrait {
    /// The overarching event type.
    type Event: From<Event<Self, I>> + Into<<Self as frame_system::Trait>::Event>;

    /// Required origin for changing the active limit.
    /// It's recommended that e.g., in case of a committee,
    /// this be an origin that cannot be formed through a committee majority.
    type LimitOrigin: EnsureOrigin<<Self as frame_system::Trait>::Origin>;

    /// Required origin for adding a member (though can always be Root).
    type AddOrigin: EnsureOrigin<<Self as frame_system::Trait>::Origin>;

    /// Required origin for removing a member (though can always be Root).
    type RemoveOrigin: EnsureOrigin<<Self as frame_system::Trait>::Origin>;

    /// Required origin for adding and removing a member in a single action.
    type SwapOrigin: EnsureOrigin<<Self as frame_system::Trait>::Origin>;

    /// Required origin for resetting membership.
    type ResetOrigin: EnsureOrigin<<Self as frame_system::Trait>::Origin>;

    /// The receiver of the signal for when the membership has been initialized. This happens pre-
    /// genesis and will usually be the same as `MembershipChanged`. If you need to do something
    /// different on initialization, then you can change this accordingly.
    type MembershipInitialized: InitializeMembers<IdentityId>;

    /// The receiver of the signal for when the membership has changed.
    type MembershipChanged: ChangeMembers<IdentityId>;

    /// Weight information for extrinsics in this pallet.
    type WeightInfo: WeightInfo;
}

decl_event!(
    pub enum Event<T, I> where
    <T as frame_system::Trait>::AccountId,
    <T as Trait<I>>::Event,
    {
        /// The given member was added; see the transaction for who.
        /// caller DID, New member DID.
        MemberAdded(IdentityId, IdentityId),
        /// The given member was removed; see the transaction for who.
        /// caller DID, member DID that get removed.
        MemberRemoved(IdentityId, IdentityId),
        /// The given member has been revoked at specific time-stamp.
        /// caller DID, member DID that get revoked.
        MemberRevoked(IdentityId, IdentityId),
        /// Two members were swapped; see the transaction for who.
        /// caller DID, Removed DID, New add DID.
        MembersSwapped(IdentityId, IdentityId, IdentityId),
        /// The membership was reset; see the transaction for who the new set is.
        /// caller DID, List of new members.
        MembersReset(IdentityId, Vec<IdentityId>),
        /// The limit of how many active members there can be concurrently was changed.
        ActiveLimitChanged(IdentityId, MemberCount, MemberCount),
        /// Phantom member, never used.
        Dummy(sp_std::marker::PhantomData<(AccountId, Event)>),
    }
);

pub trait GroupTrait<Moment: PartialOrd + Copy> {
    /// Retrieve members
    fn get_members() -> Vec<IdentityId>;

    /// Retrieve valid members: active and revoked members.
    fn get_inactive_members() -> Vec<InactiveMember<Moment>>;

    /// It moves `who` from active to inactive group.
    /// Any generated claim from `at` is considered as invalid. If `at` is `None` it will use `now`
    /// by default.
    /// If `expiry` is some value, that member will be removed automatically from this group at the
    /// specific moment, and any generated claim will be invalidated.
    fn disable_member(
        who: IdentityId,
        expiry: Option<Moment>,
        at: Option<Moment>,
    ) -> DispatchResult;

    /// Adds a member `who` to the group.
    fn add_member(who: IdentityId) -> DispatchResult;

    /// It returns the current "active members" and any "inactive member" which its
    /// expiration time-stamp is greater than `moment`.
    fn get_valid_members_at(moment: Moment) -> Vec<IdentityId> {
        Self::get_active_members()
            .into_iter()
            .chain(
                Self::get_inactive_members()
                    .into_iter()
                    .filter(|m| !Self::is_member_expired(&m, moment))
                    .map(|m| m.id),
            )
            .collect::<Vec<_>>()
    }

    fn is_member_expired(member: &InactiveMember<Moment>, now: Moment) -> bool {
        if let Some(expiry) = member.expiry {
            expiry <= now
        } else {
            false
        }
    }

    #[inline]
    fn get_active_members() -> Vec<IdentityId> {
        Self::get_members()
    }

    /// Current set size
    #[inline]
    fn member_count() -> usize {
        Self::get_members().len()
    }

    #[inline]
    fn is_member(member_id: &IdentityId) -> bool {
        Self::get_members().contains(member_id)
    }
}
