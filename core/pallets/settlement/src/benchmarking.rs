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

pub use frame_benchmarking::{account, benchmarks};
use frame_support::{traits::Get, weights::Weight};
use frame_system::RawOrigin;
use pallet_asset::{BalanceOf, SecurityToken, Tokens};
use pallet_contracts::ContractAddressFor;
use pallet_identity as identity;
use pallet_portfolio::PortfolioAssetBalances;
use pallet_statistics::TransferManager;
use polymesh_common_utilities::{
    benchs::{self, generate_ticker, user, User, UserBuilder},
    constants::currency::POLY,
    traits::asset::AssetFnTrait,
};
use polymesh_primitives::{
    asset::{AssetName, AssetType},
    Claim, Condition, ConditionType, CountryCode, IdentityId, PortfolioId, PortfolioName,
    PortfolioNumber, Scope, SmartExtension, SmartExtensionType, Ticker, TrustedIssuer,
};
use sp_runtime::traits::Hash;
use sp_runtime::SaturatedConversion;
use sp_std::convert::TryInto;
use sp_std::prelude::*;

#[cfg(not(feature = "std"))]
use hex_literal::hex;

use sp_core::sr25519::Signature;
use sp_runtime::MultiSignature;

const MAX_VENUE_DETAILS_LENGTH: u32 = 100000;
const MAX_SIGNERS_ALLOWED: u32 = 50;
const MAX_VENUE_ALLOWED: u32 = 100;
const MAX_LEGS_IN_INSTRUCTION: u32 = 25;

type Portfolio<T> = pallet_portfolio::Module<T>;

#[derive(Encode, Decode, Clone, Copy)]
pub struct UserData<T: Trait> {
    pub account: T::AccountId,
    pub did: IdentityId,
}

impl<T: Trait> From<User<T>> for UserData<T> {
    fn from(user: User<T>) -> Self {
        Self {
            account: user.account(),
            did: user.did(),
        }
    }
}

fn make_asset<T: Trait>(owner: &User<T>, name: Option<Vec<u8>>) -> Ticker {
    benchs::make_asset::<T::AssetFn, T, T::Balance, T::AccountId, T::Origin, Vec<u8>>(owner, name)
        .expect("Asset cannot be created")
}

fn set_block_number<T: Trait>(new_block_no: u64) {
    system::Module::<T>::set_block_number(new_block_no.saturated_into::<T::BlockNumber>());
}

/// Set venue related storage without any sanity checks.
fn create_venue_<T: Trait>(did: IdentityId, signers: Vec<T::AccountId>) -> u64 {
    // Worst case length for the venue details.
    let venue_details = VenueDetails::from(vec![b'A'; 200 as usize].as_slice());
    let venue = Venue::new(did, venue_details, VenueType::Distribution);
    // NB: Venue counter starts with 1.
    let venue_counter = Module::<T>::venue_counter();
    <VenueInfo>::insert(venue_counter, venue);
    for signer in signers {
        <VenueSigners<T>>::insert(venue_counter, signer, true);
    }
    <VenueCounter>::put(venue_counter + 1);
    Module::<T>::venue_counter() - 1
}

/// Set instruction leg status to `LegStatus::ExecutionToBeSkipped` without any sanity checks.
fn set_instruction_leg_status_to_skipped<T: Trait>(
    instruction_id: u64,
    leg_id: u64,
    signer: T::AccountId,
    receipt_uid: u64,
) {
    <ReceiptsUsed<T>>::insert(&signer, receipt_uid, true);
    <InstructionLegStatus<T>>::insert(
        instruction_id,
        leg_id,
        LegStatus::ExecutionToBeSkipped(signer, receipt_uid),
    );
}

/// Set Leg status to `LegStatus::ExecutionPending`
fn set_instruction_leg_status_to_pending<T: Trait>(instruction_id: u64, leg_id: u64) {
    <InstructionLegStatus<T>>::insert(instruction_id, leg_id, LegStatus::ExecutionPending);
}

/// Set user affirmation without any sanity checks.
fn set_user_affirmations(instruction_id: u64, portfolio: PortfolioId, affirm: AffirmationStatus) {
    UserAffirmations::insert(portfolio, instruction_id, affirm);
}

// create asset
fn create_asset_<T: Trait>(owner_did: IdentityId) -> Result<Ticker, DispatchError> {
    let ticker = Ticker::try_from(generate_ticker(8u64).as_slice()).unwrap();
    let name = AssetName::from(vec![b'N'; 8 as usize].as_slice());
    let total_supply: T::Balance = 90000u32.into();
    let token = SecurityToken {
        name,
        total_supply,
        owner_did,
        divisible: true,
        asset_type: AssetType::EquityCommon,
        primary_issuance_agent: None,
    };
    <Tokens<T>>::insert(ticker, token);
    <BalanceOf<T>>::insert(ticker, owner_did, total_supply);
    Portfolio::<T>::set_default_portfolio_balance(owner_did, &ticker, total_supply);
    Ok(ticker)
}

