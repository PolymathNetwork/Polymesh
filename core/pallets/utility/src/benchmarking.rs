use crate::*;
use frame_benchmarking::benchmarks;
use pallet_balances::{self as balances, Call as BalancesCall};
use polymesh_common_utilities::benchs::{User, UserBuilder};
use sp_core::sr25519::Signature;
use sp_runtime::traits::StaticLookup;
use sp_runtime::MultiSignature;

const MAX_CALLS: u32 = 30;

/// Generate `c` no-op system remark calls.
fn make_calls<T: Trait>(c: u32) -> Vec<<T as Trait>::Call> {
    let call: <T as Trait>::Call = frame_system::Call::<T>::remark(vec![]).into();
    vec![call; c as usize]
}

/// Generate `c` transfers calls to `to` account of `amount` poly.
fn make_transfer_calls<T: Trait>(
    c: u32,
    to: T::AccountId,
    amount: u128,
) -> Vec<<T as Trait>::Call> {
    let idx = <T as frame_system::Trait>::Lookup::unlookup(to);
    let call: <T as Trait>::Call = BalancesCall::transfer(idx, amount.into()).into();
    vec![call; c as usize]
}

/// Double-check that free balance of `account` account is the expected value.
fn verify_free_balance<T: Trait>(account: &T::AccountId, expected_balance: u128) {
    let acc_balance = balances::Module::<T>::free_balance(account);
    assert_eq!(acc_balance, expected_balance.into())
}

fn make_relay_tx_users<T: Trait>() -> (User<T>, User<T>) {
    let alice = UserBuilder::<T>::default()
        .balance(1_000_000u32)
        .generate_did()
        .build("Caller");
    let bob = UserBuilder::<T>::default()
        .balance(1_000_000u32)
        .generate_did()
        .build("Target");

    (alice, bob)
}

fn remark_call_builder<T: Trait>(
    signer: &User<T>,
    _: T::AccountId,
) -> (UniqueCall<<T as Trait>::Call>, Vec<u8>) {
    let call = make_calls::<T>(1).pop().unwrap();
    let nonce: AuthorizationNonce = Module::<T>::nonce(signer.account());
    let call = UniqueCall::new(nonce, call);

    // Signer signs the relay call.
    // NB: Decode as T::OffChainSignature because there is not type constraints in
    // `T::OffChainSignature` to limit it.
    let raw_signature: [u8; 64] = signer
        .sign(&call.encode())
        .expect("Data cannot be signed")
        .0;
    let encoded = MultiSignature::from(Signature::from_raw(raw_signature)).encode();

    (call, encoded)
}

fn transfer_call_builder<T: Trait>(
    signer: &User<T>,
    target: T::AccountId,
) -> (UniqueCall<<T as Trait>::Call>, Vec<u8>) {
    let call = make_transfer_calls::<T>(1, target, 1).pop().unwrap();
    let nonce: AuthorizationNonce = Module::<T>::nonce(signer.account());
    let call = UniqueCall::new(nonce, call);

    // Signer signs the relay call.
    // NB: Decode as T::OffChainSignature because there is not type constraints in
    // `T::OffChainSignature` to limit it.
    let raw_signature: [u8; 64] = signer
        .sign(&call.encode())
        .expect("Data cannot be signed")
        .0;
    let encoded = MultiSignature::from(Signature::from_raw(raw_signature)).encode();

    (call, encoded)
}

benchmarks! {
    _ {}

    batch {
        let c in 0..MAX_CALLS;

        let u = UserBuilder::<T>::default().generate_did().build("ALICE");
        let calls = make_calls::<T>(c);

    }: _(u.origin, calls)
    verify {
        // NB In this case we are using `frame_system::Call::<T>::remark` which makes *no DB
        // operations*. This helps us to fetch the DB read/write ops only from `batch` instead of
        // its batched calls.
        // So there is no way to verify it.
        // The following cases use `balances::transfer` to be able to verify their outputs.
    }

    batch_transfer {
        let c in 0..MAX_CALLS;

        let sender = UserBuilder::<T>::default().balance(1_000_000u32).generate_did().build("SENDER");
        let receiver = UserBuilder::<T>::default().balance(1_000_000u32).generate_did().build("RECEIVER");

        let transfer_calls = make_transfer_calls::<T>(c, receiver.account(), 500);
    }: batch(sender.origin, transfer_calls)
    verify {
        verify_free_balance::<T>( &sender.account, (1_000_000 - (500 * c)) as u128);
        verify_free_balance::<T>( &receiver.account, (1_000_000 + (500 * c)) as u128);
    }

    batch_atomic {
        let c in 0..MAX_CALLS;

        let alice = UserBuilder::<T>::default().generate_did().build("ALICE");
        let calls = make_calls::<T>(c);
    }: _(alice.origin, calls)
    verify {
        // NB see comment at `batch` verify section.
    }

    batch_atomic_transfer {
        let c in 0..MAX_CALLS;

        let alice = UserBuilder::<T>::default().balance(1_000_000u32).generate_did().build("ALICE");
        let bob = UserBuilder::<T>::default().balance(1_000_000u32).generate_did().build("BOB");
        let calls = make_transfer_calls::<T>(c, bob.account(), 100);

    }: batch_atomic(alice.origin, calls)
    verify {
        verify_free_balance::<T>( &alice.account, (1_000_000 - (100 * c)) as u128);
        verify_free_balance::<T>( &bob.account, (1_000_000 + (100 * c)) as u128);
    }

    batch_optimistic {
        let c in 0..MAX_CALLS;

        let alice = UserBuilder::<T>::default().generate_did().build("ALICE");
        let calls = make_calls::<T>(c);

    }: _(alice.origin, calls)
    verify {
        // NB see comment at `batch` verify section.
    }

    batch_optimistic_transfer {
        let c in 0..MAX_CALLS;

        let alice = UserBuilder::<T>::default().balance(1_000_000u32).generate_did().build("ALICE");
        let bob = UserBuilder::<T>::default().balance(1_000_000u32).generate_did().build("BOB");
        let calls = make_transfer_calls::<T>(c, bob.account(), 100);

    }: batch_optimistic(alice.origin, calls)
    verify {
        verify_free_balance::<T>( &alice.account, (1_000_000 - (100 * c)) as u128);
        verify_free_balance::<T>( &bob.account, (1_000_000 + (100 * c)) as u128);
    }

    relay_tx {
        let (caller, target) = make_relay_tx_users::<T>();
        let (call, encoded) = remark_call_builder( &target, caller.account());

        // Rebuild signature from `encoded`.
        let signature = T::OffChainSignature::decode(&mut &encoded[..])
            .expect("OffChainSignature cannot be decoded from a MultiSignature");

    }: _(caller.origin.clone(), target.account(), signature, call)
    verify {
        // NB see comment at `batch` verify section.
    }

    relay_tx_transfer {
        let (caller, target) = make_relay_tx_users::<T>();
        let (call, encoded) = transfer_call_builder( &target, caller.account());

        // Rebuild signature from `encoded`.
        let signature = T::OffChainSignature::decode(&mut &encoded[..])
            .expect("OffChainSignature cannot be decoded from a MultiSignature");
    }: relay_tx(caller.origin.clone(), target.account(), signature, call)
    verify {
        verify_free_balance::<T>( &caller.account, 1_000_001u128);
        verify_free_balance::<T>( &target.account, 999_999u128);
    }
}
