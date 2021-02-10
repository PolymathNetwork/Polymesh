pub use pallet_group_rpc_runtime_api::{GroupApi as GroupRuntimeApi, Member};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use sp_std::prelude::*;
use std::{marker::PhantomData, sync::Arc};

/// Group RPC methods.
#[rpc]
pub trait GroupApi<BlockHash> {
    /// Valid members: active member and inactive who are not yet expired.
    #[rpc(name = "group_getCDDValidMembers")]
    fn get_cdd_valid_members(&self, at: Option<BlockHash>) -> Result<Vec<Member>>;

    #[rpc(name = "group_getGCValidMembers")]
    fn get_gc_valid_members(&self, at: Option<BlockHash>) -> Result<Vec<Member>>;
}

pub struct Group<T, U> {
    client: Arc<T>,
    _marker: PhantomData<U>,
}

impl<T, U> From<Arc<T>> for Group<T, U> {
    fn from(client: Arc<T>) -> Self {
        Group {
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

impl<C, Block> GroupApi<<Block as BlockT>::Hash> for Group<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: GroupRuntimeApi<Block>,
{
    fn get_cdd_valid_members(&self, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<Member>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.get_cdd_valid_members(&at).map_err(|e| RpcError {
            code: ErrorCode::ServerError(Error::RuntimeError as i64),
            message: "Unable to fetch CDD providers.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_gc_valid_members(&self, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<Member>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.get_gc_valid_members(&at).map_err(|e| RpcError {
            code: ErrorCode::ServerError(Error::RuntimeError as i64),
            message: "Unable to fetch Governance Committee members.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
