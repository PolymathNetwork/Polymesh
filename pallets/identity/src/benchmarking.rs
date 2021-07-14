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

use crate::*;

use confidential_identity::mocked::make_investor_uid as make_investor_uid_v2;
use confidential_identity_v1::mocked::make_investor_uid as make_investor_uid_v1;
use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use polymesh_common_utilities::{
    benchs::{AccountIdOf, User, UserBuilder},
    traits::{identity::TargetIdAuthorization, TestUtilsFn},
};
use polymesh_primitives::{
    investor_zkproof_data::{v1, v2},
    AuthorizationData, Claim, CountryCode, IdentityId, Permissions, Scope, ScopeId, SecondaryKey,
    Signatory,
};
use sp_std::prelude::*;

const SEED: u32 = 0;

fn setup_investor_uniqueness_claim_common<T, P, IF, CF, SF, IUF, PF>(
    name: &'static str,
    make_investor_uid: IF,
    make_cdd_id: CF,
    make_scope_id: SF,
    make_claim: IUF,
    make_proof: PF,
) -> (User<T>, Scope, Claim, P)
where
    T: Config + TestUtilsFn<AccountIdOf<T>>,
    IF: Fn(&[u8]) -> InvestorUid,
    CF: Fn(IdentityId, InvestorUid) -> CddId,
    SF: Fn(&[u8], &InvestorUid) -> IdentityId,
    PF: Fn(&IdentityId, &InvestorUid, &Ticker) -> P,
    IUF: Fn(Scope, ScopeId, CddId) -> Claim,
{
    let user = UserBuilder::<T>::default().generate_did().build(name);

    // Create CDD and add it to `user`.
    let did = user.did();
    let investor_uid = make_investor_uid(did.as_bytes());
    let cdd_id = make_cdd_id(did, investor_uid.clone());
    let cdd_claim = Claim::CustomerDueDiligence(cdd_id.clone());
    Module::<T>::base_add_claim(did, cdd_claim, GC_DID, None);

    // Create the scope.
    let ticker = Ticker::default();
    let scope_id = make_scope_id(&ticker.as_slice(), &investor_uid);

    let scope = Scope::Ticker(ticker);
    let claim = make_claim(scope.clone(), scope_id, cdd_id);
    let proof = make_proof(&did, &investor_uid, &ticker);
    (user, scope, claim, proof)
}

fn setup_investor_uniqueness_claim_v2<T>(
    name: &'static str,
) -> (User<T>, Scope, Claim, v2::InvestorZKProofData)
where
    T: Config + TestUtilsFn<AccountIdOf<T>>,
{
    setup_investor_uniqueness_claim_common::<T, _, _, _, _, _, _>(
        name,
        |raw_did| make_investor_uid_v2(raw_did).into(),
        CddId::new_v2,
        v2::InvestorZKProofData::make_scope_id,
        |_scope, _scope_id, cdd_id| Claim::InvestorUniquenessV2(cdd_id),
        v2::InvestorZKProofData::new,
    )
}

// NB As `schnorkell` uses internally a `RNG`, we have to use different `make_proof` for both
// environments.
// In `std`, we use directly the constructor from `v1::InvestorZKProofData`.
// In non `std`, we decode the proof from an hard-coded hexadecimal value, so we avoid the use of
// `schnorkell` functionality here.
fn setup_investor_uniqueness_claim_v1<T>(
    name: &'static str,
) -> (User<T>, Scope, Claim, v1::InvestorZKProofData)
where
    T: Config + TestUtilsFn<AccountIdOf<T>>,
{
    #[cfg(feature = "std")]
    let make_proof = v1::InvestorZKProofData::new;
    #[cfg(not(feature = "std"))]
    let make_proof = |_: &IdentityId, _: &InvestorUid, _: &Ticker| {
        let proof_encoded = hex::decode("0e0e257ad7cce3bd73462a28824134fff972df3379a9be9f0205d37fbde3212e51edd0a96a3b76df4a1c35b0d07394cad263d361c108d3ffa8efa10350410380").unwrap();
        <v1::InvestorZKProofData>::decode(&mut &proof_encoded[..]).expect("Invalid encoded proof")
    };

    setup_investor_uniqueness_claim_common::<T, _, _, _, _, _, _>(
        name,
        |raw_did| make_investor_uid_v1(raw_did).into(),
        CddId::new_v1,
        v1::InvestorZKProofData::make_scope_id,
        |scope, scope_id, cdd_id| Claim::InvestorUniqueness(scope, scope_id, cdd_id),
        make_proof,
    )
}

pub fn generate_secondary_keys<T: Config>(n: usize) -> Vec<SecondaryKey<T::AccountId>> {
    let mut secondary_keys = Vec::with_capacity(n);
    for x in 0..n {
        secondary_keys.push(SecondaryKey {
            signer: Signatory::Account(account("key", x as u32, SEED)),
            ..Default::default()
        });
    }
    secondary_keys
}

