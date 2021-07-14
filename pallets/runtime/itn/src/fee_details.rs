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

use crate::{runtime, Runtime};
use codec::{Decode, Encode};
use frame_support::{StorageDoubleMap, StorageMap};
use polymesh_common_utilities::{traits::transaction_payment::CddAndFeeDetails, Context};
use polymesh_primitives::{AccountId, AuthorizationData, Hash, IdentityId, Signatory};
use polymesh_runtime_common::fee_details::{
    CallType, ValidPayerResult, CDD_REQUIRED, INVALID_AUTH, MISSING_ID,
};
use sp_core::crypto::UncheckedFrom;

type Identity = pallet_identity::Module<Runtime>;
type Bridge = pallet_bridge::Module<Runtime>;
type Call = runtime::Call;

#[derive(Default, Encode, Decode, Clone, Eq, PartialEq)]
pub struct CddHandler;

impl CddAndFeeDetails<AccountId, Call> for CddHandler
where
    AccountId: UncheckedFrom<Hash> + AsRef<[u8]>,
{
    /// Check if there's an eligible payer with valid CDD.
    /// Return the payer if found or else an error.
    /// Can also return Ok(none) to represent the case where
    /// CDD is valid but no payer should pay fee for this tx
    /// This also sets the identity in the context to the identity that was checked for CDD
    /// However, this does not set the payer context since that is meant to remain constant
    /// throughout the transaction. This function can also be used to simply check CDD and update identity context.
    fn get_valid_payer(call: &Call, caller: &AccountId) -> ValidPayerResult {
        let handle_multisig = |multisig, caller: &AccountId| {
            let sig = Signatory::Account(caller.clone());
            if <pallet_multisig::MultiSigSigners<Runtime>>::contains_key(multisig, sig) {
                check_cdd(&<pallet_multisig::MultiSigToIdentity<Runtime>>::get(
                    multisig,
                ))
            } else {
                MISSING_ID
            }
        };

        // The CDD check and fee payer varies depending on the transaction.
        // This match covers all possible scenarios.
        match call {
            // Call made by a new Account key to accept invitation to become a secondary key
            // of an existing multisig that has a valid CDD. The auth should be valid.
            Call::MultiSig(pallet_multisig::Call::accept_multisig_signer_as_key(auth_id)) => {
                is_auth_valid(caller, auth_id, CallType::AcceptMultiSigSigner)
            }
            // Call made by a new Account key to accept invitation to become a secondary key
            // of an existing identity that has a valid CDD. The auth should be valid.
            Call::Identity(pallet_identity::Call::join_identity_as_key(auth_id, ..)) => {
                is_auth_valid(caller, auth_id, CallType::AcceptIdentitySecondary)
            }
            // Call made by a new Account key to accept invitation to become the primary key
            // of an existing identity that has a valid CDD. The auth should be valid.
            Call::Identity(pallet_identity::Call::accept_primary_key(rotation_auth_id, ..)) => {
                is_auth_valid(caller, rotation_auth_id, CallType::AcceptIdentityPrimary)
            }
            // Call made by a new Account key to remove invitation for certain authorizations
            // in an existing identity that has a valid CDD. The auth should be valid.
            Call::Identity(pallet_identity::Call::remove_authorization(_, auth_id, true)) => {
                is_auth_valid(caller, auth_id, CallType::RemoveAuthorization)
            }
            // Call made by a user key to accept subsidy from a paying key. The auth should be valid.
            Call::Relayer(pallet_relayer::Call::accept_paying_key(auth_id)) => {
                is_auth_valid(caller, auth_id, CallType::AcceptRelayerPayingKey)
            }
            // Call made by an Account key to propose or approve a multisig transaction.
            // The multisig must have valid CDD and the caller must be a signer of the multisig.
            Call::MultiSig(
                pallet_multisig::Call::create_or_approve_proposal_as_key(multisig, ..)
                | pallet_multisig::Call::create_proposal_as_key(multisig, ..)
                | pallet_multisig::Call::approve_as_key(multisig, ..),
            ) => handle_multisig(multisig, caller),
            // Call made by an Account key to propose or approve a multisig transaction via the bridge helper
            // The multisig must have valid CDD and the caller must be a signer of the multisig.
            Call::Bridge(
                pallet_bridge::Call::propose_bridge_tx(..)
                | pallet_bridge::Call::batch_propose_bridge_tx(..),
            ) => handle_multisig(&Bridge::controller_key(), caller),
            // All other calls.
            //
            // The external account must directly be linked to an identity with valid CDD.
            _ => match Identity::get_identity(caller) {
                Some(did) if Identity::has_valid_cdd(did) => {
                    Context::set_current_identity::<Identity>(Some(did));
                    Ok(Some(caller.clone()))
                }
                Some(_) => CDD_REQUIRED,
                // Return if there's no DID.
                None => MISSING_ID,
            },
        }
    }

    /// Clears context. Should be called in post_dispatch
    fn clear_context() {
        Context::set_current_identity::<Identity>(None);
        Context::set_current_payer::<Identity>(None);
    }

    /// Sets payer in context. Should be called by the signed extension that first charges fee.
    fn set_payer_context(payer: Option<AccountId>) {
        Context::set_current_payer::<Identity>(payer);
    }

    /// Fetches fee payer for further payments (forwarded calls)
    fn get_payer_from_context() -> Option<AccountId> {
        Context::current_payer::<Identity>()
    }

    fn set_current_identity(did: &IdentityId) {
        Context::set_current_identity::<Identity>(Some(*did));
    }
}

/// Returns signatory to charge fee if auth is valid.
fn is_auth_valid(acc: &AccountId, auth_id: &u64, call_type: CallType) -> ValidPayerResult {
    // Fetch the auth if it exists and has not expired.
    match Identity::get_non_expired_auth(&Signatory::Account(acc.clone()), auth_id)
        .map(|auth| (auth.authorized_by, (auth.authorization_data, call_type)))
    {
        // Different auths have different authorization data requirements.
        // Hence we match call type to ensure proper authorization data is present.
        // We only need to check that there's a payer with a valid CDD.
        // Business logic for authorisations can be checked post-Signed Extension.
        Some((
            by,
            (AuthorizationData::AddMultiSigSigner(_), CallType::AcceptMultiSigSigner)
            | (AuthorizationData::JoinIdentity(_), CallType::AcceptIdentitySecondary)
            | (AuthorizationData::RotatePrimaryKey(_), CallType::AcceptIdentityPrimary)
            | (AuthorizationData::AddRelayerPayingKey(..), CallType::AcceptRelayerPayingKey)
            | (_, CallType::RemoveAuthorization),
        )) => check_cdd(&by),
        // None of the above apply, so error.
        _ => INVALID_AUTH,
    }
}

/// Returns signatory to charge fee if cdd is valid.
fn check_cdd(did: &IdentityId) -> ValidPayerResult {
    if Identity::has_valid_cdd(*did) {
        Context::set_current_identity::<Identity>(Some(*did));
        Ok(Some(Identity::did_records(&did).primary_key))
    } else {
        CDD_REQUIRED
    }
}
