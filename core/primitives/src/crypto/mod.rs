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

use sp_core::sr25519::Signature;
use sp_runtime_interface::runtime_interface;

/// Native interface for runtime module to use some Schnorrkel functionality.
#[runtime_interface]
pub trait NativeSchnorrkel {
    /// Sign the message `message`, using the given secret key.
    /// It returns `None` if the secret key cannot be created from the input raw bytes.
    fn sign(raw_sk: [u8; 64], message: &[u8]) -> Option<Signature> {
        use schnorrkel::{keys::SecretKey, signing_context, Keypair};
        const SIGNING_CTX: &[u8] = b"substrate";

        SecretKey::from_bytes(&raw_sk[..])
            .map(|sk| {
                let pair = Keypair::from(sk);
                let context = signing_context(SIGNING_CTX);
                let raw_signature = pair.sign(context.bytes(message)).to_bytes();
                Signature::from_raw(raw_signature)
            })
            .ok()
    }
}