// fund portfolio
fn fund_portfolio<T: Trait>(portfolio: &PortfolioId, ticker: &Ticker, amount: T::Balance) {
    <PortfolioAssetBalances<T>>::insert(portfolio, ticker, amount);
}

fn setup_leg_and_portfolio<T: Trait>(
    to_user: Option<UserData<T>>,
    from_user: Option<UserData<T>>,
    index: u32,
    legs: &mut Vec<Leg<T::Balance>>,
    sender_portfolios: &mut Vec<PortfolioId>,
    receiver_portfolios: &mut Vec<PortfolioId>,
) {
    let variance = index + 1;
    let ticker = Ticker::try_from(generate_ticker(variance.into()).as_slice()).unwrap();
    let portfolio_from = generate_portfolio::<T>("", variance + 500, from_user);
    let _ = fund_portfolio::<T>(&portfolio_from, &ticker, 500u32.into());
    let portfolio_to = generate_portfolio::<T>("to_did", variance + 800, to_user);
    legs.push(Leg {
        from: portfolio_from,
        to: portfolio_to,
        asset: ticker,
        amount: 100u32.into(),
    });
    receiver_portfolios.push(portfolio_to);
    sender_portfolios.push(portfolio_from);
}

fn generate_portfolio<T: Trait>(
    portfolio_to: &'static str,
    pseudo_random_no: u32,
    user: Option<UserData<T>>,
) -> PortfolioId {
    let u = match user {
        None => {
            let user = UserBuilder::<T>::default()
                .generate_did()
                .seed(pseudo_random_no)
                .build(portfolio_to);
            UserData::from(user)
        }
        Some(u) => u,
    };
    let portfolio_no = (Portfolio::<T>::next_portfolio_number(u.did)).0;
    let portfolio_name =
        PortfolioName::try_from(vec![b'P'; portfolio_no as usize].as_slice()).unwrap();
    Portfolio::<T>::create_portfolio(
        RawOrigin::Signed(u.account.clone()).into(),
        portfolio_name.clone(),
    )
    .expect("Failed to generate portfolio");
    PortfolioId::user_portfolio(u.did, PortfolioNumber::from(portfolio_no))
}

fn populate_legs_for_instruction<T: Trait>(index: u32, legs: &mut Vec<Leg<T::Balance>>) {
    let ticker = Ticker::try_from(generate_ticker(index.into()).as_slice()).unwrap();
    legs.push(Leg {
        from: generate_portfolio::<T>("from_did", index + 500, None),
        to: generate_portfolio::<T>("to_did", index + 800, None),
        asset: ticker,
        amount: 100u32.into(),
    });
}

fn verify_add_instruction<T: Trait>(
    v_id: u64,
    s_type: SettlementType<T::BlockNumber>,
) -> DispatchResult {
    ensure!(
        Module::<T>::instruction_counter() == 2,
        "Instruction counter not increased"
    );
    let Instruction {
        instruction_id,
        venue_id,
        settlement_type,
        ..
    } = Module::<T>::instruction_details(Module::<T>::instruction_counter() - 1);
    assert_eq!(instruction_id, 1u64);
    assert_eq!(venue_id, v_id);
    assert_eq!(settlement_type, s_type);
    Ok(())
}

fn verify_add_and_affirm_instruction<T: Trait>(
    venue_id: u64,
    settlement_type: SettlementType<T::BlockNumber>,
    portfolios: Vec<PortfolioId>,
) -> DispatchResult {
    let _ = verify_add_instruction::<T>(venue_id, settlement_type)?;
    for portfolio_id in portfolios.iter() {
        ensure!(
            matches!(
                Module::<T>::affirms_received(1, portfolio_id),
                AffirmationStatus::Affirmed
            ),
            "Affirmation fails"
        );
    }
    Ok(())
}

fn emulate_add_instruction<T: Trait>(
    l: u32,
    create_portfolios: bool,
) -> Result<
    (
        Vec<Leg<T::Balance>>,
        u64,
        RawOrigin<T::AccountId>,
        IdentityId,
        Vec<PortfolioId>,
        Vec<PortfolioId>,
        T::AccountId,
    ),
    DispatchError,
