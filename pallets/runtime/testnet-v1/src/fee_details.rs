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
use pallet_balances as balances;
use pallet_identity as identity;
use pallet_multisig as multisig;
use pallet_transaction_payment::CddAndFeeDetails;
use polymesh_common_utilities::Context;
use polymesh_primitives::{
    traits::IdentityCurrency, AccountId, AuthorizationData, IdentityId, Signatory, TransactionError,
};
use polymesh_runtime_common::bridge;
use sp_runtime::transaction_validity::InvalidTransaction;

type Identity = identity::Module<Runtime>;
type Balances = balances::Module<Runtime>;
type Bridge = bridge::Module<Runtime>;

type Call = runtime::Call;

enum CallType {
    AcceptMultiSigSigner,
    AcceptIdentitySigner,
    AcceptIdentityMaster,
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
    fn get_valid_payer(
        call: &Call,
        caller: &Signatory<AccountId>,
    ) -> Result<Option<Signatory<AccountId>>, InvalidTransaction> {
        // The CDD check and fee payer varies depending on the transaction.
        // This match covers all possible scenarios.
        match call {
            // Register did call. This should be removed before mainnet launch and
            // all did registration should go through CDD
            Call::Identity(identity::Call::register_did(..)) => {
                sp_runtime::print("register_did, CDD check bypassed");
                Ok(Some(caller.clone()))
            }
            // Call made by a new Account key to accept invitation to become a signing key
            // of an existing multisig that has a valid CDD. The auth should be valid.
            Call::MultiSig(multisig::Call::accept_multisig_signer_as_key(auth_id)) => {
                sp_runtime::print("accept_multisig_signer_as_key");
                is_auth_valid(caller, auth_id, CallType::AcceptMultiSigSigner)
            }
            // Call made by a new Account key to accept invitation to become a signing key
            // of an existing identity that has a valid CDD. The auth should be valid.
            Call::Identity(identity::Call::join_identity_as_key(auth_id, ..)) => {
                sp_runtime::print("join_identity_as_key");
                is_auth_valid(caller, auth_id, CallType::AcceptIdentitySigner)
            }
            // Call made by a new Account key to accept invitation to become the master key
            // of an existing identity that has a valid CDD. The auth should be valid.
            Call::Identity(identity::Call::accept_master_key(rotation_auth_id, ..)) => {
                sp_runtime::print("accept_master_key");
                is_auth_valid(caller, rotation_auth_id, CallType::AcceptIdentityMaster)
            }
            // Call made by an Account key to propose or approve a multisig transaction.
            // The multisig must have valid CDD and the caller must be a signer of the multisig.
            Call::MultiSig(multisig::Call::create_or_approve_proposal_as_key(multisig, ..))
            | Call::MultiSig(multisig::Call::create_proposal_as_key(multisig, ..))
            | Call::MultiSig(multisig::Call::approve_as_key(multisig, ..)) => {
                sp_runtime::print("multisig stuff");
                if <multisig::MultiSigSigners<Runtime>>::contains_key(multisig, caller) {
                    if let Some(did) = Identity::get_identity(&multisig) {
                        return check_cdd(&did);
                    }
                }
                Err(InvalidTransaction::Custom(TransactionError::MissingIdentity as u8).into())
            }
            // Call made by an Account key to propose or approve a multisig transaction via the bridge helper
            // The multisig must have valid CDD and the caller must be a signer of the multisig.
            Call::Bridge(bridge::Call::propose_bridge_tx(..))
            | Call::Bridge(bridge::Call::batch_propose_bridge_tx(..)) => {
                sp_runtime::print("multisig stuff via bridge");
                let multisig = Bridge::controller_key();
                if <multisig::MultiSigSigners<Runtime>>::contains_key(&multisig, caller) {
                    if let Some(did) = Identity::get_identity(&multisig) {
                        return check_cdd(&did);
                    }
                }
                Err(InvalidTransaction::Custom(TransactionError::MissingIdentity as u8).into())
            }
            // Call to set fee payer
            Call::Balances(balances::Call::change_charge_did_flag(charge_did)) => match caller {
                Signatory::Account(key) => {
                    if let Some(did) = Identity::get_identity(key) {
                        if Identity::has_valid_cdd(did) {
                            Context::set_current_identity::<Identity>(Some(did));
                            if *charge_did {
                                return Ok(Some(Signatory::from(did)));
                            } else {
                                return Ok(Some(caller.clone()));
                            }
                        }
                        return Err(InvalidTransaction::Custom(
                            TransactionError::CddRequired as u8,
                        )
                        .into());
                    }
                    // Return an error if any of the above checks fail
                    Err(InvalidTransaction::Custom(TransactionError::MissingIdentity as u8).into())
                }
                // A did was passed as the caller. The did should be charged the fee.
                // This will never happen during an external call.
                Signatory::Identity(did) => check_cdd(did),
            },
            // All other calls
            _ => match caller {
                // An external account was passed as the caller. This is the normal use case.
                // If the account has enabled charging fee to identity then the identity should be charged
                // otherwise, the account should be charged. In any case, the external account
                // must directly be linked to an identity with valid CDD.
                Signatory::Account(key) => {
                    if let Some(did) = Identity::get_identity(key) {
                        if Identity::has_valid_cdd(did) {
                            // TODO: Don't set current identity if key is ms signer, and ms signer is not attached to anything

                            Context::set_current_identity::<Identity>(Some(did));
                            if let Some(fee_did) = Balances::charge_fee_to_identity(key) {
                                sp_runtime::print("charging identity");
                                return Ok(Some(Signatory::from(fee_did)));
                            } else {
                                sp_runtime::print("charging key");
                                return Ok(Some(caller.clone()));
                            }
                        }
                        return Err(InvalidTransaction::Custom(
                            TransactionError::CddRequired as u8,
                        )
                        .into());
                    }
                    // Return an error if any of the above checks fail
                    Err(InvalidTransaction::Custom(TransactionError::MissingIdentity as u8).into())
                }
                // A did was passed as the caller. The did should be charged the fee.
                // This will never happen during an external call.
                Signatory::Identity(did) => check_cdd(did),
            },
        }
    }

