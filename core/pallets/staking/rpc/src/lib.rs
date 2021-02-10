use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
pub use pallet_staking_rpc_runtime_api::StakingApi as StakingRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT, Perbill};
use std::sync::Arc;

#[rpc]
pub trait StakingApi<BlockHash> {
    #[rpc(name = "staking_getCurve")]
    fn get_curve(&self, at: Option<BlockHash>) -> Result<Vec<(Perbill, Perbill)>>;
}

/// A struct that implements the [`StakingApi`].
pub struct Staking<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> Staking<C, M> {
    /// Create new `Staking` instance with the given reference to the client.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

/// Error type of this RPC api.
pub enum Error {
    /// The transaction was not decodable.
    DecodeError,
    /// The call to runtime failed.
    RuntimeError,
}

impl From<Error> for i64 {
    fn from(e: Error) -> i64 {
        match e {
            Error::RuntimeError => 1,
            Error::DecodeError => 2,
        }
    }
}

impl<C, Block> StakingApi<<Block as BlockT>::Hash> for Staking<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: StakingRuntimeApi<Block>,
{
    fn get_curve(&self, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<(Perbill, Perbill)>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(||
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash));

        api.get_curve(&at).map_err(|e| RpcError {
            code: ErrorCode::ServerError(Error::RuntimeError.into()),
            message: "Unable to query dispatch info.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