> {
    let mut legs: Vec<Leg<T::Balance>> = Vec::with_capacity(l as usize);
    let mut sender_portfolios: Vec<PortfolioId> = Vec::with_capacity(l as usize);
    let mut receiver_portfolios: Vec<PortfolioId> = Vec::with_capacity(l as usize);
    // create venue
    let user = UserBuilder::<T>::default().generate_did().build("creator");
    let user_data = UserData::from(user);
    let venue_id = create_venue_::<T>(user_data.did, vec![user_data.account.clone()]);

    // Create legs vector.
    // Assuming the worst case where there is no dedup of `from` and `to` in the legs vector.
    if create_portfolios {
        // Assumption here is that instruction will never be executed as still there is one auth pending.
        for n in 0..l {
            setup_leg_and_portfolio::<T>(
                None,
                Some(user_data.clone()),
                n,
                &mut legs,
                &mut sender_portfolios,
                &mut receiver_portfolios,
            );
        }
    } else {
        for i in 1..l {
            populate_legs_for_instruction::<T>(i, &mut legs);
        }
    }
    <pallet_timestamp::Now<T>>::set(100000000u32.into());
    Ok((
        legs,
        venue_id,
        RawOrigin::Signed(user_data.account.clone()),
        user_data.did,
        sender_portfolios,
        receiver_portfolios,
        user_data.account,
    ))
}

fn emulate_portfolios<T: Trait>(
    sender: Option<UserData<T>>,
    receiver: Option<UserData<T>>,
    ticker: Ticker,
    index: u32,
    legs: &mut Vec<Leg<T::Balance>>,
    sender_portfolios: &mut Vec<PortfolioId>,
    receiver_portfolios: &mut Vec<PortfolioId>,
) {
    let transacted_amount = 500 * POLY;
    let sender_portfolio = generate_portfolio::<T>("", index + 500, sender);
    let receiver_portfolio = generate_portfolio::<T>("", index + 800, receiver);
    let _ = fund_portfolio::<T>(&sender_portfolio, &ticker, transacted_amount.into());
    sender_portfolios.push(sender_portfolio);
    receiver_portfolios.push(receiver_portfolio);
    legs.push(Leg {
        from: sender_portfolio,
        to: receiver_portfolio,
        asset: ticker,
        amount: transacted_amount.into(),
    })
}

// Generate signature.
fn get_encoded_signature<T: Trait>(signer: &User<T>, msg: &Receipt<T::Balance>) -> Vec<u8> {
    let raw_signature: [u8; 64] = signer.sign(&msg.encode()).expect("Data cannot be signed").0;
    let encoded = MultiSignature::from(Signature::from_raw(raw_signature)).encode();
    encoded
}

fn add_trusted_issuer<T: Trait>(
    origin: RawOrigin<T::AccountId>,
    ticker: Ticker,
    issuer: TrustedIssuer,
) {
    pallet_compliance_manager::Module::<T>::add_default_trusted_claim_issuer(
        origin.into(),
        ticker,
        issuer,
    )
    .expect("Default trusted claim issuer cannot be added");
}

pub fn setup_conditions<T: Trait>(
    count: u32,
    trusted_issuer: TrustedIssuer,
    dids: Vec<IdentityId>,
    ticker: Ticker,
) -> Vec<Condition> {
    let encoded_ticker = ticker.encode();
    (0..count)
        .map(|i| {
            let scope =
                Scope::Custom((encoded_ticker.clone(), vec![1; i.try_into().unwrap()]).encode());
            let claim = Claim::Jurisdiction(CountryCode::AF, scope);
            for did in &dids {
                identity::Module::<T>::base_add_claim(
                    did.clone(),
                    claim.clone(),
                    trusted_issuer.issuer,
                    None,
                );
            }
            Condition::new(
                ConditionType::IsPresent(claim),
                vec![trusted_issuer.clone()],
            )
        })
        .collect()
}

pub fn compliance_setup<T: Trait>(
    max_complexity: u32,
    ticker: Ticker,
    origin: RawOrigin<T::AccountId>,
    from_did: IdentityId,
    to_did: IdentityId,
    trusted_issuer: TrustedIssuer,
) {
    // Add investor uniqueness claim.
    <T as pallet_compliance_manager::Trait>::Asset::add_investor_uniqueness_claim(from_did, ticker);
    <T as pallet_compliance_manager::Trait>::Asset::add_investor_uniqueness_claim(to_did, ticker);
    // Add trusted issuer.
    add_trusted_issuer::<T>(origin.clone(), ticker, trusted_issuer.clone());

    let conditions = setup_conditions::<T>(
        max_complexity / 2,
        trusted_issuer,
        vec![from_did, to_did],
        ticker,
    );
    pallet_compliance_manager::Module::<T>::add_compliance_requirement(
        origin.clone().into(),
        ticker,
        conditions.clone(),
        conditions,
    )
    .expect("Failed to add the asset compliance");
}