    /// Clears context. Should be called in post_dispatch
    fn clear_context() {
        Context::set_current_identity::<Identity>(None);
        Context::set_current_payer::<Identity>(None);
    }

    /// Sets payer in context. Should be called by the signed extension that first charges fee.
    fn set_payer_context(payer: Option<Signatory<AccountId>>) {
        Context::set_current_payer::<Identity>(payer);
    }

    /// Fetches fee payer for further payements (forwareded calls)
    fn get_payer_from_context() -> Option<Signatory<AccountId>> {
        Context::current_payer::<Identity>()
    }

    fn set_current_identity(did: &IdentityId) {
        Context::set_current_identity::<Identity>(Some(*did));
    }
}

/// Returns signatory to charge fee if auth is valid.
fn is_auth_valid(
    singer: &Signatory<AccountId>,
    auth_id: &u64,
    call_type: CallType,
) -> Result<Option<Signatory<AccountId>>, InvalidTransaction> {
    // Fetches the auth if it exists and has not expired
    if let Some(auth) = Identity::get_non_expired_auth(singer, auth_id) {
        // Different auths have different authorization data requirements and hence we match call type
        // to make sure proper authorization data is present.
        match call_type {
            CallType::AcceptMultiSigSigner => {
                if let AuthorizationData::AddMultiSigSigner(multisig) = auth.authorization_data {
                    return check_cdd(&auth.authorized_by);
                }
            }
            CallType::AcceptIdentitySigner => {
                if let AuthorizationData::JoinIdentity(identity_data_to_join) =
                    auth.authorization_data
                {
                    return check_cdd(&auth.authorized_by);
                }
            }
            CallType::AcceptIdentityMaster => {
                if let AuthorizationData::RotateMasterKey(did) = auth.authorization_data {
                    return check_cdd(&auth.authorized_by);
                }
            }
        }
    }
    // Return an error if any of the above checks fail
    Err(InvalidTransaction::Custom(TransactionError::InvalidAuthorization as u8).into())
}

/// Returns signatory to charge fee if cdd is valid.
fn check_cdd(did: &IdentityId) -> Result<Option<Signatory<AccountId>>, InvalidTransaction> {
    if Identity::has_valid_cdd(*did) {
        Context::set_current_identity::<Identity>(Some(*did));
        return Ok(Some(Signatory::from(*did)));
    } else {
        sp_runtime::print("ERROR: This transaction requires an Identity");
        Err(InvalidTransaction::Custom(TransactionError::CddRequired as u8).into())
    }
}
