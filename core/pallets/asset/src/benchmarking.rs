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

use frame_benchmarking::benchmarks;
use frame_support::StorageValue;
use frame_system::RawOrigin;
use polymesh_common_utilities::{
    benchs::{self, User, UserBuilder},
    constants::currency::POLY,
};
use polymesh_contracts::ExtensionInfo;
use polymesh_primitives::{
    asset::AssetName, ticker::TICKER_LEN, ExtensionAttributes, SmartExtension, Ticker,
};
use sp_io::hashing::keccak_256;
use sp_std::{convert::TryInto, iter, prelude::*};

const MAX_DOCS_PER_ASSET: u32 = 64;
const MAX_DOC_URI: usize = 4096;
const MAX_DOC_NAME: usize = 1024;
const MAX_DOC_TYPE: usize = 1024;
const MAX_IDENTIFIERS_PER_ASSET: u32 = 512;

pub fn make_ticker<T: Trait>(owner: T::Origin) -> Ticker {
    benchs::make_ticker::<T::AssetFn, T::Balance, T::AccountId, T::Origin, &str>(owner, None)
        .expect("Ticker cannot be created")
}

fn make_asset<T: Trait>(owner: &User<T>) -> Ticker {
    benchs::make_asset::<T::AssetFn, T, T::Balance, T::AccountId, T::Origin, &str>(owner, None)
        .expect("Asset cannot be created")
}

pub fn make_indivisible_asset<T: Trait>(owner: &User<T>) -> Ticker {
    benchs::make_indivisible_asset::<T::AssetFn, T, T::Balance, T::AccountId, T::Origin, &str>(
        owner, None,
    )
    .expect("Indivisible asset cannot be created")
}

pub fn make_document() -> Document {
    Document {
        uri: [b'u'; MAX_DOC_URI].into(),
        content_hash: b"572cdd8d8f1754dd0c4a75d99b569845"[..].try_into().unwrap(), // MD5 output is 128bits.
        name: [b'n'; MAX_DOC_NAME].into(),
        doc_type: Some([b't'; MAX_DOC_TYPE].into()),
        filing_date: None,
    }
}

fn make_default_reg_config<T: Trait>() -> TickerRegistrationConfig<T::Moment> {
    TickerRegistrationConfig {
        max_ticker_length: 8,
        registration_length: Some(10000u32.into()),
    }
}

fn make_classic_ticker<T: Trait>(eth_owner: ethereum::EthereumAddress, ticker: Ticker) {
    let classic_ticker = ClassicTickerImport {
        eth_owner,
        ticker,
        is_created: false,
        is_contract: false,
    };
    let reg_config = make_default_reg_config::<T>();
    let root = frame_system::RawOrigin::Root.into();

    <Module<T>>::reserve_classic_ticker(root, classic_ticker, 0u128.into(), reg_config)
        .expect("`reserve_classic_ticker` failed");
}

fn make_extension<T: Trait>(is_archive: bool) -> SmartExtension<T::AccountId> {
    // Simulate that extension was added.
    let extension_id = UserBuilder::<T>::default().build("extension").account;
    let extension_details = SmartExtension {
        extension_type: SmartExtensionType::TransferManager,
        extension_name: b"PTM".into(),
        extension_id: extension_id.clone(),
        is_archive,
    };

    // Add extension info into contracts wrapper.
    let version = 1u32;
    CompatibleSmartExtVersion::insert(&extension_details.extension_type, version);

    let attr = ExtensionAttributes {
        version,
        ..Default::default()
    };
    ExtensionInfo::<T>::insert(extension_id, attr);

    extension_details
}

fn add_ext<T: Trait>(is_archive: bool) -> (User<T>, Ticker, T::AccountId) {
    let owner = owner::<T>();
    let ticker = make_asset::<T>(&owner);
    let ext_details = make_extension::<T>(is_archive);
    let ext_id = ext_details.extension_id.clone();
    Module::<T>::add_extension(owner.origin().into(), ticker, ext_details)
        .expect("Extension cannot be added");
    (owner, ticker, ext_id)
}

