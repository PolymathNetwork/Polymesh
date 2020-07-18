use codec::Codec;
use pallet_identity::types::{AssetDidResult, CddStatus, DidRecords, DidStatus, Link, LinkType};
use sp_std::prelude::*;

sp_api::decl_runtime_apis! {
    pub trait IdentityApi<IdentityId, Ticker, AccountId, SigningItem, Signatory, Moment> where
        IdentityId: Codec,
        Ticker: Codec,
        AccountId: Codec,
        SigningItem: Codec,
        Signatory: Codec,
        Moment: Codec
    {
        /// Returns CDD status of an identity
        fn is_identity_has_valid_cdd(did: IdentityId, buffer_time: Option<u64>) -> CddStatus;

        /// Returns DID of an asset
        fn get_asset_did(ticker: Ticker) -> AssetDidResult;

        /// Retrieve DidRecord for a given `did`.
        fn get_did_records(did: IdentityId) -> DidRecords<AccountId, SigningItem>;

        /// Retrieve list of a link for a given signatory
        fn get_filtered_links(
            signatory: Signatory,
            allow_expired: bool,
            link_type: Option<LinkType>
        ) -> Vec<Link<Moment>>;

        /// Retrieve the status of the DID
        fn get_did_status(dids: Vec<IdentityId>) -> Vec<DidStatus>;
    }
}
