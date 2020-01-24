//! # Multisig Module
//!
//! The Multisig module provides functionality for n of m multisigs.
//!
//! ## Overview
//!
//! The multisig module provides functions for:
//!
//! - Creating a new multisig
//! - Proposing a multisig transaction
//! - Approving a multisig transaction
//! - Adding new signers to the multisig
//! - Removing existing signers from multisig
//!
//! ### Terminology
//!
//! - **Multisig:** It is a special type of account that can do tranaction only if at least n of its m signers approve.
//! - **Proposal:** It is a general transaction that the multisig can do,
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `create_multi_sig` - Creates a new multisig.
//! - `create_proposal` - Creates a proposal for a multisig transaction.
//! - `approve_as_identity` - Approves a proposal as an Identity.
//! - `approve_as_key` - Approves a proposal as a Signing key.
//! - `accept_multi_sig_signer_as_identity` - Accept being added as a signer of a multisig.
//! - `accept_multi_sig_signer_as_key` - Accept being added as a signer of a multisig.
//! - `add_multi_sig_signer` - Adds a signer to the multisig.
//! - `remove_multi_sig_signer` - Removes a signer from the multisig.
//! - `change_sigs_required` - Changes the number of signatures required to execute a transaction.

#![cfg_attr(not(feature = "std"), no_std)]

use crate::identity;
use codec::{Decode, Encode};
use primitives::{AuthorizationData, AuthorizationError, Key, Signer};
use rstd::{convert::TryFrom, prelude::*};
use sr_primitives::{
    traits::{Dispatchable, Hash},
    weights::{GetDispatchInfo, Weight},
    DispatchError,
};
use srml_support::{decl_event, decl_module, decl_storage, dispatch::Result, ensure, StorageValue};
use system::ensure_signed;

pub trait Trait: system::Trait + identity::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as Multisig {
        /// Nonce to ensure unique Multisig addresses are generated. starts from 1.
        pub MultiSigNonce get(ms_nonce) build(|_| 1u64): u64;

        /// Signers of a multisig. (mulisig, signer) => true/false
        pub MultiSigSigners get(ms_signers): map (T::AccountId, Signer) => bool;
        /// Confirmations required before processing a multisig tx
        pub MultiSigSignsRequired get(ms_signs_required): map T::AccountId => u64;
        /// Number of transactions proposed in a multisig. Used as tx id. starts from 0
        pub MultiSigTxDone get(ms_tx_done): map T::AccountId => u64;

        /// Proposals presented for voting to a multisig (multisig, proposal id) => Option<proposal>.
        /// It is deleted after proposal is processed
        pub Proposals get(proposals): map (T::AccountId, u64) => Option<T::Proposal>;

        /// Number of votes in favor of a tx. Mapping from (multisig, tx id) => no. of approvals.
        pub TxApprovals get(tx_approvals): map (T::AccountId, u64) => u64;
        /// Individual multisig signer votes. (multi sig, signer, )
        pub Votes get(votes): map (T::AccountId, Signer, u64) => bool;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        /// Creates a multisig
        ///
        /// # Arguments
        /// * `signers` - Signers of the multisig (They need to accept authorization before they are actually added).
        /// * `sigs_required` - Number of sigs required to process a multi-sig tx.
        pub fn create_multi_sig(origin, signers: Vec<Signer>, sigs_required: u64) -> Result {
            let sender = ensure_signed(origin)?;
            ensure!(signers.len() > 0, "No signers provided");
            ensure!(u64::try_from(signers.len()).unwrap_or_default() >= sigs_required && sigs_required > 0,
                "Sigs required out of bounds"
            );
            let nonce: u64 = Self::ms_nonce();
            let new_nonce: u64 = nonce + 1u64;
            <MultiSigNonce>::put(new_nonce);

            let h: T::Hash = T::Hashing::hash(&(b"MULTI_SIG", new_nonce, sender.clone()).encode());
            let wallet_id = T::AccountId::decode(&mut &h.encode()[..])
                .map_err( |_| "Error in decoding multisig address")?;

            for signer in signers.clone() {
                <identity::Module<T>>::add_auth(
                    Signer::from(Key::try_from(wallet_id.encode())?),
                    signer,
                    AuthorizationData::AddMultisigSigner,
                    None
                );
            }

            <MultiSigSignsRequired<T>>::insert(&wallet_id, &sigs_required);

            Self::deposit_event(RawEvent::MultiSigCreated(wallet_id, sender, signers, sigs_required));

            Ok(())
        }

