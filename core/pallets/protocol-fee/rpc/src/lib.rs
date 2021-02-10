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

use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
pub use pallet_protocol_fee_rpc_runtime_api::{CappedFee, ProtocolFeeApi as ProtocolFeeRuntimeApi};
use polymesh_common_utilities::protocol_fee::ProtocolOp;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

#[rpc]
pub trait ProtocolFeeApi<BlockHash> {
    #[rpc(name = "protocolFee_computeFee")]
    fn compute_fee(&self, op: ProtocolOp, at: Option<BlockHash>) -> Result<CappedFee>;
}

/// A struct that implements the [`ProtocolFeeApi`].
pub struct ProtocolFee<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> ProtocolFee<C, M> {
    /// Create new `ProtocolFee` instance with the given reference to the client.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

/// Error type of this RPC API.
pub enum Error {
    /// The transaction was not decodable.
    DecodeError = 1,
    /// The call to runtime failed.
    RuntimeError = 2,
}

impl<C, Block> ProtocolFeeApi<<Block as BlockT>::Hash> for ProtocolFee<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: ProtocolFeeRuntimeApi<Block>,
{
    fn compute_fee(
        &self,
        op: ProtocolOp,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<CappedFee> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(||
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash));

        api.compute_fee(&at, op).map_err(|e| RpcError {
            code: ErrorCode::ServerError(Error::RuntimeError as i64),
            message: "Unable to query dispatch info.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
