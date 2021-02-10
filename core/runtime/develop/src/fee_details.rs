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

use pallet_bridge as bridge;
use pallet_identity as identity;
use pallet_multisig as multisig;

use polymesh_common_utilities::{traits::transaction_payment::CddAndFeeDetails, Context};
use polymesh_primitives::{AccountId, AuthorizationData, IdentityId, Signatory, TransactionError};
use sp_runtime::transaction_validity::InvalidTransaction;

use codec::{Decode, Encode};
use frame_support::{StorageDoubleMap, StorageMap};

type Identity = identity::Module<Runtime>;
type Bridge = bridge::Module<Runtime>;

type Call = runtime::Call;

#[derive(Encode, Decode)]
enum CallType {
    AcceptMultiSigSigner,
    AcceptIdentitySecondary,
    AcceptIdentityPrimary,
    /// Matches any call to `remove_authorization`,
    /// where the authorization is available for `auth.authorized_by` payer redirection.
    RemoveAuthorization,
}

type ValidPayerResult = Result<Option<AccountId>, InvalidTransaction>;

fn cdd_required() -> ValidPayerResult {
    Err(InvalidTransaction::Custom(
        TransactionError::CddRequired as u8,
    ))
}

#[derive(Default, Encode, Decode, Clone, Eq, PartialEq)]
pub struct CddHandler;

impl CddAndFeeDetails<AccountId, Call> for CddHandler {
    /// Check if there's an eligible payer with valid CDD.
    /// Return the payer if found or else an error.
    /// Can also return Ok(none) to represent the case where
    /// CDD is valid but no payer should pay fee for this tx
    /// This also sets the identity in the context to the identity that was checked for CDD
    /// However, this does not set the payer context since that is meant to remain constant
    /// throughout the transaction. This function can also be used to simply check CDD and update identity context.
    fn get_valid_payer(call: &Call, caller: &AccountId) -> ValidPayerResult {
        let missing_id = || {
            Err(InvalidTransaction::Custom(
                TransactionError::MissingIdentity as u8,
            ))
        };
        let handle_multisig = |multisig, caller: &AccountId| {
            let sig = Signatory::Account(caller.clone());
            if <multisig::MultiSigSigners<Runtime>>::contains_key(multisig, sig) {
                let did = <multisig::MultiSigToIdentity<Runtime>>::get(multisig);
                check_cdd(&did)
            } else {
                missing_id()
            }
        };

        // The CDD check and fee payer varies depending on the transaction.
        // This match covers all possible scenarios.
        match call {
            // Register did call. This should be removed before mainnet launch and
            // all did registration should go through CDD
            Call::Identity(identity::Call::register_did(..)) => Ok(Some(caller.clone())),
            // Call made by a new Account key to accept invitation to become a secondary key
            // of an existing multisig that has a valid CDD. The auth should be valid.
            Call::MultiSig(multisig::Call::accept_multisig_signer_as_key(auth_id)) => {
                is_auth_valid(caller, auth_id, CallType::AcceptMultiSigSigner)
            }
            // Call made by a new Account key to accept invitation to become a secondary key
            // of an existing identity that has a valid CDD. The auth should be valid.
            Call::Identity(identity::Call::join_identity_as_key(auth_id, ..)) => {
                is_auth_valid(caller, auth_id, CallType::AcceptIdentitySecondary)
            }
            // Call made by a new Account key to accept invitation to become the primary key
            // of an existing identity that has a valid CDD. The auth should be valid.
            Call::Identity(identity::Call::accept_primary_key(rotation_auth_id, ..)) => {
                is_auth_valid(caller, rotation_auth_id, CallType::AcceptIdentityPrimary)
            }
            // Call made by a new Account key to remove invitation for certain authorizations
            // in an existing identity that has a valid CDD. The auth should be valid.
            Call::Identity(identity::Call::remove_authorization(_, auth_id, true)) => {
                is_auth_valid(caller, auth_id, CallType::RemoveAuthorization)
            }
            // Call made by an Account key to propose or approve a multisig transaction.
            // The multisig must have valid CDD and the caller must be a signer of the multisig.
            Call::MultiSig(
                multisig::Call::create_or_approve_proposal_as_key(multisig, ..)
                | multisig::Call::create_proposal_as_key(multisig, ..)
                | multisig::Call::approve_as_key(multisig, ..),
            ) => handle_multisig(multisig, caller),
            // Call made by an Account key to propose or approve a multisig transaction via the bridge helper
            // The multisig must have valid CDD and the caller must be a signer of the multisig.
            Call::Bridge(
                bridge::Call::propose_bridge_tx(..) | bridge::Call::batch_propose_bridge_tx(..),
            ) => handle_multisig(&Bridge::controller_key(), caller),
            // All other calls.
            //
            // The external account must directly be linked to an identity with valid CDD.
            _ => match Identity::get_identity(caller) {
                Some(did) if Identity::has_valid_cdd(did) => {
                    Context::set_current_identity::<Identity>(Some(did));
                    Ok(Some(caller.clone()))
                }
                Some(_) => cdd_required(),
                // Return if there's no DID.
                None => missing_id(),
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
            | (_, CallType::RemoveAuthorization),
        )) => check_cdd(&by),
        // None of the above apply, so error.
        _ => Err(InvalidTransaction::Custom(
            TransactionError::InvalidAuthorization as u8,
        )),
    }
}

/// Returns signatory to charge fee if cdd is valid.
fn check_cdd(did: &IdentityId) -> ValidPayerResult {
    if Identity::has_valid_cdd(*did) {
        Context::set_current_identity::<Identity>(Some(*did));
        let primary_key = Identity::did_records(&did).primary_key;
        Ok(Some(primary_key))
    } else {
        cdd_required()
    }
}
