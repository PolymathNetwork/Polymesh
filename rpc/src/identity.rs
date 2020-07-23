pub use pallet_identity::types::{AssetDidResult, CddStatus, DidRecords, DidStatus};
use polymesh_primitives::{Authorization, AuthorizationType};

pub use node_rpc_runtime_api::identity::IdentityApi as IdentityRuntimeApi;

use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, Zero},
};
use std::{convert::TryInto, sync::Arc};

const MAX_IDENTITIES_ALLOWED_TO_QUERY: u32 = 500;

/// Identity RPC methods
#[rpc]
pub trait IdentityApi<BlockHash, IdentityId, Ticker, AccountId, SigningKey, Signatory, Moment> {
    /// Below function use to tell whether the given did has valid cdd claim or not
    #[rpc(name = "identity_isIdentityHasValidCdd")]
    fn is_identity_has_valid_cdd(
        &self,
        did: IdentityId,
        buffer_time: Option<u64>,
        at: Option<BlockHash>,
    ) -> Result<CddStatus>;

    /// Below function is used to query the given ticker DID.
    #[rpc(name = "identity_getAssetDid")]
    fn get_asset_did(&self, ticker: Ticker, at: Option<BlockHash>) -> Result<AssetDidResult>;

    /// DidRecords for a `did`
    #[rpc(name = "identity_getDidRecords")]
    fn get_did_records(
        &self,
        did: IdentityId,
        at: Option<BlockHash>,
    ) -> Result<DidRecords<AccountId, SigningKey>>;

    /// Retrieve the list of authorizations for a given signatory.
    #[rpc(name = "identity_getFilteredAuthorizations")]
    fn get_filtered_authorizations(
        &self,
        signatory: Signatory,
        allow_expired: bool,
        auth_type: Option<AuthorizationType>,
        at: Option<BlockHash>,
    ) -> Result<Vec<Authorization<AccountId, Moment>>>;

    /// Provide the status of a given DID
    #[rpc(name = "identity_getDidStatus")]
    fn get_did_status(
        &self,
        dids: Vec<IdentityId>,
        at: Option<BlockHash>,
    ) -> Result<Vec<DidStatus>>;
}

/// A struct that implements the [`IdentityApi`].
pub struct Identity<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> Identity<C, M> {
    /// Create new `Identity` instance with the given reference to the client.
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

impl<C, Block, IdentityId, Ticker, AccountId, SigningKey, Signatory, Moment>
    IdentityApi<
        <Block as BlockT>::Hash,
        IdentityId,
        Ticker,
        AccountId,
        SigningKey,
        Signatory,
        Moment,
    > for Identity<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api:
        IdentityRuntimeApi<Block, IdentityId, Ticker, AccountId, SigningKey, Signatory, Moment>,
    IdentityId: Codec,
    Ticker: Codec,
    AccountId: Codec,
    SigningKey: Codec,
    Signatory: Codec,
    Moment: Codec,
{
    fn is_identity_has_valid_cdd(
        &self,
        did: IdentityId,
        buffer_time: Option<u64>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<CddStatus> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(||
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash));
        api.is_identity_has_valid_cdd(&at, did, buffer_time)
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(Error::RuntimeError as i64),
                message: "Either cdd claim not exist or Identity.".into(),
                data: Some(format!("{:?}", e).into()),
            })
    }

    fn get_asset_did(
        &self,
        ticker: Ticker,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<AssetDidResult> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(||
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash));
        api.get_asset_did(&at, ticker).map_err(|e| RpcError {
            code: ErrorCode::ServerError(Error::RuntimeError as i64),
            message: "Unable to fetch did of the given ticker".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_did_records(
        &self,
        did: IdentityId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<DidRecords<AccountId, SigningKey>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        api.get_did_records(&at, did).map_err(|e| RpcError {
            code: ErrorCode::ServerError(Error::RuntimeError as i64),
            message: "Unable to fetch DID records".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_filtered_authorizations(
        &self,
        signatory: Signatory,
        allow_expired: bool,
        auth_type: Option<AuthorizationType>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Vec<Authorization<AccountId, Moment>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        api.get_filtered_authorizations(&at, signatory, allow_expired, auth_type)
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(Error::RuntimeError as i64),
                message: "Unable to fetch authorizations data".into(),
                data: Some(format!("{:?}", e).into()),
            })
    }

    fn get_did_status(
        &self,
        dids: Vec<IdentityId>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Vec<DidStatus>> {
        if dids.len()
            > MAX_IDENTITIES_ALLOWED_TO_QUERY
                .try_into()
                .unwrap_or(Zero::zero())
        {
            return Err(RpcError {
                code: ErrorCode::ServerError(Error::RuntimeError as i64),
                message: "Unable to fetch dids status".into(),
                data: Some(
                    format!(
                        "Provided vector length is more than the maximum allowed length i.e {:?}",
                        MAX_IDENTITIES_ALLOWED_TO_QUERY
                    )
                    .into(),
                ),
            });
        }
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        api.get_did_status(&at, dids).map_err(|e| RpcError {
            code: ErrorCode::ServerError(Error::RuntimeError as i64),
            message: "Unable to fetch dids status".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
