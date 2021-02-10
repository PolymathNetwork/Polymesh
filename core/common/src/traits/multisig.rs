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

//! # Trait Interface to the Multisig Module
//!
//! The interface allows to process addition of a multisig signer from modules other than the
//! multisig module itself.

use frame_support::dispatch::DispatchResult;
use polymesh_primitives::Signatory;

use sp_std::vec::Vec;

/// This trait is used to add a signer to a multisig and enable unlinking multisig from an identity
pub trait MultiSigSubTrait<AccountId> {
    /// Accepts and adds a multisig signer.
    ///
    /// # Arguments
    /// * `signer` - DID/key of the signer
    /// * `auth_id` - Authorization ID of the authorization created by the multisig.
    fn accept_multisig_signer(signer: Signatory<AccountId>, auth_id: u64) -> DispatchResult;

    /// Fetches signers of a multisig
    ///
    /// # Arguments
    /// * `multisig` - multisig AccountId
    fn get_key_signers(multisig: &AccountId) -> Vec<AccountId>;

    /// Checks if the account is a multisig
    ///
    /// # Arguments
    /// * `account` - AccountId to check
    fn is_multisig(account: &AccountId) -> bool;

    /// Checks if the account is a multisig signer
    ///
    /// # Arguments
    /// * `account` - AccountId to check
    fn is_signer(key: &AccountId) -> bool;
}