fn emulate_controller_transfer<T: Trait>(
    ticker: Ticker,
    investor_did: IdentityId,
    pia: IdentityId,
) {
    // Assign balance to an investor.
    let mock_storage = |id: IdentityId, bal: T::Balance| {
        let s_id: ScopeId = id;
        <BalanceOf<T>>::insert(ticker, id, bal);
        <BalanceOfAtScope<T>>::insert(s_id, id, bal);
        <AggregateBalance<T>>::insert(ticker, id, bal);
        ScopeIdOf::insert(ticker, id, s_id);
        Statistics::<T>::update_transfer_stats(&ticker, None, Some(bal), bal);
    };
    mock_storage(investor_did, 1000u32.into());
    mock_storage(pia, 5000u32.into());
}

fn owner<T: Trait>() -> User<T> {
    UserBuilder::<T>::default().generate_did().build("owner")
}

pub fn owned_ticker<T: Trait>() -> (User<T>, Ticker) {
    let owner = owner::<T>();
    let ticker = make_asset::<T>(&owner);
    (owner, ticker)
}

fn verify_ownership<T: Trait>(
    ticker: Ticker,
    old: IdentityId,
    new: IdentityId,
    rel: AssetOwnershipRelation,
) {
    assert_eq!(
        Module::<T>::asset_ownership_relation(old, ticker),
        AssetOwnershipRelation::NotOwned
    );
    assert_eq!(Module::<T>::asset_ownership_relation(new, ticker), rel);
}

fn set_config<T: Trait>() {
    <TickerConfig<T>>::put(TickerRegistrationConfig {
        max_ticker_length: TICKER_LEN as u8,
        registration_length: Some((60u32 * 24 * 60 * 60).into()),
    });
}