fn setup_affirm_instruction<T: Trait>(
    l: u32,
) -> (
    Vec<PortfolioId>,
    UserData<T>,
    UserData<T>,
    Vec<Ticker>,
    Vec<Leg<T::Balance>>,
) {
    // create venue
    let from = UserBuilder::<T>::default().generate_did().build("creator");
    let venue_id = create_venue_::<T>(from.did(), vec![]);
    let settlement_type: SettlementType<T::BlockNumber> = SettlementType::SettleOnAffirmation;
    let to = UserBuilder::<T>::default().generate_did().build("receiver");
    let mut portfolios_from: Vec<PortfolioId> = Vec::with_capacity(l as usize);
    let mut portfolios_to: Vec<PortfolioId> = Vec::with_capacity(l as usize);
    let mut legs: Vec<Leg<T::Balance>> = Vec::with_capacity(l as usize);
    let mut tickers = Vec::with_capacity(l as usize);
    let from_data = UserData::from(from.clone());
    let to_data = UserData::from(to);

    for n in 0..l {
        tickers.push(make_asset::<T>(&from, Some(generate_ticker(n as u64 + 1))));
        emulate_portfolios::<T>(
            Some(from_data.clone()),
            Some(to_data.clone()),
            tickers[n as usize],
            l,
            &mut legs,
            &mut portfolios_from,
            &mut portfolios_to,
        );
    }
    Module::<T>::add_and_affirm_instruction(
        (RawOrigin::Signed(from_data.account.clone())).into(),
        venue_id,
        settlement_type,
        None,
        None,
        legs.clone(),
        portfolios_from,
    )
    .expect("Unable to add and affirm the instruction");

    (portfolios_to, from_data, to_data, tickers, legs)
}

#[allow(dead_code)]
fn add_smart_extension_to_ticker<T: Trait>(
    code_hash: <T::Hashing as Hash>::Output,
    origin: RawOrigin<T::AccountId>,
    account: T::AccountId,
    ticker: Ticker,
) {
    let data = vec![
        209, 131, 81, 43, 160, 134, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
    ]; // Allow 100% as percentage ownership and allow primary issuance.
    <polymesh_contracts::Module<T>>::instantiate(
        origin.clone().into(),
        0u32.into(),
        Weight::max_value(),
        code_hash,
        data.clone(),
        0u32.into(),
    )
    .expect("Settlement: Failed to instantiate the contract");
    let extension_id =
        T::DetermineContractAddress::contract_address_for(&code_hash, &data, &account);
    let extension_details = SmartExtension {
        extension_type: SmartExtensionType::TransferManager,
        extension_name: b"PTM".into(),
        extension_id: extension_id.clone(),
        is_archive: false,
    };
    <pallet_asset::Module<T>>::add_extension(origin.into(), ticker, extension_details)
        .expect("Settlement: Fail to add the smart extension to a given asset");
}

fn create_receipt_details<T: Trait>(
    index: u32,
    leg: Leg<T::Balance>,
) -> ReceiptDetails<T::AccountId, T::OffChainSignature> {
    let User {
        account, secret, ..
    } = UserBuilder::<T>::default().build("creator");
    let msg = Receipt {
        receipt_uid: index as u64,
        from: leg.from,
        to: leg.to,
        asset: leg.asset,
        amount: leg.amount,
    };
    let origin = RawOrigin::Signed(account.clone());
    let creator = User {
        account: account.clone(),
        secret,
        origin,
        uid: None,
        did: None,
    };
    let encoded = get_encoded_signature::<T>(&creator, &msg);
    let signature = T::OffChainSignature::decode(&mut &encoded[..])
        .expect("OffChainSignature cannot be decoded from a MultiSignature");
    // Receipt details.
    ReceiptDetails {
        receipt_uid: index as u64,
        leg_id: index as u64,
        signer: account,
        signature,
        metadata: ReceiptMetadata::from(vec![b'D'; 10 as usize].as_slice()),
    }
}

pub fn add_transfer_managers<T: Trait>(
    ticker: Ticker,
    origin: RawOrigin<T::AccountId>,
    exempted_entity: IdentityId,
    count: u32,
) {
    for i in 0..count {
        let tm = TransferManager::CountTransferManager(i.into());
        // Add Transfer manager
        <pallet_statistics::Module<T>>::add_transfer_manager(
            origin.clone().into(),
            ticker,
            tm.clone(),
        )
        .expect("failed to add transfer manager");
        // Exempt the user.
        <pallet_statistics::Module<T>>::add_exempted_entities(
            origin.clone().into(),
            ticker,
            tm,
            vec![exempted_entity],
        )
        .expect("failed to add exempted entities");
    }
    <pallet_statistics::Module<T>>::set_investor_count(&ticker, count.into());
}

