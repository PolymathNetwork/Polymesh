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

use crate::{Claim, Condition, ConditionType, IdentityId, TargetIdentity};
use codec::{Decode, Encode};

use sp_std::prelude::*;

/// Context using during an `Proposition` evaluation.
#[derive(Encode, Decode, Clone, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Context<C> {
    /// Proposition evaluation will use those claims.
    pub claims: C,
    /// Identity of this context.
    /// It could be the sender DID during the evaluation of sender's conditions or
    /// the receiver DID on a receiver's condition evaluation.
    pub id: IdentityId,
    /// Identity of the primary_issuance_agent of the token
    pub primary_issuance_agent: IdentityId,
}

// Proposition Trait
// ==================================

/// It allows composition and evaluation of claims based on a context.
pub trait Proposition<C> {
    /// Evaluates this proposition based on `context`.
    fn evaluate(&self, context: Context<C>) -> bool;

    /// It generates a new proposition that represents the logical AND
    /// of two propositions: `Self` and `other`.
    #[inline]
    fn and<B: Proposition<C>>(self, other: B) -> AndProposition<Self, B>
    where
        Self: Sized,
    {
        AndProposition::new(self, other)
    }

    /// It generates a new proposition that represents the logical OR
    /// of two propositions: `Self` and `other`.
    #[inline]
    fn or<B: Proposition<C>>(self, other: B) -> OrProposition<Self, B>
    where
        Self: Sized,
    {
        OrProposition::new(self, other)
    }

    /// It generates a new proposition that represents the logical NOT
    /// of this proposition.
    #[inline]
    fn not(self) -> NotProposition<Self>
    where
        Self: Sized,
    {
        NotProposition::new(self)
    }
}

/// Base and simple propositions
pub mod base;
pub use base::{
    AndProposition, AnyProposition, ExistentialProposition, NotProposition, OrProposition,
    TargetIdentityProposition,
};

// Helper functions
// ======================================

/// It creates a proposition to evaluate the matching of `id` with primary_issuance_agent in the context.
#[inline]
pub fn equals<'a>(
    id: &'a TargetIdentity,
    primary_issuance_agent: &'a IdentityId,
) -> TargetIdentityProposition<'a> {
    match id {
        TargetIdentity::PrimaryIssuanceAgent => TargetIdentityProposition {
            identity: primary_issuance_agent,
        },
        TargetIdentity::Specific(identity) => TargetIdentityProposition { identity },
    }
}

/// It creates a proposition to evaluate the existential of `claim` in the context.
#[inline]
pub fn exists(claim: &Claim) -> ExistentialProposition<'_> {
    ExistentialProposition { claim }
}

/// It creates a proposition to evaluate if any of `claims` are found in the context.
#[inline]
pub fn any(claims: &[Claim]) -> AnyProposition<'_> {
    AnyProposition { claims }
}

/// It create a negate proposition of `proposition`.
#[inline]
pub fn not<P: Proposition<C>, C>(proposition: P) -> NotProposition<P> {
    NotProposition::new(proposition)
}

/// Helper function to run propositions from a context.
pub fn run<C: Iterator<Item = Claim>>(condition: &Condition, context: Context<C>) -> bool {
    match &condition.condition_type {
        ConditionType::IsPresent(claim) => exists(claim).evaluate(context),
        ConditionType::IsAbsent(claim) => not::<_, C>(exists(claim)).evaluate(context),
        ConditionType::IsAnyOf(claims) => any(claims).evaluate(context),
        ConditionType::IsNoneOf(claims) => not::<_, C>(any(claims)).evaluate(context),
        ConditionType::IsIdentity(id) => {
            equals(id, &context.primary_issuance_agent.clone()).evaluate(context)
        }
    }
}