benchmarks! {
    _ {}

    register_ticker {
        let caller = UserBuilder::<T>::default().generate_did().build("caller");
        // Generate a ticker of length `t`.
        set_config::<T>();
        let ticker = Ticker::repeating(b'A');
    }: _(caller.origin, ticker)
    verify {
        assert_eq!(Module::<T>::is_ticker_available(&ticker), false);
    }

    accept_ticker_transfer {
        let owner = owner::<T>();
        let ticker = make_ticker::<T>(owner.origin().into());
        let new_owner = UserBuilder::<T>::default().generate_did().build("new_owner");
        let did = new_owner.did();

        Module::<T>::asset_ownership_relation(owner.did(), ticker);
        let new_owner_auth_id = identity::Module::<T>::add_auth(
            owner.did(),
            Signatory::from(did),
            AuthorizationData::TransferTicker(ticker),
            None
        );
    }: _(new_owner.origin, new_owner_auth_id)
    verify {
        verify_ownership::<T>(ticker, owner.did(), did, AssetOwnershipRelation::TickerOwned);
    }

    accept_asset_ownership_transfer {
        let (owner, ticker) = owned_ticker::<T>();
        let new_owner = UserBuilder::<T>::default().generate_did().build("new_owner");
        let did = new_owner.did();

        let new_owner_auth_id = identity::Module::<T>::add_auth(
            owner.did(),
            Signatory::from(did),
            AuthorizationData::TransferAssetOwnership(ticker),
            None,
        );
    }: _(new_owner.origin, new_owner_auth_id)
    verify {
        assert_eq!(Module::<T>::token_details(&ticker).owner_did, did);
        verify_ownership::<T>(ticker, owner.did(), did, AssetOwnershipRelation::AssetOwned);
    }

    create_asset {
        // Token name length.
        let n in 1 .. T::AssetNameMaxLength::get() as u32;
        // Length of the vector of identifiers.
        let i in 1 .. MAX_IDENTIFIERS_PER_ASSET;
        // Funding round name length.
        let f in 1 .. T::FundingRoundNameMaxLength::get() as u32;

        set_config::<T>();
        let ticker = Ticker::repeating(b'A');
        let name = AssetName::from(vec![b'N'; n as usize].as_slice());

        let identifiers: Vec<_> = iter::repeat(AssetIdentifier::cusip(*b"17275R102").unwrap())
            .take(i as usize)
            .collect();
        let identifiers2 = identifiers.clone();
        let fundr = FundingRoundName::from(vec![b'F'; f as usize].as_slice());
        let owner = owner::<T>();
        let total_supply: T::Balance = 1_000_000u32.into();

        let asset_type = AssetType::default();
        let token = SecurityToken {
            name: name.clone(),
            owner_did: owner.did(),
            total_supply: total_supply.clone(),
            divisible: true,
            asset_type: asset_type.clone(),
            primary_issuance_agent: None,
        };
    }: _(owner.origin, name, ticker, total_supply, token.divisible, asset_type, identifiers, Some(fundr))
    verify {
        assert_eq!(Module::<T>::token_details(ticker), token);
        assert_eq!(Module::<T>::identifiers(ticker), identifiers2);
    }

    freeze {
        let (owner, ticker) = owned_ticker::<T>();
    }: _(owner.origin, ticker)
    verify {
        assert_eq!(Module::<T>::frozen(&ticker), true);
    }

    unfreeze {
        let (owner, ticker) = owned_ticker::<T>();

        Module::<T>::freeze(owner.origin().into(), ticker)
            .expect("Asset cannot be frozen");

        assert_eq!(Module::<T>::frozen(&ticker), true);
    }: _(owner.origin, ticker)
    verify {
        assert_eq!(Module::<T>::frozen(&ticker), false);
    }

    rename_asset {
        // New token name length.
        let n in 1 .. T::AssetNameMaxLength::get() as u32;

        let new_name = AssetName::from(vec![b'N'; n as usize].as_slice());
        let new_name2 = new_name.clone();
        let (owner, ticker) = owned_ticker::<T>();
    }: _(owner.origin, ticker, new_name)
    verify {
        assert_eq!(Module::<T>::token_details(ticker).name, new_name2);
    }

    issue {
        let (owner, ticker) = owned_ticker::<T>();
    }: _(owner.origin, ticker, (1_000_000 * POLY).into())
    verify {
        assert_eq!(Module::<T>::token_details(ticker).total_supply, (2_000_000 * POLY).into());
    }

    redeem {
        let (owner, ticker) = owned_ticker::<T>();
    }: _(owner.origin, ticker, (600_000 * POLY).into())
    verify {
        assert_eq!(Module::<T>::token_details(ticker).total_supply, (400_000 * POLY).into());
    }

    make_divisible {
        let owner = owner::<T>();
        let ticker = make_indivisible_asset::<T>(&owner);
    }: _(owner.origin, ticker)
    verify {
        assert_eq!(Module::<T>::token_details(ticker).divisible, true);
    }

    add_documents {
        // It starts at 1 in order to get something for `verify` section.
        let d in 1 .. MAX_DOCS_PER_ASSET;

        let (owner, ticker) = owned_ticker::<T>();
        let docs = iter::repeat(make_document()).take(d as usize).collect::<Vec<_>>();
        let docs2 = docs.clone();
    }: _(owner.origin, docs, ticker)
    verify {
        for i in 1..d {
            assert_eq!(Module::<T>::asset_documents(ticker, DocumentId(i)), docs2[i as usize]);
        }
    }

    remove_documents {
        let d in 1 .. MAX_DOCS_PER_ASSET;

        let (owner, ticker) = owned_ticker::<T>();
        let docs = iter::repeat(make_document())
            .take(MAX_DOCS_PER_ASSET as usize)
            .collect::<Vec<_>>();
        Module::<T>::add_documents(owner.origin().into(), docs.clone(), ticker)
            .expect("Documents cannot be added");

        let remove_doc_ids = (1..d).map(|i| DocumentId(i - 1)).collect::<Vec<_>>();
    }: _(owner.origin, remove_doc_ids, ticker)
    verify {
        for i in 1..d {
            assert_eq!(AssetDocuments::contains_key( &ticker, DocumentId(i-1)), false);
        }
    }

    set_funding_round {
        let f in 1 .. T::FundingRoundNameMaxLength::get() as u32;

        let (owner, ticker) = owned_ticker::<T>();
        let fundr = FundingRoundName::from(vec![b'X'; f as usize].as_slice());
        let fundr2 = fundr.clone();
    }: _(owner.origin, ticker, fundr)
    verify {
        assert_eq!(Module::<T>::funding_round(ticker), fundr2);
    }

    update_identifiers {
        let i in 1 .. MAX_IDENTIFIERS_PER_ASSET;

        let (owner, ticker) = owned_ticker::<T>();

        let identifiers: Vec<_> = iter::repeat(AssetIdentifier::cusip(*b"037833100").unwrap())
            .take(i as usize)
            .collect();
        let identifiers2 = identifiers.clone();
    }: _(owner.origin, ticker, identifiers)
    verify {
        assert_eq!(Module::<T>::identifiers(ticker), identifiers2);
    }

    add_extension {
        let (owner, ticker) = owned_ticker::<T>();
        let details = make_extension::<T>(false);
        let details2 = details.clone();
    }: _(owner.origin, ticker, details)
    verify {
        assert_eq!(details2, Module::<T>::extension_details((ticker, details2.extension_id.clone())));
    }

    archive_extension {
        let (owner, ticker, ext_id) = add_ext::<T>(false);
        let ext_id2 = ext_id.clone();
    }: _(owner.origin, ticker, ext_id)
    verify {
        assert_eq!(Module::<T>::extension_details((ticker, ext_id2)).is_archive, true);
    }

    unarchive_extension {
        let (owner, ticker, ext_id) = add_ext::<T>(true);
        let ext_id2 = ext_id.clone();
    }: _(owner.origin, ticker, ext_id)
    verify {
        assert_eq!(Module::<T>::extension_details((ticker, ext_id2)).is_archive, false);
    }

    remove_smart_extension {
        let (owner, ticker, ext_id) = add_ext::<T>(false);
        let ext_id2 = ext_id.clone();
    }: _(owner.origin, ticker, ext_id)
    verify {
        assert_eq!(<ExtensionDetails<T>>::contains_key((ticker, ext_id2)), false);
    }

    remove_primary_issuance_agent {
        let (owner, ticker) = owned_ticker::<T>();
    }: _(owner.origin, ticker)
    verify {
        assert_eq!(Module::<T>::token_details(ticker).primary_issuance_agent, None);
    }

    claim_classic_ticker {
        let owner = owner::<T>();
        let did = owner.did();
        let owner_eth_sk = secp256k1::SecretKey::parse(&keccak_256(b"owner")).unwrap();
        let owner_eth_pk = ethereum::address(&owner_eth_sk);

        let ticker: Ticker = b"USDX1"[..].try_into().unwrap();
        make_classic_ticker::<T>(owner_eth_pk, ticker);

        let eth_sig = ethereum::eth_msg(did, b"classic_claim", &owner_eth_sk);
    }: _(owner.origin, ticker, eth_sig)
    verify {
        assert_eq!(did, Module::<T>::ticker_registration(ticker).owner);
    }

    reserve_classic_ticker {
        let owner = owner::<T>();

        let ticker: Ticker = b"ACME"[..].try_into().unwrap();
        let config = make_default_reg_config::<T>();
        let classic = ClassicTickerImport {
            eth_owner: ethereum::EthereumAddress(*b"0x012345678987654321"),
            ticker,
            is_created: true,
            is_contract: false,
        };
    }: _(RawOrigin::Root, classic, owner.did(), config)
    verify {
        assert_eq!(<Tickers<T>>::contains_key(&ticker), true);
    }

    accept_primary_issuance_agent_transfer {
        let (owner, ticker) = owned_ticker::<T>();
        let pia = UserBuilder::<T>::default().generate_did().build("1stIssuance");

        let auth_id = identity::Module::<T>::add_auth(
            owner.did(),
            Signatory::from(pia.did()),
            AuthorizationData::TransferPrimaryIssuanceAgent(ticker),
            None,
        );
    }: _(pia.origin, auth_id)
    verify {
        assert_eq!(Module::<T>::token_details(&ticker).primary_issuance_agent, pia.did);
    }

    controller_transfer {
        let (owner, ticker) = owned_ticker::<T>();
        let pia = UserBuilder::<T>::default().generate_did().build("1stIssuance");
        let investor = UserBuilder::<T>::default().generate_did().build("investor");
        let auth_id = identity::Module::<T>::add_auth(
            owner.did(),
            Signatory::from(pia.did()),
            AuthorizationData::TransferPrimaryIssuanceAgent(ticker),
            None,
        );
        Module::<T>::accept_primary_issuance_agent_transfer(pia.origin().into(), auth_id)?;
        emulate_controller_transfer::<T>(ticker, investor.did(), pia.did());
        let portfolio_to = PortfolioId::default_portfolio(investor.did());
    }: _(pia.origin, ticker, 500u32.into(), portfolio_to)
    verify {
        assert_eq!(Module::<T>::balance_of(ticker, investor.did()), 500u32.into());
    }
}