#[cfg(feature = "running-ci")]
mod limits {
    pub const MAX_SECONDARY_KEYS: u32 = 2;
}

#[cfg(not(feature = "running-ci"))]
mod limits {
    pub const MAX_SECONDARY_KEYS: u32 = 100;
}

use limits::*;

benchmarks! {
    where_clause { where T: TestUtilsFn<AccountIdOf<T>> }

    cdd_register_did {
        // Number of secondary items.
        let i in 0 .. MAX_SECONDARY_KEYS;

        let cdd = UserBuilder::<T>::default().generate_did().become_cdd_provider().build("cdd");
        let target: T::AccountId = account("target", SEED, SEED);
        let secondary_keys = generate_secondary_keys::<T>(i as usize);
    }: _(cdd.origin, target, secondary_keys)

    invalidate_cdd_claims {
        // NB: This function loops over all cdd claims issued by the cdd provider.
        // Therefore, it's unbounded in complexity. However, this can only be called by governance.
        // Hence, the weight is for best case scenario

        let cdd = UserBuilder::<T>::default().generate_did().become_cdd_provider().build("cdd");

    }: _(RawOrigin::Root, cdd.did(), 0u32.into(), None)

    remove_secondary_keys {
        // Number of secondary items.
        let i in 0 .. MAX_SECONDARY_KEYS;

        let target = UserBuilder::<T>::default().generate_did().build("target");

        let mut signatories = Vec::with_capacity(i as usize);
        for x in 0..i {
            let signer = Signatory::Account(account("key", x, SEED));
            signatories.push(signer.clone());
            Module::<T>::unsafe_join_identity(target.did(), Permissions::default(), &signer);
        }
    }: _(target.origin, signatories.clone())

    accept_primary_key {
        let cdd = UserBuilder::<T>::default().generate_did().become_cdd_provider().build("cdd");
        let target = UserBuilder::<T>::default().generate_did().build("target");
        let new_key = UserBuilder::<T>::default().build("key");
        let signatory = Signatory::Account(new_key.account());

        let cdd_auth_id =  Module::<T>::add_auth(
            cdd.did(), signatory.clone(),
            AuthorizationData::AttestPrimaryKeyRotation(target.did()),
            None,
        );
        Module::<T>::change_cdd_requirement_for_mk_rotation(
            RawOrigin::Root.into(),
            true
        ).unwrap();

        let owner_auth_id =  Module::<T>::add_auth(
            target.did(), signatory,
            AuthorizationData::RotatePrimaryKey(target.did()),
            None,
        );
    }: _(new_key.origin, owner_auth_id, Some(cdd_auth_id))

    change_cdd_requirement_for_mk_rotation {
        assert!(
            !Module::<T>::cdd_auth_for_primary_key_rotation(),
            "CDD auth for primary key rotation is enabled"
        );
    }: _(RawOrigin::Root, true)
    verify {
        assert!(
            Module::<T>::cdd_auth_for_primary_key_rotation(),
            "CDD auth for primary key rotation did not change"
        );
    }

    join_identity_as_key {
        let target = UserBuilder::<T>::default().generate_did().build("target");
        let new_key = UserBuilder::<T>::default().build("key");

        let auth_id =  Module::<T>::add_auth(
            target.did(),
            Signatory::Account(new_key.account()),
            AuthorizationData::JoinIdentity(Permissions::default()),
            None,
        );
    }: _(new_key.origin, auth_id)

    join_identity_as_identity {
        let target = UserBuilder::<T>::default().generate_did().build("target");
        let new_user = UserBuilder::<T>::default().generate_did().build("key");

        let auth_id =  Module::<T>::add_auth(
            target.did(),
            Signatory::Identity(new_user.did()),
            AuthorizationData::JoinIdentity(Permissions::default()),
            None,
        );
    }: _(new_user.origin, auth_id)

    leave_identity_as_key {
        let target = UserBuilder::<T>::default().generate_did().build("target");
        let key = UserBuilder::<T>::default().build("key");
        let signatory = Signatory::Account(key.account());

        let auth_id =  Module::<T>::add_auth(
            target.did(),
            signatory,
            AuthorizationData::JoinIdentity(Permissions::default()),
            None,
        );
        Module::<T>::join_identity_as_key(key.origin().into(), auth_id)
            .expect("Key cannot be joined to identity");

    }: _(key.origin())
    verify {
        assert!(
            !KeyToIdentityIds::<T>::contains_key(key.account),
            "Key was not removed from its identity"
        );
    }

    leave_identity_as_identity {
        let target = UserBuilder::<T>::default().generate_did().build("target");
        let new_user = UserBuilder::<T>::default().generate_did().build("key");
        let signatory = Signatory::Identity(new_user.did());

        Module::<T>::unsafe_join_identity(target.did(), Permissions::default(), &signatory);

    }: _(new_user.origin, target.did())

    add_claim {
        let caller = UserBuilder::<T>::default().generate_did().build("caller");
        let target = UserBuilder::<T>::default().generate_did().build("target");
        let scope = Scope::Identity(caller.did());
        let claim = Claim::Jurisdiction(CountryCode::BB, scope);
    }: _(caller.origin, target.did(), claim, Some(666u32.into()))

    revoke_claim {
        let (caller, scope, claim, proof) = setup_investor_uniqueness_claim_v1::<T>("caller");
        Module::<T>::add_investor_uniqueness_claim(caller.origin.clone().into(), caller.did(), claim.clone(), proof, Some(666u32.into())).unwrap();
    }: _(caller.origin, caller.did(), claim)

    revoke_claim_by_index {
        let (caller, scope, claim, proof) = setup_investor_uniqueness_claim_v2::<T>("caller");
        let claim_type = claim.claim_type();
        Module::<T>::add_investor_uniqueness_claim_v2(caller.origin.clone().into(), caller.did(), scope.clone(), claim, proof.0, Some(666u32.into())).unwrap();
    }: _(caller.origin, caller.did(), claim_type, Some(scope))

    set_permission_to_signer {
        let target = UserBuilder::<T>::default().generate_did().build("target");
        let key = UserBuilder::<T>::default().build("key");
        let signatory = Signatory::Account(key.account);

        Module::<T>::unsafe_join_identity(target.did(), Permissions::empty(), &signatory);
    }: _(target.origin, signatory, Permissions::default().into())

    freeze_secondary_keys {
        let caller = UserBuilder::<T>::default().generate_did().build("caller");
    }: _(caller.origin)

    unfreeze_secondary_keys {
        let caller = UserBuilder::<T>::default().generate_did().build("caller");
        Module::<T>::freeze_secondary_keys(caller.origin.clone().into()).unwrap();
    }: _(caller.origin)

    add_authorization {
        let caller = UserBuilder::<T>::default().generate_did().build("caller");
        let signatory = Signatory::Identity(caller.did());
        let auth_data = AuthorizationData::JoinIdentity(Permissions::default());
    }: _(caller.origin, signatory, auth_data, Some(666u32.into()))

    remove_authorization {
        let caller = UserBuilder::<T>::default().generate_did().build("caller");
        let signatory = Signatory::Identity(caller.did());
        let auth_id =  Module::<T>::add_auth(
            caller.did(),
            signatory.clone(),
            AuthorizationData::JoinIdentity(Permissions::default()),
            Some(666u32.into()),
        );
    }: _(caller.origin, signatory, auth_id, true)

    // TODO: fix this.
    // Account keyring is not available in no_std so it's not possible to sign data directly.
    // However, substrate injects the required functions as host functions in WASM.
    // We need to setup some helper functions to access those.
    // A defensive weight has been hardcoded for now.
    // add_secondary_keys_with_authorization {
    //     // Number of keys.
    //     let n in 0 .. 8;

    //     let (_, origin, did) = make_account::<T>("caller", SEED);

    //     let expires_at = 600u64;
    //     let authorization = TargetIdAuthorization::<u64> {
    //         target_id: did.clone(),
    //         nonce: Module::<T>::offchain_authorization_nonce(did),
    //         expires_at,
    //     };
    //     let auth_encoded = authorization.encode();

    //     let accounts = [
    //         AccountKeyring::Alice,
    //         AccountKeyring::Bob,
    //         AccountKeyring::Charlie,
    //         AccountKeyring::Dave,
    //         AccountKeyring::Eve,
    //         AccountKeyring::Ferdie,
    //         AccountKeyring::One,
    //         AccountKeyring::Two
    //     ];

    //     let secondary_keys_with_auth = accounts.into_iter().enumerate().take(n as usize).map(|(i, acc)| {
    //         let (_, _, key_did) = make_account::<T>("key", i as u32);
    //         let sig = H512::from(acc.sign(&auth_encoded));
    //         SecondaryKeyWithAuth {
    //             secondary_key: SecondaryKey::from(key_did).into(),
    //             auth_signature: sig,
    //         }
    //     }).collect::<Vec<_>>();

    // }: _(origin, secondary_keys_with_auth, expires_at)

    revoke_offchain_authorization {
        let caller = UserBuilder::<T>::default().generate_did().build("caller");
        let nonce = Module::<T>::offchain_authorization_nonce(caller.did());

        let authorization = TargetIdAuthorization::<T::Moment> {
            target_id: caller.did(),
            nonce,
            expires_at: 600u32.into(),
        };
    }: _(caller.origin, Signatory::Identity(caller.did()), authorization)

    add_investor_uniqueness_claim {
        let (caller, _, claim, proof) = setup_investor_uniqueness_claim_v1::<T>("caller");
    }: _(caller.origin, caller.did(), claim, proof, Some(666u32.into()))

    add_investor_uniqueness_claim_v2 {
        let (caller, scope, claim, proof) = setup_investor_uniqueness_claim_v2::<T>("caller");
    }: _(caller.origin, caller.did(), scope, claim, proof.0, Some(666u32.into()))
}
