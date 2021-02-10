//! Utilities and types for dealing with Ethereum addresses and messages.

// Logic is largely taken from:
// https://github.com/paritytech/polkadot/blob/013c4a8041e6f1739cc5b785a2874061919c5db9/runtime/common/src/claims.rs#L248-L251

use codec::{Decode, Encode};
use sp_io::{crypto::secp256k1_ecdsa_recover, hashing::keccak_256};
#[cfg(feature = "std")]
use sp_runtime::{Deserialize, Serialize};
use sp_std::vec::Vec;

/// An Ethereum address (i.e. 20 bytes, used to represent an Ethereum account).
///
/// This gets serialized to the 0x-prefixed hex representation.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, Default, Debug)]
pub struct EthereumAddress(pub [u8; 20]);

#[derive(Encode, Decode, Clone)]
pub struct EcdsaSignature(pub [u8; 65]);

impl PartialEq for EcdsaSignature {
    fn eq(&self, other: &Self) -> bool {
        &self.0[..] == &other.0[..]
    }
}

impl sp_std::fmt::Debug for EcdsaSignature {
    fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
        write!(f, "EcdsaSignature({:?})", &self.0[..])
    }
}

/// Check that `data` is the message of `ecdsa_sig` and return the Ethereum address.
pub fn eth_check(
    data: impl Encode,
    prefix: &[u8],
    ecdsa_sig: &EcdsaSignature,
) -> Option<EthereumAddress> {
    let data = data.using_encoded(to_ascii_hex);
    eth_recover(&ecdsa_sig, prefix, &data, &[])
}

/// Returns a signature for `prefix` combined with `data` as a message,
/// signed by the given `secret` key.
pub fn eth_msg(data: impl Encode, prefix: &[u8], secret: &secp256k1::SecretKey) -> EcdsaSignature {
    sig(secret, prefix, &data.encode(), &[])
}

/// Converts the given binary data into ASCII-encoded hex. It will be twice the length.
fn to_ascii_hex(data: &[u8]) -> Vec<u8> {
    let mut r = Vec::with_capacity(data.len() * 2);
    let mut push_nibble = |n| r.push(if n < 10 { b'0' + n } else { b'a' - 10 + n });
    for &b in data.iter() {
        push_nibble(b / 16);
        push_nibble(b % 16);
    }
    r
}

// Constructs the message that Ethereum RPC's `personal_sign` and `eth_sign` would sign.
fn ethereum_signable_message(prefix: &[u8], what: &[u8], extra: &[u8]) -> Vec<u8> {
    let mut l = prefix.len() + what.len() + extra.len();
    let mut rev = Vec::new();
    while l > 0 {
        rev.push(b'0' + (l % 10) as u8);
        l /= 10;
    }
    let head = b"\x19Ethereum Signed Message:\n";
    let len = [head, &rev as &[u8], &prefix, what, extra]
        .iter()
        .map(|p| p.len())
        .sum();
    let mut v = Vec::with_capacity(len);
    v.extend_from_slice(head);
    v.extend(rev.into_iter().rev());
    v.extend_from_slice(&prefix[..]);
    v.extend_from_slice(what);
    v.extend_from_slice(extra);
    v
}

// Attempts to recover the Ethereum address from a message signature signed by using
// the Ethereum RPC's `personal_sign` and `eth_sign`.
fn eth_recover(
    s: &EcdsaSignature,
    prefix: &[u8],
    what: &[u8],
    extra: &[u8],
) -> Option<EthereumAddress> {
    let msg = keccak_256(&ethereum_signable_message(prefix, what, extra));
    Some(acc_from_data(&secp256k1_ecdsa_recover(&s.0, &msg).ok()?))
}

fn acc_from_data(data: &[u8]) -> EthereumAddress {
    let mut res = EthereumAddress::default();
    res.0.copy_from_slice(&keccak_256(data)[12..]);
    res
}

/// Returns the public key derived from the given `secret` key.
fn public(secret: &secp256k1::SecretKey) -> secp256k1::PublicKey {
    secp256k1::PublicKey::from_secret_key(secret)
}

/// Derive the Ethereum address from the `secret` key.
pub fn address(secret: &secp256k1::SecretKey) -> EthereumAddress {
    acc_from_data(&public(secret).serialize()[1..65])
}

/// Signs the message `prefix ++ what ++ extra` using the `secret` key.
fn sig(secret: &secp256k1::SecretKey, prefix: &[u8], what: &[u8], extra: &[u8]) -> EcdsaSignature {
    let msg = ethereum_signable_message(prefix, &to_ascii_hex(what), extra);
    let msg = keccak_256(&msg);
    let (sig, recovery_id) = secp256k1::sign(&secp256k1::Message::parse(&msg), secret);
    let mut r = [0u8; 65];
    r[0..64].copy_from_slice(&sig.serialize());
    r[64] = recovery_id.serialize();
    EcdsaSignature(r)
}