        /// Creates a multisig proposal
        ///
        /// # Arguments
        /// * `multi_sig` - Multisig address.
        /// * `proposal` - Proposal to be voted on.
        /// If this is 1 of m multisig, the proposal will be immediately executed.
        pub fn create_proposal(origin, multi_sig: T::AccountId, proposal: Box<T::Proposal>) -> Result {
            let sender = ensure_signed(origin)?;
            let sender_signer = Signer::from(Key::try_from(sender.encode())?);
            ensure!(Self::ms_signers((multi_sig.clone(), sender_signer)), "not an signer");
            let proposal_id = Self::ms_tx_done(multi_sig.clone());
            <Proposals<T>>::insert((multi_sig.clone(), proposal_id), proposal);
            let next_proposal_id: u64 = proposal_id + 1u64;
            <MultiSigTxDone<T>>::insert(multi_sig.clone(), next_proposal_id);
            Self::deposit_event(RawEvent::ProposalAdded(multi_sig.clone(), proposal_id));
            Self::approve_for(multi_sig, proposal_id, sender_signer)
        }

        /// Approves a multisig proposal using caller's identity
        ///
        /// # Arguments
        /// * `multi_sig` - Multisig address.
        /// * `proposal_id` - Proposal id to approve.
        /// If enough voted are reached, the proposal will be immediately executed.
        pub fn approve_as_identity(origin, multi_sig: T::AccountId, proposal_id: u64) -> Result {
            let sender = ensure_signed(origin)?;
            let sender_key = Key::try_from(sender.encode())?;
            let signer_did =  match <identity::Module<T>>::current_did() {
                Some(x) => x,
                None => {
                    if let Some(did) = <identity::Module<T>>::get_identity(&sender_key) {
                        did
                    } else {
                        return Err("did not found");
                    }
                }
            };
            let signer = Signer::from(signer_did);
            ensure!(Self::ms_signers((multi_sig.clone(), signer)), "not an signer");
            Self::approve_for(multi_sig, proposal_id, signer)
        }

        /// Approves a multisig proposal using caller's signing key (AccountId)
        ///
        /// # Arguments
        /// * `multi_sig` - Multisig address.
        /// * `proposal_id` - Proposal id to approve.
        /// If enough voted are reached, the proposal will be immediately executed.
        pub fn approve_as_key(origin, multi_sig: T::AccountId, proposal_id: u64) -> Result {
            let sender = ensure_signed(origin)?;
            let signer = Signer::from(Key::try_from(sender.encode())?);
            ensure!(Self::ms_signers((multi_sig.clone(), signer)), "not an signer");
            Self::approve_for(multi_sig, proposal_id, signer)
        }

        /// Accept a multisig signer authorization given to signer's identity
        ///
        /// # Arguments
        /// * `multi_sig` - Multisig address.
        /// * `proposal_id` - Auth id of the authorization.
        pub fn accept_multi_sig_signer_as_identity(origin, auth_id: u64) -> Result {
            let sender = ensure_signed(origin)?;
            let sender_key = Key::try_from(sender.encode())?;
            let signer_did =  match <identity::Module<T>>::current_did() {
                Some(x) => x,
                None => {
                    if let Some(did) = <identity::Module<T>>::get_identity(&sender_key) {
                        did
                    } else {
                        return Err("did not found");
                    }
                }
            };
            let signer = Signer::from(signer_did);
            Self::_accept_multi_sig_signer(signer, auth_id)
        }

