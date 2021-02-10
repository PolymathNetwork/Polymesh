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

//! Provides a for-events-only protector newtype for arbitrary objects.

use codec::{Decode, Encode};
#[cfg(feature = "std")]
use sp_runtime::{Deserialize, Serialize};

/// A protective newtype around any type,
/// signalling that the contained element is only for use by events.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, Default, Debug)]
pub struct EventOnly<T>(T);

impl<T> EventOnly<T> {
    /// Wrap the protectee in the protector.
    pub fn new(x: T) -> Self {
        EventOnly(x)
    }

    /// Consume and extract the protectee from the for-event-only protector.
    /// This is a risky move, make sure you are not using the protectee for anything important.
    pub fn risky_into_inner(self) -> T {
        self.0
    }
}