benchmarks! {
    _{}

    create_venue {
        // Variations for the venue_details length.
        let d in 1 .. MAX_VENUE_DETAILS_LENGTH;
        // Variations for the no. of signers allowed.
        let s in 0 .. MAX_SIGNERS_ALLOWED;
        let mut signers = Vec::with_capacity(s as usize);
        let User {origin, did, .. } = UserBuilder::<T>::default().generate_did().build("caller");
        let venue_details = VenueDetails::from(vec![b'D'; d as usize].as_slice());
        let venue_type = VenueType::Distribution;
        // Create signers vector.
        for signer in 0 .. s {
            signers.push(UserBuilder::<T>::default().generate_did().seed(signer).build("signers").account());
        }
    }: _(origin, venue_details, signers, venue_type)
    verify {
        ensure!(matches!(Module::<T>::venue_counter(), 2), "Invalid venue counter");
        ensure!(matches!(Module::<T>::user_venues(did.unwrap()).into_iter().last(), Some(1)), "Invalid venue id");
        ensure!(Module::<T>::venue_info(1).is_some(), "Incorrect venue info set");
    }


    update_venue {
        // Variations for the venue_details length.
        let d in 1 .. MAX_VENUE_DETAILS_LENGTH;
        let venue_details = VenueDetails::from(vec![b'D'; d as usize].as_slice());
        // Venue type.
        let venue_type = VenueType::Sto;
        let User {account, origin, did, ..} = UserBuilder::<T>::default().generate_did().build("creator");
        // create venue
        let venue_id = create_venue_::<T>(did.unwrap(), vec![]);
    }: _(origin, venue_id, Some(venue_details), Some(venue_type))
    verify {
        let updated_venue_details = Module::<T>::venue_info(1).unwrap();
        ensure!(matches!(updated_venue_details.venue_type, VenueType::Sto), "Incorrect venue type value");
        ensure!(matches!(updated_venue_details.details, venue_details), "Incorrect venue details");
    }


    add_instruction {

        let l in 1 .. MAX_LEGS_IN_INSTRUCTION; // Variation for the MAX leg count.
        // Define settlement type
        let settlement_type = SettlementType::SettleOnAffirmation;
        // Emulate the add instruction and get all the necessary arguments.
        let (legs, venue_id, origin, did , _, _, _ ) = emulate_add_instruction::<T>(l, false)?;

    }: _(origin, venue_id, settlement_type, Some(99999999u32.into()), Some(99999999u32.into()), legs)
    verify {
        verify_add_instruction::<T>(venue_id, settlement_type)?;
    }


    add_instruction_with_settle_on_block_type {
        let l in 1 .. MAX_LEGS_IN_INSTRUCTION; // Variation for the MAX leg count.
        // Define settlement type
        let settlement_type = SettlementType::SettleOnBlock(100u32.into());
        set_block_number::<T>(50);

        // Emulate the add instruction and get all the necessary arguments.
        let (legs, venue_id, origin, did , _, _, _ ) = emulate_add_instruction::<T>(l, false)?;

    }: add_instruction(origin, venue_id, settlement_type, Some(99999999u32.into()), Some(99999999u32.into()), legs)
    verify {
        verify_add_instruction::<T>(venue_id, settlement_type)?;
    }


    add_and_affirm_instruction {
        let l in 1 .. MAX_LEGS_IN_INSTRUCTION;
        // Define settlement type
        let settlement_type = SettlementType::SettleOnAffirmation;
        // Emulate the add instruction and get all the necessary arguments.
        let (legs, venue_id, origin, did , portfolios, _, _) = emulate_add_instruction::<T>(l, true)?;
        let s_portfolios = portfolios.clone();
    }: _(origin, venue_id, settlement_type, Some(99999999u32.into()), Some(99999999u32.into()), legs, s_portfolios)
    verify {
        verify_add_and_affirm_instruction::<T>(venue_id, settlement_type, portfolios)?;
    }


    add_and_affirm_instruction_with_settle_on_block_type {
        let l in 1 .. MAX_LEGS_IN_INSTRUCTION;
        // Define settlement type.
        let settlement_type = SettlementType::SettleOnBlock(100u32.into());
        set_block_number::<T>(50);
        // Emulate the add instruction and get all the necessary arguments.
        let (legs, venue_id, origin, did , portfolios, _, _) = emulate_add_instruction::<T>(l, true)?;
        let s_portfolios = portfolios.clone();
    }: add_and_affirm_instruction(origin, venue_id, settlement_type, Some(99999999u32.into()), Some(99999999u32.into()), legs, s_portfolios)
    verify {
        verify_add_and_affirm_instruction::<T>(venue_id, settlement_type, portfolios)?;
    }


    set_venue_filtering {
        // Constant time function. It is only for allow venue filtering.
        let User {account, origin, did, ..} = UserBuilder::<T>::default().generate_did().build("creator");
        let ticker = create_asset_::<T>(did.unwrap())?;
    }: _(origin, ticker, true)
    verify {
        ensure!(Module::<T>::venue_filtering(ticker), "Fail: set_venue_filtering failed");
    }


    set_venue_filtering_disallow {
        // Constant time function. It is only for disallowing venue filtering.
        let User {account, origin, did, ..} = UserBuilder::<T>::default().generate_did().build("creator");
        let ticker = create_asset_::<T>(did.unwrap())?;
    }: set_venue_filtering(origin, ticker, false)
    verify {
        ensure!(!Module::<T>::venue_filtering(ticker), "Fail: set_venue_filtering failed");
    }


    allow_venues {
        // Count of venue is variant for this dispatchable.
        let v in 0 .. MAX_VENUE_ALLOWED;
        let User {account, origin, did, .. } = UserBuilder::<T>::default().generate_did().build("creator");
        let ticker = create_asset_::<T>(did.unwrap())?;
        let mut venues: Vec<u64> = Vec::new();
        for i in 0 .. v {
            venues.push(i.into());
        }
        let s_venues = venues.clone();
    }: _(origin, ticker, s_venues)
    verify {
        for v in venues.iter() {
            ensure!(Module::<T>::venue_allow_list(ticker, v), "Fail: allow_venue dispatch");
        }
    }


    disallow_venues {
        // Count of venue is variant for this dispatchable.
        let v in 0 .. MAX_VENUE_ALLOWED;
        let User {account, origin, did, .. } = UserBuilder::<T>::default().generate_did().build("creator");
        let ticker = create_asset_::<T>(did.unwrap())?;
        let mut venues: Vec<u64> = Vec::new();
        for i in 0 .. v {
            venues.push(i.into());
        }
        let s_venues = venues.clone();
    }: _(origin, ticker, s_venues)
    verify {
        for v in venues.iter() {
            ensure!(!Module::<T>::venue_allow_list(ticker, v), "Fail: allow_venue dispatch");
        }
    }


    withdraw_affirmation {
        // Below setup is for the onchain affirmation.

        let l in 0 .. MAX_LEGS_IN_INSTRUCTION;
        // Emulate the add instruction and get all the necessary arguments.
        let (legs, venue_id, origin, did , portfolios, _, _) = emulate_add_instruction::<T>(l, true)?;
        // Add instruction
        Module::<T>::base_add_instruction(did, venue_id, SettlementType::SettleOnAffirmation, None, None, legs.clone())?;
        let instruction_id: u64 = 1;
        // Affirm an instruction
        let portfolios_set = portfolios.clone().into_iter().collect::<BTreeSet<_>>();
        Module::<T>::unsafe_affirm_instruction(did, instruction_id, portfolios_set, l.into(), None)?;

    }: _(origin, instruction_id, portfolios, l.into())
    verify {
        for (idx, leg) in legs.iter().enumerate() {
            ensure!(matches!(Module::<T>::instruction_leg_status(instruction_id, u64::try_from(idx).unwrap_or_default()), LegStatus::PendingTokenLock), "Fail: withdraw affirmation dispatch");
        }
    }


    withdraw_affirmation_with_receipt {
        // Below setup is for the receipt based affirmation

        let l in 0 .. MAX_LEGS_IN_INSTRUCTION;
        // Emulate the add instruction and get all the necessary arguments.
        let (legs, venue_id, origin, did , portfolios, _, account_id) = emulate_add_instruction::<T>(l, true)?;
        // Add instruction
        Module::<T>::base_add_instruction(did, venue_id, SettlementType::SettleOnAffirmation, None, None, legs.clone())?;
        let instruction_id: u64 = 1;
        // Affirm an instruction
        portfolios.clone().into_iter().for_each(|p| {
            set_user_affirmations(instruction_id, p, AffirmationStatus::Affirmed);
        });
        for (idx, _) in legs.clone().iter().enumerate() {
            let leg_id = u64::try_from(idx).unwrap_or_default();
            // use leg_id for the receipt_uid as well.
            set_instruction_leg_status_to_skipped::<T>(instruction_id, leg_id, account_id.clone(), leg_id);
        }
    }: withdraw_affirmation(origin, instruction_id, portfolios, l.into())
    verify {
        for (idx, leg) in legs.iter().enumerate() {
            ensure!(matches!(Module::<T>::instruction_leg_status(instruction_id, u64::try_from(idx).unwrap_or_default()), LegStatus::PendingTokenLock), "Fail: withdraw affirmation dispatch");
        }
    }


    unclaim_receipt {
        // There is no catalyst in this dispatchable, It will be time constant always.
        // Emulate the add instruction and get all the necessary arguments.
        let (legs, venue_id, origin, did, _, _, account_id) = emulate_add_instruction::<T>(1, true)?;
        // Add instruction
        Module::<T>::base_add_instruction(did, venue_id, SettlementType::SettleOnAffirmation, None, None, legs.clone())?;
        let instruction_id = 1;
        let leg_id = 0;

        set_instruction_leg_status_to_skipped::<T>(instruction_id, leg_id, account_id.clone(), 0);
    }: _(origin, instruction_id, leg_id)
    verify {
        ensure!(matches!(Module::<T>::instruction_leg_status(instruction_id, leg_id), LegStatus::ExecutionPending), "Fail: unclaim_receipt dispatch");
        ensure!(!Module::<T>::receipts_used(&account_id, 0), "Fail: Receipt status didn't get update");
    }


    reject_instruction {
        // At least one portfolio needed
        let l in 1 .. MAX_LEGS_IN_INSTRUCTION;
        // Emulate the add instruction and get all the necessary arguments.
        let (legs, venue_id, origin, did , portfolios, _, account_id) = emulate_add_instruction::<T>(l, true)?;
        // Add and affirm instruction.
        Module::<T>::add_and_affirm_instruction((origin.clone()).into(), venue_id, SettlementType::SettleOnAffirmation, None, None, legs, portfolios.clone()).expect("Unable to add and affirm the instruction");
        let instruction_id: u64 = 1;
        let s_portfolios = portfolios.clone();
    }: _(origin, instruction_id, s_portfolios, l.into())
    verify {
        for p in portfolios.iter() {
            ensure!(Module::<T>::affirms_received(instruction_id, p) == AffirmationStatus::Rejected, "Settlement: Failed to reject instruction");
        }
    }


    reject_instruction_with_no_pre_affirmations {
        // At least one portfolio needed
        let l in 1 .. MAX_LEGS_IN_INSTRUCTION;
        // Emulate the add instruction and get all the necessary arguments.
        let (legs, venue_id, origin, did , portfolios, _, account_id) = emulate_add_instruction::<T>(l, true)?;
        // Add instruction
        Module::<T>::base_add_instruction(did, venue_id, SettlementType::SettleOnAffirmation, None, None, legs.clone())?;
        let instruction_id: u64 = 1;
        let s_portfolios = portfolios.clone();
    }: reject_instruction(origin, instruction_id, s_portfolios, l.into())
    verify {
        for p in portfolios.iter() {
            ensure!(Module::<T>::affirms_received(instruction_id, p) == AffirmationStatus::Rejected, "Settlement: Failed to reject instruction");
        }
    }


    affirm_instruction {
        let l in 0 .. MAX_LEGS_IN_INSTRUCTION; // At least 2 legs needed to achieve worst case.
        let (portfolios_to, _, to, _, _) = setup_affirm_instruction::<T>(l);
        let instruction_id = 1; // It will always be `1` as we know there is no other instruction in the storage yet.
        let to_portfolios = portfolios_to.clone();
        let legs_count = (l / 2).into();
    }: _(RawOrigin::Signed(to.account), instruction_id, to_portfolios, legs_count)
    verify {
        for p in portfolios_to.iter() {
            ensure!(Module::<T>::affirms_received(instruction_id, p) == AffirmationStatus::Affirmed, "Settlement: Failed to affirm instruction");
        }
    }


    claim_receipt {
        // There is no catalyst in this dispatchable, It will always be time constant.
        // Emulate the add instruction and get all the necessary arguments.
        let (legs, venue_id, origin, did , s_portfolios, r_portfolios, account_id) = emulate_add_instruction::<T>(1, true)?;
        // Add instruction
        Module::<T>::base_add_instruction(did, venue_id, SettlementType::SettleOnAffirmation, None, None, legs.clone())?;
        let instruction_id = 1;
        let ticker = Ticker::try_from(generate_ticker(1u64).as_slice()).unwrap();
        let receipt = create_receipt_details::<T>(0, legs.first().unwrap().clone());
        let leg_id = 0;
        let amount = 100u128;
        // Some manual setup to support the extrinsic.
        set_instruction_leg_status_to_pending::<T>(instruction_id, leg_id);
        T::Portfolio::lock_tokens(s_portfolios.first().unwrap(), &ticker, &amount.into())?;
        let s_receipt = receipt.clone();
    }: _(origin, instruction_id, s_receipt)
    verify {
        ensure!(Module::<T>::instruction_leg_status(instruction_id, leg_id) ==  LegStatus::ExecutionToBeSkipped(
            receipt.signer,
            receipt.receipt_uid,
        ), "Settlement: Fail to unclaim the receipt");
    }


    affirm_with_receipts {
        // Catalyst here is the length of receipts vector.
        let r in 1 .. MAX_LEGS_IN_INSTRUCTION;
        // Emulate the add instruction and get all the necessary arguments.
        let (legs, venue_id, origin, did , s_portfolios, r_portfolios, account_id) = emulate_add_instruction::<T>(r, true)?;
        // Add instruction
        Module::<T>::base_add_instruction(did, venue_id, SettlementType::SettleOnAffirmation, None, None, legs.clone())?;
        let instruction_id = 1;
        let mut receipt_details = Vec::with_capacity(r as usize);
        legs.clone().into_iter().enumerate().for_each(|(idx, l)| {
            receipt_details.push(create_receipt_details::<T>(idx as u32, l));
        });
        let s_receipt_details = receipt_details.clone();
    }: _(origin, instruction_id, s_receipt_details, s_portfolios, r)
    verify {
        for (i, receipt) in receipt_details.iter().enumerate() {
            ensure!(Module::<T>::instruction_leg_status(instruction_id, i as u64) ==  LegStatus::ExecutionToBeSkipped(
                receipt.signer.clone(),
                receipt.receipt_uid,
            ), "Settlement: Fail to affirm with receipts");
        }
    }

    change_receipt_validity {
        let signer = user::<T>("signer", 0);
    }: _(signer.origin(), 0, false)
    verify {
        ensure!(Module::<T>::receipts_used(&signer.account(), 0), "Settlement: change_receipt_validity didn't work");
    }

    execute_scheduled_instruction {
        // This dispatch execute an instruction.
        //
        // Worst case scenarios.
        // 1. Create maximum legs and both traded assets are different assets/securities.
        // 2. Assets have maximum compliance restriction complexity.
        // 3. Assets have maximum no. of TMs.

        let l in 0 .. MAX_LEGS_IN_INSTRUCTION;
        let s = T::MaxTransferManagersPerAsset::get() as u32;
        let c = T::MaxConditionComplexity::get() as u32;

        // Setup affirm instruction (One party (i.e from) already affirms the instruction)
        let (portfolios_to, from, to, tickers, legs) = setup_affirm_instruction::<T>(l);
        // Keep the portfolio asset balance before the instruction execution to verify it later.
        let legs_count: u32 = legs.len().try_into().unwrap();
        let first_leg = legs.into_iter().nth(0).unwrap_or_default();
        let before_transfer_balance = <PortfolioAssetBalances<T>>::get(first_leg.from, first_leg.asset);
        // It always be one as no other instruction is already scheduled.
        let instruction_id = 1;
        let origin = RawOrigin::Root;
        let from_origin = RawOrigin::Signed(from.account.clone());
        let to_origin = RawOrigin::Signed(to.account.clone());
        // Do another affirmations that lead to scheduling an instruction.
        Module::<T>::affirm_instruction((to_origin.clone()).into(), instruction_id, portfolios_to, (legs_count / 2).into()).expect("Settlement: Failed to affirm instruction");
        // Create trusted issuer for both the ticker
        let t_issuer = UserBuilder::<T>::default().generate_did().build("TrustedClaimIssuer");
        let trusted_issuer = TrustedIssuer::from(t_issuer.did());

        // Need to provide the Investor uniqueness claim for both sender and receiver,
        // for both assets also add the compliance rules as per the `MaxConditionComplexity`.
        for ticker in tickers {
            compliance_setup::<T>(c, ticker, from_origin.clone(), from.did, to.did, trusted_issuer.clone());
            add_transfer_managers::<T>(ticker, from_origin.clone(), from.did, s);
        }

        // -------- Commented the smart extension integration ----------------
        // let code_hash = emulate_blueprint_in_storage::<T>(0, from_origin.clone(), "ptm")?;
        // for i in 0 .. s {
        //     add_smart_extension_to_ticker::<T>(code_hash, from_origin.clone(), from.account.clone(), from_ticker);
        //     add_smart_extension_to_ticker::<T>(code_hash, to_origin.clone(), to.account.clone(), to_ticker);
        // }
    }: _(origin, instruction_id, l)
    verify {
        // Ensure that any one leg processed through that give sufficient evidence of successful execution of instruction.
        let after_transfer_balance = <PortfolioAssetBalances<T>>::get(first_leg.from, first_leg.asset);
        let traded_amount = before_transfer_balance - after_transfer_balance;
        let expected_transfer_amount = first_leg.amount;
        ensure!(matches!(traded_amount, expected_transfer_amount),"Settlement: Failed to execute the instruction");
    }
}