        /// Accept a multisig signer authorization given to signer's key (AccountId)
        ///
        /// # Arguments
        /// * `multi_sig` - Multisig address.
        /// * `proposal_id` - Auth id of the authorization.
        pub fn accept_multi_sig_signer_as_key(origin, auth_id: u64) -> Result {
            let sender = ensure_signed(origin)?;
            let signer = Signer::from(Key::try_from(sender.encode())?);
            Self::_accept_multi_sig_signer(signer, auth_id)
        }

        /// Add a signer to the multisig. This must be called by the multisig itself.
        ///
        /// # Arguments
        /// * `multi_sig` - Multisig address.
        /// * `signer` - Signer to add.
        pub fn add_multi_sig_signer(origin, signer: Signer) -> Result {
            let sender = ensure_signed(origin)?;
            let sender_signer = Signer::from(Key::try_from(sender.encode())?);
            ensure!(!<MultiSigSignsRequired<T>>::exists(&sender), "Multi sig does not exist");
            <identity::Module<T>>::add_auth(
                sender_signer,
                signer,
                AuthorizationData::AddMultisigSigner,
                None
            );
            Self::deposit_event(RawEvent::MultiSigSignerAuthorized(sender, signer));
            Ok(())
        }

        /// Remove a signer from the multisig. This must be called by the multisig itself.
        ///
        /// # Arguments
        /// * `multi_sig` - Multisig address.
        /// * `signer` - Signer to remove.
        pub fn remove_multi_sig_signer(origin, signer: Signer) -> Result {
            let sender = ensure_signed(origin)?;
            ensure!(!<MultiSigSignsRequired<T>>::exists(&sender), "Multi sig does not exist");
            <MultiSigSigners<T>>::insert((sender.clone(), signer), false);
            Self::deposit_event(RawEvent::MultiSigSignerRemoved(sender, signer));
            Ok(())
        }

        /// Change number of sigs required by a multisig. This must be called by the multisig itself.
        ///
        /// # Arguments
        /// * `multi_sig` - Multisig address.
        /// * `sigs_required` - New number os sigs required.
        pub fn change_sigs_required(origin, sigs_required: u64) -> Result {
            let sender = ensure_signed(origin)?;
            ensure!(!<MultiSigSignsRequired<T>>::exists(&sender), "Multi sig does not exist");
            <MultiSigSignsRequired<T>>::insert(&sender, &sigs_required);
            Self::deposit_event(RawEvent::MultiSigSignaturesRequiredChanged(sender, sigs_required));
            Ok(())
        }
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as system::Trait>::AccountId,
    {
        /// Event for multi sig creation. (Multisig address, Creator address, Signers(pending approval), Sigs required)
        MultiSigCreated(AccountId, AccountId, Vec<Signer>, u64),
        /// Event for adding a proposal (Multisig, proposalid)
        ProposalAdded(AccountId, u64),
        /// Emitted when a proposal is executed. (Multisig, proposalid, result)
        ProposalExecuted(AccountId, u64, bool),
        /// Signer added (Authorization accepted) (Multisig, signer_added)
        MultiSigSignerAdded(AccountId, Signer),
        /// Multi Sig Signer Authorized to be added (Multisig, signer_authorized)
        MultiSigSignerAuthorized(AccountId, Signer),
        /// Multi Sig Signer removed (Multisig, signer_removed)
        MultiSigSignerRemoved(AccountId, Signer),
        /// Change in signatures required by a multisig (Multisig, new_sigs_required)
        MultiSigSignaturesRequiredChanged(AccountId, u64),
    }
);

impl<T: Trait> Module<T> {
    /// Approves a multisig transaction and executes the proposal if enough sigs have been received
    fn approve_for(multi_sig: T::AccountId, proposal_id: u64, signer: Signer) -> Result {
        let multi_sig_signer_proposal = (multi_sig.clone(), signer, proposal_id);
        let multi_sig_proposal = (multi_sig.clone(), proposal_id);
        ensure!(!Self::votes(&multi_sig_signer_proposal), "Already approved");
        if let Some(proposal) = Self::proposals(&multi_sig_proposal) {
            Self::charge_fee(multi_sig.clone(), proposal.get_dispatch_info().weight)?;
            <Votes<T>>::insert(&multi_sig_signer_proposal, true);
            let approvals: u64 = Self::tx_approvals(&multi_sig_proposal) + 1u64;
            <TxApprovals<T>>::insert(&multi_sig_proposal, approvals);
            let approvals_needed = Self::ms_signs_required(multi_sig.clone());
            if approvals >= approvals_needed {
                let res =
                    match proposal.dispatch(system::RawOrigin::Signed(multi_sig.clone()).into()) {
                        Ok(_) => true,
                        Err(e) => {
                            let e: DispatchError = e.into();
                            sr_primitives::print(e);
                            false
                        }
                    };
                Self::deposit_event(RawEvent::ProposalExecuted(multi_sig, proposal_id, res));
                return Ok(());
            } else {
                return Ok(());
            }
        } else {
            return Err("Invalid proposal");
        }
    }

    /// Charges appropriate fee for the proposal
    fn charge_fee(_multi_sig: T::AccountId, _weight: Weight) -> Result {
        // TODO use this weight to charge appropriate fee
        Ok(())
    }

    /// Accept and process addition of a signer to a multisig
    pub fn _accept_multi_sig_signer(signer: Signer, auth_id: u64) -> Result {
        ensure!(
            <identity::Authorizations<T>>::exists((signer, auth_id)),
            AuthorizationError::Invalid.into()
        );

        let auth = <identity::Module<T>>::authorizations((signer, auth_id));

        ensure!(
            auth.authorization_data == AuthorizationData::AddMultisigSigner,
            "Not a multi sig signer auth"
        );

        let wallet_id;
        if let Signer::Key(multi_sig_key) = auth.authorized_by {
            wallet_id = T::AccountId::decode(&mut &multi_sig_key.as_slice()[..])
                .map_err(|_| "Error in decoding multisig address")?;
        } else {
            return Err("Error in decoding multisig address");
        }

        ensure!(
            <MultiSigSignsRequired<T>>::exists(&wallet_id),
            "Multi sig does not exist"
        );

        let wallet_signer = Signer::from(Key::try_from(wallet_id.encode())?);
        <identity::Module<T>>::consume_auth(wallet_signer, signer, auth_id)?;

        <MultiSigSigners<T>>::insert((wallet_id.clone(), signer), true);

        Self::deposit_event(RawEvent::MultiSigSignerAdded(wallet_id, signer));

        Ok(())
    }

    pub fn get_next_multi_sig_address(sender: T::AccountId) -> T::AccountId {
        let nonce: u64 = Self::ms_nonce();
        let new_nonce: u64 = nonce + 1u64;
        let h: T::Hash = T::Hashing::hash(&(b"MULTI_SIG", new_nonce, sender.clone()).encode());
        T::AccountId::decode(&mut &h.encode()[..]).unwrap_or_default()
    }
}

/// This trait is used to add a signer to a multisig
pub trait AddSignerMultisig {
    /// Accept and add a multisig signer
    ///
    /// # Arguments
    /// * `signer` did/key of the signer
    /// * `auth_id` Authorization id of the authorization created by the multisig
    fn accept_multi_sig_signer(signer: Signer, auth_id: u64) -> Result;
}

impl<T: Trait> AddSignerMultisig for Module<T> {
    fn accept_multi_sig_signer(signer: Signer, auth_id: u64) -> Result {
        Self::_accept_multi_sig_signer(signer, auth_id)
    }
}
