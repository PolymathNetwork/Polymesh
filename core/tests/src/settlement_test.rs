use super::{
    storage::{
        default_portfolio_vec, make_account, make_account_without_cdd,
        provide_scope_claim_to_multiple_parties, register_keyring_account, user_portfolio_vec,
        TestStorage,
    },
    ExtBuilder,
};
use codec::Encode;
use frame_support::{
    assert_noop, assert_ok, traits::OnInitialize, IterableStorageDoubleMap, StorageMap,
};
use pallet_asset as asset;
use pallet_balances as balances;
use pallet_compliance_manager as compliance_manager;
use pallet_identity as identity;
use pallet_portfolio::MovePortfolioItem;
use pallet_scheduler as scheduler;
use pallet_settlement::{
    self as settlement, AffirmationStatus, Instruction, InstructionStatus, Leg, LegStatus, Receipt,
    ReceiptDetails, ReceiptMetadata, SettlementType, VenueDetails, VenueType,
};
use polymesh_common_utilities::constants::ERC1400_TRANSFER_SUCCESS;
use polymesh_primitives::{
    asset::AssetType, AuthorizationData, Claim, Condition, ConditionType, IdentityId, PortfolioId,
    PortfolioName, Signatory, Ticker,
};
use rand::{prelude::*, thread_rng};
use sp_core::sr25519::Public;
use sp_runtime::AnySignature;
use std::collections::HashMap;
use std::convert::TryFrom;
use test_client::AccountKeyring;

type Identity = identity::Module<TestStorage>;
type Balances = balances::Module<TestStorage>;
type Asset = asset::Module<TestStorage>;
type Portfolio = pallet_portfolio::Module<TestStorage>;
type PortfolioError = pallet_portfolio::Error<TestStorage>;
type Timestamp = pallet_timestamp::Module<TestStorage>;
type ComplianceManager = compliance_manager::Module<TestStorage>;
type AssetError = asset::Error<TestStorage>;
type OffChainSignature = AnySignature;
type Origin = <TestStorage as frame_system::Trait>::Origin;
type DidRecords = identity::DidRecords<TestStorage>;
type Settlement = settlement::Module<TestStorage>;
type System = frame_system::Module<TestStorage>;
type Error = settlement::Error<TestStorage>;
type Scheduler = scheduler::Module<TestStorage>;

macro_rules! assert_add_claim {
    ($signer:expr, $target:expr, $claim:expr) => {
        assert_ok!(Identity::add_claim($signer, $target, $claim, None,));
    };
}

macro_rules! assert_instruction_execution {
    ($assert:ident, $x:expr, $y:expr $(,)?) => {
        next_block();
        $assert!($x, $y);
    };
}

macro_rules! assert_affirm_instruction {
    ($signer:expr, $instruction_counter:expr, $did:expr, $count:expr) => {
        assert_ok!(Settlement::affirm_instruction(
            $signer,
            $instruction_counter,
            default_portfolio_vec($did),
            $count
        ));
    };
}

macro_rules! assert_affirm_instruction_with_one_leg {
    ($signer:expr, $instruction_counter:expr, $did:expr) => {
        assert_affirm_instruction!($signer, $instruction_counter, $did, 1);
    };
}

macro_rules! assert_affirm_instruction_with_zero_leg {
    ($signer:expr, $instruction_counter:expr, $did:expr) => {
        assert_affirm_instruction!($signer, $instruction_counter, $did, 0);
    };
}

fn init(token_name: &[u8], ticker: Ticker, keyring: Public) -> u64 {
    create_token(token_name, ticker, keyring);
    let venue_counter = Settlement::venue_counter();
    assert_ok!(Settlement::create_venue(
        Origin::signed(keyring),
        VenueDetails::default(),
        vec![keyring],
        VenueType::Other
    ));
    venue_counter
}

fn create_token(token_name: &[u8], ticker: Ticker, keyring: Public) {
    assert_ok!(Asset::create_asset(
        Origin::signed(keyring),
        token_name.into(),
        ticker,
        100_000,
        true,
        AssetType::default(),
        vec![],
        None,
    ));
    assert_ok!(ComplianceManager::add_compliance_requirement(
        Origin::signed(keyring),
        ticker,
        vec![],
        vec![]
    ));
}

pub fn next_block() {
    let block_number = System::block_number() + 1;
    set_current_block_number(block_number);
    let _ = Scheduler::on_initialize(block_number);
}

pub fn set_current_block_number(block: u64) {
    System::set_block_number(block);
}

#[test]
fn venue_registration() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_signed = Origin::signed(AccountKeyring::Alice.public());
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let venue_counter = Settlement::venue_counter();
        assert_ok!(Settlement::create_venue(
            alice_signed.clone(),
            VenueDetails::default(),
            vec![AccountKeyring::Alice.public(), AccountKeyring::Bob.public()],
            VenueType::Exchange
        ));
        let venue_info = Settlement::venue_info(venue_counter).unwrap();
        assert_eq!(Settlement::venue_counter(), venue_counter + 1);
        assert_eq!(Settlement::user_venues(alice_did), [venue_counter]);
        assert_eq!(venue_info.creator, alice_did);
        assert_eq!(venue_info.instructions.len(), 0);
        assert_eq!(venue_info.details, VenueDetails::default());
        assert_eq!(venue_info.venue_type, VenueType::Exchange);
        assert_eq!(
            Settlement::venue_signers(venue_counter, AccountKeyring::Alice.public()),
            true
        );
        assert_eq!(
            Settlement::venue_signers(venue_counter, AccountKeyring::Bob.public()),
            true
        );
        assert_eq!(
            Settlement::venue_signers(venue_counter, AccountKeyring::Charlie.public()),
            false
        );

        // Creating a second venue
        assert_ok!(Settlement::create_venue(
            alice_signed.clone(),
            VenueDetails::default(),
            vec![AccountKeyring::Alice.public(), AccountKeyring::Bob.public()],
            VenueType::Exchange
        ));
        assert_eq!(
            Settlement::user_venues(alice_did),
            [venue_counter, venue_counter + 1]
        );

        // Editing venue details
        assert_ok!(Settlement::update_venue(
            alice_signed,
            venue_counter,
            Some([0x01].into()),
            None
        ));
        let venue_info = Settlement::venue_info(venue_counter).unwrap();
        assert_eq!(venue_info.creator, alice_did);
        assert_eq!(venue_info.instructions.len(), 0);
        assert_eq!(venue_info.details, [0x01].into());
        assert_eq!(venue_info.venue_type, VenueType::Exchange);
    });
}

#[test]
fn basic_settlement() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let alice_signed = Origin::signed(AccountKeyring::Alice.public());
            let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
            let bob_signed = Origin::signed(AccountKeyring::Bob.public());
            let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
            let token_name = b"ACME";
            let ticker = Ticker::try_from(&token_name[..]).unwrap();
            let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
            let instruction_counter = Settlement::instruction_counter();
            let alice_init_balance = Asset::balance_of(&ticker, alice_did);
            let bob_init_balance = Asset::balance_of(&ticker, bob_did);
            let amount = 100u128;
            let eve = AccountKeyring::Eve.public();

            // Provide scope claim to sender and receiver of the transaction.
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker, eve);

            assert_ok!(Settlement::add_instruction(
                alice_signed.clone(),
                venue_counter,
                SettlementType::SettleOnAffirmation,
                None,
                None,
                vec![Leg {
                    from: PortfolioId::default_portfolio(alice_did),
                    to: PortfolioId::default_portfolio(bob_did),
                    asset: ticker,
                    amount: amount
                }]
            ));
            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);

            assert_affirm_instruction_with_one_leg!(
                alice_signed.clone(),
                instruction_counter,
                alice_did
            );

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            set_current_block_number(5);
            // Instruction get scheduled to next block.
            assert_affirm_instruction_with_zero_leg!(
                bob_signed.clone(),
                instruction_counter,
                bob_did
            );

            // Advances the block no. to execute the instruction.
            let new_balance = alice_init_balance - amount;
            assert_instruction_execution!(
                assert_eq,
                Asset::balance_of(&ticker, alice_did),
                new_balance
            );
            assert_eq!(
                Asset::balance_of(&ticker, bob_did),
                bob_init_balance + amount
            );
        });
}

#[test]
fn create_and_affirm_instruction() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let alice_signed = Origin::signed(AccountKeyring::Alice.public());
            let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
            let bob_signed = Origin::signed(AccountKeyring::Bob.public());
            let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
            let token_name = b"ACME";
            let ticker = Ticker::try_from(&token_name[..]).unwrap();
            let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
            let instruction_counter = Settlement::instruction_counter();
            let alice_init_balance = Asset::balance_of(&ticker, alice_did);
            let bob_init_balance = Asset::balance_of(&ticker, bob_did);
            let amount = 100u128;
            let eve = AccountKeyring::Eve.public();

            // Provide scope claim to both the parties of the transaction.
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker, eve);

            let add_and_affirm_tx = |affirm_from_portfolio| {
                Settlement::add_and_affirm_instruction(
                    alice_signed.clone(),
                    venue_counter,
                    SettlementType::SettleOnAffirmation,
                    None,
                    None,
                    vec![Leg {
                        from: PortfolioId::default_portfolio(alice_did),
                        to: PortfolioId::default_portfolio(bob_did),
                        asset: ticker,
                        amount: amount,
                    }],
                    affirm_from_portfolio,
                )
            };

            // If affirmation fails, the instruction should be rolled back.
            // i.e. this tx should be a no-op.
            assert_noop!(
                add_and_affirm_tx(user_portfolio_vec(alice_did, 1u64.into())),
                Error::UnexpectedAffirmationStatus
            );

            assert_ok!(add_and_affirm_tx(default_portfolio_vec(alice_did)));

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);

            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );
            set_current_block_number(5);

            assert_affirm_instruction_with_zero_leg!(
                bob_signed.clone(),
                instruction_counter,
                bob_did
            );

            // Advances the block no.
            assert_instruction_execution!(
                assert_eq,
                Asset::balance_of(&ticker, alice_did),
                alice_init_balance - amount
            );

            assert_eq!(
                Asset::balance_of(&ticker, bob_did),
                bob_init_balance + amount
            );
        });
}

#[test]
fn overdraft_failure() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_signed = Origin::signed(AccountKeyring::Alice.public());
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let _bob_signed = Origin::signed(AccountKeyring::Bob.public());
        let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
        let token_name = b"ACME";
        let ticker = Ticker::try_from(&token_name[..]).unwrap();
        let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
        let instruction_counter = Settlement::instruction_counter();
        let alice_init_balance = Asset::balance_of(&ticker, alice_did);
        let bob_init_balance = Asset::balance_of(&ticker, bob_did);
        let amount = 100_000_000u128;
        assert_ok!(Settlement::add_instruction(
            alice_signed.clone(),
            venue_counter,
            SettlementType::SettleOnAffirmation,
            None,
            None,
            vec![Leg {
                from: PortfolioId::default_portfolio(alice_did),
                to: PortfolioId::default_portfolio(bob_did),
                asset: ticker,
                amount: amount
            }]
        ));
        assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
        assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
        assert_noop!(
            Settlement::affirm_instruction(
                alice_signed.clone(),
                instruction_counter,
                default_portfolio_vec(alice_did),
                1
            ),
            Error::FailedToLockTokens
        );

        assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
        assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
    });
}

#[test]
fn token_swap() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let alice_signed = Origin::signed(AccountKeyring::Alice.public());
            let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
            let bob_signed = Origin::signed(AccountKeyring::Bob.public());
            let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
            let token_name = b"ACME";
            let ticker = Ticker::try_from(&token_name[..]).unwrap();
            let token_name2 = b"ACME2";
            let ticker2 = Ticker::try_from(&token_name2[..]).unwrap();
            let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
            let eve = AccountKeyring::Eve.public();
            init(token_name2, ticker2, AccountKeyring::Bob.public());

            let instruction_counter = Settlement::instruction_counter();
            let alice_init_balance = Asset::balance_of(&ticker, alice_did);
            let bob_init_balance = Asset::balance_of(&ticker, bob_did);
            let alice_init_balance2 = Asset::balance_of(&ticker2, alice_did);
            let bob_init_balance2 = Asset::balance_of(&ticker2, bob_did);

            let amount = 100u128;
            let legs = vec![
                Leg {
                    from: PortfolioId::default_portfolio(alice_did),
                    to: PortfolioId::default_portfolio(bob_did),
                    asset: ticker,
                    amount: amount,
                },
                Leg {
                    from: PortfolioId::default_portfolio(bob_did),
                    to: PortfolioId::default_portfolio(alice_did),
                    asset: ticker2,
                    amount: amount,
                },
            ];

            assert_ok!(Settlement::add_instruction(
                alice_signed.clone(),
                venue_counter,
                SettlementType::SettleOnAffirmation,
                None,
                None,
                legs.clone()
            ));

            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );

            for i in 0..legs.len() {
                assert_eq!(
                    Settlement::instruction_legs(
                        instruction_counter,
                        u64::try_from(i).unwrap_or_default()
                    ),
                    legs[i]
                );
            }

            let instruction_details = Instruction {
                instruction_id: instruction_counter,
                venue_id: venue_counter,
                status: InstructionStatus::Pending,
                settlement_type: SettlementType::SettleOnAffirmation,
                created_at: Some(Timestamp::get()),
                trade_date: None,
                value_date: None,
            };
            assert_eq!(
                Settlement::instruction_details(instruction_counter),
                instruction_details
            );
            assert_eq!(
                Settlement::instruction_affirms_pending(instruction_counter),
                2
            );
            assert_eq!(
                Settlement::venue_info(venue_counter).unwrap().instructions,
                vec![instruction_counter]
            );

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
            assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

            // Provide scope claim to parties involved in a instruction.
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker, eve);
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker2, eve);

            assert_affirm_instruction_with_one_leg!(
                alice_signed.clone(),
                instruction_counter,
                alice_did
            );

            assert_eq!(
                Settlement::instruction_affirms_pending(instruction_counter),
                1
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(alice_did)
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(bob_did)
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 0),
                LegStatus::ExecutionPending
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 1),
                LegStatus::PendingTokenLock
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                amount
            );

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
            assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

            assert_ok!(Settlement::withdraw_affirmation(
                alice_signed.clone(),
                instruction_counter,
                default_portfolio_vec(alice_did),
                1
            ));

            assert_eq!(
                Settlement::instruction_affirms_pending(instruction_counter),
                2
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(alice_did)
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(bob_did)
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 0),
                LegStatus::PendingTokenLock
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 1),
                LegStatus::PendingTokenLock
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                0
            );
            assert_affirm_instruction_with_one_leg!(
                alice_signed.clone(),
                instruction_counter,
                alice_did
            );

            assert_eq!(
                Settlement::instruction_affirms_pending(instruction_counter),
                1
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(alice_did)
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(bob_did)
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 0),
                LegStatus::ExecutionPending
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 1),
                LegStatus::PendingTokenLock
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                amount
            );

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
            assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);
            set_current_block_number(500);

            assert_affirm_instruction_with_one_leg!(
                bob_signed.clone(),
                instruction_counter,
                bob_did
            );

            assert_instruction_execution!(
                assert_eq,
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                0
            );
            assert_eq!(
                Asset::balance_of(&ticker, alice_did),
                alice_init_balance - amount
            );
            assert_eq!(
                Asset::balance_of(&ticker, bob_did),
                bob_init_balance + amount
            );
            assert_eq!(
                Asset::balance_of(&ticker2, alice_did),
                alice_init_balance2 + amount
            );
            assert_eq!(
                Asset::balance_of(&ticker2, bob_did),
                bob_init_balance2 - amount
            );
        });
}

#[test]
fn claiming_receipt() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let alice_signed = Origin::signed(AccountKeyring::Alice.public());
            let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
            let bob_signed = Origin::signed(AccountKeyring::Bob.public());
            let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
            let token_name = b"ACME";
            let ticker = Ticker::try_from(&token_name[..]).unwrap();
            let token_name2 = b"ACME2";
            let ticker2 = Ticker::try_from(&token_name2[..]).unwrap();
            let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
            let eve = AccountKeyring::Eve.public();
            init(token_name2, ticker2, AccountKeyring::Bob.public());

            let instruction_counter = Settlement::instruction_counter();
            let alice_init_balance = Asset::balance_of(&ticker, alice_did);
            let bob_init_balance = Asset::balance_of(&ticker, bob_did);
            let alice_init_balance2 = Asset::balance_of(&ticker2, alice_did);
            let bob_init_balance2 = Asset::balance_of(&ticker2, bob_did);

            // Provide scope claims to multiple parties of a transactions.
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker, eve);
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker2, eve);

            let amount = 100u128;
            let legs = vec![
                Leg {
                    from: PortfolioId::default_portfolio(alice_did),
                    to: PortfolioId::default_portfolio(bob_did),
                    asset: ticker,
                    amount: amount,
                },
                Leg {
                    from: PortfolioId::default_portfolio(bob_did),
                    to: PortfolioId::default_portfolio(alice_did),
                    asset: ticker2,
                    amount: amount,
                },
            ];

            assert_ok!(Settlement::add_instruction(
                alice_signed.clone(),
                venue_counter,
                SettlementType::SettleOnAffirmation,
                None,
                None,
                legs.clone()
            ));

            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );

            for i in 0..legs.len() {
                assert_eq!(
                    Settlement::instruction_legs(
                        instruction_counter,
                        u64::try_from(i).unwrap_or_default()
                    ),
                    legs[i]
                );
            }

            let instruction_details = Instruction {
                instruction_id: instruction_counter,
                venue_id: venue_counter,
                status: InstructionStatus::Pending,
                settlement_type: SettlementType::SettleOnAffirmation,
                created_at: Some(Timestamp::get()),
                trade_date: None,
                value_date: None,
            };
            assert_eq!(
                Settlement::instruction_details(instruction_counter),
                instruction_details
            );
            assert_eq!(
                Settlement::instruction_affirms_pending(instruction_counter),
                2
            );
            assert_eq!(
                Settlement::venue_info(venue_counter).unwrap().instructions,
                vec![instruction_counter]
            );

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
            assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

            let msg = Receipt {
                receipt_uid: 0,
                from: PortfolioId::default_portfolio(alice_did),
                to: PortfolioId::default_portfolio(bob_did),
                asset: ticker,
                amount: amount,
            };
            let signature = OffChainSignature::from(AccountKeyring::Alice.sign(&msg.encode()));

            let claim_receipt = |signature, metadata| {
                Settlement::claim_receipt(
                    alice_signed.clone(),
                    instruction_counter,
                    ReceiptDetails {
                        receipt_uid: 0,
                        leg_id: 0,
                        signer: AccountKeyring::Alice.public(),
                        signature,
                        metadata,
                    },
                )
            };

            assert_noop!(
                claim_receipt(signature.clone(), ReceiptMetadata::default()),
                Error::LegNotPending
            );
            set_current_block_number(4);

            assert_affirm_instruction_with_one_leg!(
                alice_signed.clone(),
                instruction_counter,
                alice_did
            );

            assert_eq!(
                Settlement::instruction_affirms_pending(instruction_counter),
                1
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(alice_did)
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(bob_did)
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 0),
                LegStatus::ExecutionPending
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 1),
                LegStatus::PendingTokenLock
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                amount
            );

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
            assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

            let msg2 = Receipt {
                receipt_uid: 0,
                from: PortfolioId::default_portfolio(alice_did),
                to: PortfolioId::default_portfolio(alice_did),
                asset: ticker,
                amount: amount,
            };
            let signature2 = OffChainSignature::from(AccountKeyring::Alice.sign(&msg2.encode()));

            assert_noop!(
                claim_receipt(signature2, ReceiptMetadata::default()),
                Error::InvalidSignature
            );

            let metadata = ReceiptMetadata::from(vec![42u8]);

            // Can not claim invalidated receipt
            let change_receipt_validity = |validity| {
                assert_ok!(Settlement::change_receipt_validity(
                    alice_signed.clone(),
                    0,
                    validity
                ));
            };
            change_receipt_validity(false);
            assert_noop!(
                claim_receipt(signature.clone(), metadata.clone()),
                Error::ReceiptAlreadyClaimed
            );
            change_receipt_validity(true);

            // Claiming, unclaiming and claiming receipt
            assert_ok!(claim_receipt(signature, metadata.clone()));

            assert_eq!(
                Settlement::receipts_used(AccountKeyring::Alice.public(), 0),
                true
            );
            assert_eq!(
                Settlement::instruction_affirms_pending(instruction_counter),
                1
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(alice_did)
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(bob_did)
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 0),
                LegStatus::ExecutionToBeSkipped(AccountKeyring::Alice.public(), 0)
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 1),
                LegStatus::PendingTokenLock
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                0
            );

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
            assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

            assert_ok!(Settlement::unclaim_receipt(
                alice_signed.clone(),
                instruction_counter,
                0
            ));

            assert_eq!(
                Settlement::instruction_affirms_pending(instruction_counter),
                1
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(alice_did)
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(bob_did)
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 0),
                LegStatus::ExecutionPending
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 1),
                LegStatus::PendingTokenLock
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                amount
            );

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
            assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

            assert_ok!(Settlement::claim_receipt(
                alice_signed.clone(),
                instruction_counter,
                ReceiptDetails {
                    receipt_uid: 0,
                    leg_id: 0,
                    signer: AccountKeyring::Alice.public(),
                    signature: OffChainSignature::from(AccountKeyring::Alice.sign(&msg.encode())),
                    metadata: ReceiptMetadata::default()
                }
            ));

            assert_eq!(
                Settlement::receipts_used(AccountKeyring::Alice.public(), 0),
                true
            );
            assert_eq!(
                Settlement::instruction_affirms_pending(instruction_counter),
                1
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(alice_did)
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(bob_did)
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 0),
                LegStatus::ExecutionToBeSkipped(AccountKeyring::Alice.public(), 0)
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 1),
                LegStatus::PendingTokenLock
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                0
            );

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
            assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

            set_current_block_number(10);

            assert_affirm_instruction_with_one_leg!(
                bob_signed.clone(),
                instruction_counter,
                bob_did
            );

            // Advances block.
            assert_instruction_execution!(
                assert_eq,
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                0
            );
            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(
                Asset::balance_of(&ticker2, alice_did),
                alice_init_balance2 + amount
            );
            assert_eq!(
                Asset::balance_of(&ticker2, bob_did),
                bob_init_balance2 - amount
            );
        });
}

#[test]
fn settle_on_block() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let alice_signed = Origin::signed(AccountKeyring::Alice.public());
            let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
            let bob_signed = Origin::signed(AccountKeyring::Bob.public());
            let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
            let token_name = b"ACME";
            let ticker = Ticker::try_from(&token_name[..]).unwrap();
            let token_name2 = b"ACME2";
            let ticker2 = Ticker::try_from(&token_name2[..]).unwrap();
            let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
            init(token_name2, ticker2, AccountKeyring::Bob.public());
            let block_number = System::block_number() + 1;
            let eve = AccountKeyring::Eve.public();
            let instruction_counter = Settlement::instruction_counter();
            let alice_init_balance = Asset::balance_of(&ticker, alice_did);
            let bob_init_balance = Asset::balance_of(&ticker, bob_did);
            let alice_init_balance2 = Asset::balance_of(&ticker2, alice_did);
            let bob_init_balance2 = Asset::balance_of(&ticker2, bob_did);

            let amount = 100u128;
            let legs = vec![
                Leg {
                    from: PortfolioId::default_portfolio(alice_did),
                    to: PortfolioId::default_portfolio(bob_did),
                    asset: ticker,
                    amount: amount,
                },
                Leg {
                    from: PortfolioId::default_portfolio(bob_did),
                    to: PortfolioId::default_portfolio(alice_did),
                    asset: ticker2,
                    amount: amount,
                },
            ];

            assert_eq!(0, scheduler::Agenda::<TestStorage>::get(block_number).len());
            assert_ok!(Settlement::add_instruction(
                alice_signed.clone(),
                venue_counter,
                SettlementType::SettleOnBlock(block_number),
                None,
                None,
                legs.clone()
            ));
            assert_eq!(1, scheduler::Agenda::<TestStorage>::get(block_number).len());

            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );

            for i in 0..legs.len() {
                assert_eq!(
                    Settlement::instruction_legs(
                        instruction_counter,
                        u64::try_from(i).unwrap_or_default()
                    ),
                    legs[i]
                );
            }

            let instruction_details = Instruction {
                instruction_id: instruction_counter,
                venue_id: venue_counter,
                status: InstructionStatus::Pending,
                settlement_type: SettlementType::SettleOnBlock(block_number),
                created_at: Some(Timestamp::get()),
                trade_date: None,
                value_date: None,
            };
            assert_eq!(
                Settlement::instruction_details(instruction_counter),
                instruction_details
            );
            assert_eq!(
                Settlement::instruction_affirms_pending(instruction_counter),
                2
            );
            assert_eq!(
                Settlement::venue_info(venue_counter).unwrap().instructions,
                vec![instruction_counter]
            );

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
            assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

            // Before authorization need to provide the scope claim for both the parties of a transaction.
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker, eve);
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker2, eve);

            assert_affirm_instruction_with_one_leg!(
                alice_signed.clone(),
                instruction_counter,
                alice_did
            );

            assert_eq!(
                Settlement::instruction_affirms_pending(instruction_counter),
                1
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Pending
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(alice_did)
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(bob_did)
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 0),
                LegStatus::ExecutionPending
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 1),
                LegStatus::PendingTokenLock
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                amount
            );

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
            assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

            assert_affirm_instruction_with_one_leg!(
                bob_signed.clone(),
                instruction_counter,
                bob_did
            );

            assert_eq!(
                Settlement::instruction_affirms_pending(instruction_counter),
                0
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(alice_did)
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::affirms_received(
                    instruction_counter,
                    PortfolioId::default_portfolio(bob_did)
                ),
                AffirmationStatus::Affirmed
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 0),
                LegStatus::ExecutionPending
            );
            assert_eq!(
                Settlement::instruction_leg_status(instruction_counter, 1),
                LegStatus::ExecutionPending
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                amount
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(bob_did), &ticker2),
                amount
            );

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
            assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

            next_block();

            // Instruction should've settled
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_counter
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_counter
                ),
                AffirmationStatus::Unknown
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                0
            );
            assert_eq!(
                Asset::balance_of(&ticker, alice_did),
                alice_init_balance - amount
            );
            assert_eq!(
                Asset::balance_of(&ticker, bob_did),
                bob_init_balance + amount
            );
            assert_eq!(
                Asset::balance_of(&ticker2, alice_did),
                alice_init_balance2 + amount
            );
            assert_eq!(
                Asset::balance_of(&ticker2, bob_did),
                bob_init_balance2 - amount
            );
        });
}

#[test]
fn failed_execution() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_signed = Origin::signed(AccountKeyring::Alice.public());
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let bob_signed = Origin::signed(AccountKeyring::Bob.public());
        let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
        let token_name = b"ACME";
        let ticker = Ticker::try_from(&token_name[..]).unwrap();
        let token_name2 = b"ACME2";
        let ticker2 = Ticker::try_from(&token_name2[..]).unwrap();
        let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
        init(token_name2, ticker2, AccountKeyring::Bob.public());
        assert_ok!(ComplianceManager::reset_asset_compliance(
            Origin::signed(AccountKeyring::Bob.public()),
            ticker2,
        ));
        let block_number = System::block_number() + 1;

        let instruction_counter = Settlement::instruction_counter();
        let alice_init_balance = Asset::balance_of(&ticker, alice_did);
        let bob_init_balance = Asset::balance_of(&ticker, bob_did);
        let alice_init_balance2 = Asset::balance_of(&ticker2, alice_did);
        let bob_init_balance2 = Asset::balance_of(&ticker2, bob_did);

        let amount = 100u128;
        let legs = vec![
            Leg {
                from: PortfolioId::default_portfolio(alice_did),
                to: PortfolioId::default_portfolio(bob_did),
                asset: ticker,
                amount: amount,
            },
            Leg {
                from: PortfolioId::default_portfolio(bob_did),
                to: PortfolioId::default_portfolio(alice_did),
                asset: ticker2,
                amount: amount,
            },
        ];

        assert_eq!(0, scheduler::Agenda::<TestStorage>::get(block_number).len());
        assert_ok!(Settlement::add_instruction(
            alice_signed.clone(),
            venue_counter,
            SettlementType::SettleOnBlock(block_number),
            None,
            None,
            legs.clone()
        ));
        assert_eq!(1, scheduler::Agenda::<TestStorage>::get(block_number).len());

        assert_eq!(
            Settlement::user_affirmations(
                PortfolioId::default_portfolio(alice_did),
                instruction_counter
            ),
            AffirmationStatus::Pending
        );
        assert_eq!(
            Settlement::user_affirmations(
                PortfolioId::default_portfolio(bob_did),
                instruction_counter
            ),
            AffirmationStatus::Pending
        );

        for i in 0..legs.len() {
            assert_eq!(
                Settlement::instruction_legs(
                    instruction_counter,
                    u64::try_from(i).unwrap_or_default()
                ),
                legs[i]
            );
        }

        let instruction_details = Instruction {
            instruction_id: instruction_counter,
            venue_id: venue_counter,
            status: InstructionStatus::Pending,
            settlement_type: SettlementType::SettleOnBlock(block_number),
            created_at: Some(Timestamp::get()),
            trade_date: None,
            value_date: None,
        };
        assert_eq!(
            Settlement::instruction_details(instruction_counter),
            instruction_details
        );
        assert_eq!(
            Settlement::instruction_affirms_pending(instruction_counter),
            2
        );
        assert_eq!(
            Settlement::venue_info(venue_counter).unwrap().instructions,
            vec![instruction_counter]
        );

        assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
        assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
        assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
        assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

        assert_affirm_instruction_with_one_leg!(
            alice_signed.clone(),
            instruction_counter,
            alice_did
        );

        assert_eq!(
            Settlement::instruction_affirms_pending(instruction_counter),
            1
        );
        assert_eq!(
            Settlement::user_affirmations(
                PortfolioId::default_portfolio(alice_did),
                instruction_counter
            ),
            AffirmationStatus::Affirmed
        );
        assert_eq!(
            Settlement::user_affirmations(
                PortfolioId::default_portfolio(bob_did),
                instruction_counter
            ),
            AffirmationStatus::Pending
        );
        assert_eq!(
            Settlement::affirms_received(
                instruction_counter,
                PortfolioId::default_portfolio(alice_did)
            ),
            AffirmationStatus::Affirmed
        );
        assert_eq!(
            Settlement::affirms_received(
                instruction_counter,
                PortfolioId::default_portfolio(bob_did)
            ),
            AffirmationStatus::Unknown
        );
        assert_eq!(
            Settlement::instruction_leg_status(instruction_counter, 0),
            LegStatus::ExecutionPending
        );
        assert_eq!(
            Settlement::instruction_leg_status(instruction_counter, 1),
            LegStatus::PendingTokenLock
        );
        assert_eq!(
            Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
            amount
        );

        assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
        assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
        assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
        assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

        assert_affirm_instruction_with_one_leg!(bob_signed.clone(), instruction_counter, bob_did);

        assert_eq!(
            Settlement::instruction_affirms_pending(instruction_counter),
            0
        );
        assert_eq!(
            Settlement::user_affirmations(
                PortfolioId::default_portfolio(alice_did),
                instruction_counter
            ),
            AffirmationStatus::Affirmed
        );
        assert_eq!(
            Settlement::user_affirmations(
                PortfolioId::default_portfolio(bob_did),
                instruction_counter
            ),
            AffirmationStatus::Affirmed
        );
        assert_eq!(
            Settlement::affirms_received(
                instruction_counter,
                PortfolioId::default_portfolio(alice_did)
            ),
            AffirmationStatus::Affirmed
        );
        assert_eq!(
            Settlement::affirms_received(
                instruction_counter,
                PortfolioId::default_portfolio(bob_did)
            ),
            AffirmationStatus::Affirmed
        );
        assert_eq!(
            Settlement::instruction_leg_status(instruction_counter, 0),
            LegStatus::ExecutionPending
        );
        assert_eq!(
            Settlement::instruction_leg_status(instruction_counter, 1),
            LegStatus::ExecutionPending
        );
        assert_eq!(
            Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
            amount
        );
        assert_eq!(
            Portfolio::locked_assets(PortfolioId::default_portfolio(bob_did), &ticker2),
            amount
        );

        assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
        assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
        assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
        assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

        next_block();

        // Instruction should've settled
        assert_eq!(
            Settlement::user_affirmations(
                PortfolioId::default_portfolio(alice_did),
                instruction_counter
            ),
            AffirmationStatus::Unknown
        );
        assert_eq!(
            Settlement::user_affirmations(
                PortfolioId::default_portfolio(bob_did),
                instruction_counter
            ),
            AffirmationStatus::Unknown
        );
        assert_eq!(
            Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
            0
        );
        assert_eq!(
            Portfolio::locked_assets(PortfolioId::default_portfolio(bob_did), &ticker2),
            0
        );
        assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
        assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
        assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
        assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);
    });
}

#[test]
fn venue_filtering() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let alice_signed = Origin::signed(AccountKeyring::Alice.public());
            let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
            let bob_signed = Origin::signed(AccountKeyring::Bob.public());
            let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
            let token_name = b"ACME";
            let ticker = Ticker::try_from(&token_name[..]).unwrap();
            let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
            let block_number = System::block_number() + 1;
            let instruction_counter = Settlement::instruction_counter();
            let eve = AccountKeyring::Eve.public();

            // provide scope claim.
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker, eve);

            let legs = vec![Leg {
                from: PortfolioId::default_portfolio(alice_did),
                to: PortfolioId::default_portfolio(bob_did),
                asset: ticker,
                amount: 10,
            }];
            assert_ok!(Settlement::add_instruction(
                alice_signed.clone(),
                venue_counter,
                SettlementType::SettleOnBlock(block_number),
                None,
                None,
                legs.clone()
            ));
            assert_ok!(Settlement::set_venue_filtering(
                alice_signed.clone(),
                ticker,
                true
            ));
            assert_noop!(
                Settlement::add_instruction(
                    alice_signed.clone(),
                    venue_counter,
                    SettlementType::SettleOnBlock(block_number),
                    None,
                    None,
                    legs.clone()
                ),
                Error::UnauthorizedVenue
            );
            assert_ok!(Settlement::allow_venues(
                alice_signed.clone(),
                ticker,
                vec![venue_counter]
            ));
            assert_ok!(Settlement::add_and_affirm_instruction(
                alice_signed.clone(),
                venue_counter,
                SettlementType::SettleOnBlock(block_number + 1),
                None,
                None,
                legs.clone(),
                default_portfolio_vec(alice_did)
            ));

            assert_affirm_instruction_with_one_leg!(
                alice_signed.clone(),
                instruction_counter,
                alice_did
            );
            assert_affirm_instruction_with_zero_leg!(
                bob_signed.clone(),
                instruction_counter,
                bob_did
            );
            assert_affirm_instruction_with_zero_leg!(
                bob_signed.clone(),
                instruction_counter + 1,
                bob_did
            );

            next_block();
            assert_eq!(Asset::balance_of(&ticker, bob_did), 10);
            assert_ok!(Settlement::disallow_venues(
                alice_signed.clone(),
                ticker,
                vec![venue_counter]
            ));
            next_block();
            // Second instruction fails to settle due to venue being not whitelisted
            assert_eq!(Asset::balance_of(&ticker, bob_did), 10);
        });
}

#[test]
fn basic_fuzzing() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let alice_signed = Origin::signed(AccountKeyring::Alice.public());
            let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
            let bob_signed = Origin::signed(AccountKeyring::Bob.public());
            let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
            let charlie_signed = Origin::signed(AccountKeyring::Charlie.public());
            let charlie_did = register_keyring_account(AccountKeyring::Charlie).unwrap();
            let dave_signed = Origin::signed(AccountKeyring::Dave.public());
            let dave_did = register_keyring_account(AccountKeyring::Dave).unwrap();
            let venue_counter = Settlement::venue_counter();
            let eve = AccountKeyring::Eve.public();
            assert_ok!(Settlement::create_venue(
                Origin::signed(AccountKeyring::Alice.public()),
                VenueDetails::default(),
                vec![AccountKeyring::Alice.public()],
                VenueType::Other
            ));
            let mut tickers = Vec::with_capacity(40);
            let mut balances = HashMap::with_capacity(320);
            let dids = vec![alice_did, bob_did, charlie_did, dave_did];
            let signers = vec![
                alice_signed.clone(),
                bob_signed.clone(),
                charlie_signed.clone(),
                dave_signed.clone(),
            ];

            for i in 0..10 {
                let mut create = |x: usize, key: AccountKeyring| {
                    let tn = [b'!' + u8::try_from(i * 4 + x).unwrap()];
                    tickers.push(Ticker::try_from(&tn[..]).unwrap());
                    create_token(&tn, tickers[i * 4 + x], key.public());
                };
                create(0, AccountKeyring::Alice);
                create(1, AccountKeyring::Bob);
                create(2, AccountKeyring::Charlie);
                create(3, AccountKeyring::Dave);
            }

            let block_number = System::block_number() + 1;
            let instruction_counter = Settlement::instruction_counter();

            // initialize balances
            for i in 0..10 {
                for j in 0..4 {
                    balances.insert((tickers[i * 4 + j], dids[j], "init").encode(), 100_000);
                    balances.insert((tickers[i * 4 + j], dids[j], "final").encode(), 100_000);
                    for k in 0..4 {
                        if j == k {
                            continue;
                        }
                        balances.insert((tickers[i * 4 + j], dids[k], "init").encode(), 0);
                        balances.insert((tickers[i * 4 + j], dids[k], "final").encode(), 0);
                    }
                }
            }

            let mut legs = Vec::with_capacity(100);
            let mut receipts = Vec::with_capacity(100);
            let mut receipt_legs = HashMap::with_capacity(100);
            let mut legs_count: HashMap<IdentityId, u32> = HashMap::with_capacity(100);
            for i in 0..10 {
                for j in 0..4 {
                    let mut final_i = 100_000;
                    balances.insert((tickers[i * 4 + j], dids[j], "init").encode(), 100_000);
                    for k in 0..4 {
                        if j == k {
                            continue;
                        }
                        balances.insert((tickers[i * 4 + j], dids[k], "init").encode(), 0);
                        if random() {
                            // This leg should happen
                            if random() {
                                // Receipt to be claimed
                                balances.insert((tickers[i * 4 + j], dids[k], "final").encode(), 0);
                                receipts.push(Receipt {
                                    receipt_uid: u64::try_from(k * 1000 + i * 4 + j).unwrap(),
                                    from: PortfolioId::default_portfolio(dids[j]),
                                    to: PortfolioId::default_portfolio(dids[k]),
                                    asset: tickers[i * 4 + j],
                                    amount: 1u128,
                                });
                                receipt_legs.insert(receipts.last().unwrap().encode(), legs.len());
                            } else {
                                balances.insert((tickers[i * 4 + j], dids[k], "final").encode(), 1);
                                final_i -= 1;
                            }
                            // Provide scope claim for all the dids
                            provide_scope_claim_to_multiple_parties(
                                &[dids[j], dids[k]],
                                tickers[i * 4 + j],
                                eve,
                            );
                            legs.push(Leg {
                                from: PortfolioId::default_portfolio(dids[j]),
                                to: PortfolioId::default_portfolio(dids[k]),
                                asset: tickers[i * 4 + j],
                                amount: 1,
                            });
                            let count = if legs_count.contains_key(&dids[j]) {
                                *legs_count.get(&dids[j]).unwrap() + 1
                            } else {
                                1
                            };
                            legs_count.insert(dids[j], count);
                            if legs.len() >= 100 {
                                break;
                            }
                        }
                    }
                    balances.insert((tickers[i * 4 + j], dids[j], "final").encode(), final_i);
                    if legs.len() >= 100 {
                        break;
                    }
                }
                if legs.len() >= 100 {
                    break;
                }
            }
            assert_ok!(Settlement::add_instruction(
                alice_signed.clone(),
                venue_counter,
                SettlementType::SettleOnBlock(block_number),
                None,
                None,
                legs
            ));

            // Authorize instructions and do a few authorize/deny in between
            for (i, signer) in signers.clone().iter().enumerate() {
                let leg_count = *legs_count.get(&dids[i]).unwrap_or(&0);
                for _ in 0..2 {
                    if random() {
                        assert_affirm_instruction!(
                            signer.clone(),
                            instruction_counter,
                            dids[i],
                            leg_count
                        );
                        assert_ok!(Settlement::withdraw_affirmation(
                            signer.clone(),
                            instruction_counter,
                            default_portfolio_vec(dids[i]),
                            leg_count
                        ));
                    }
                }
                assert_affirm_instruction!(signer.clone(), instruction_counter, dids[i], leg_count);
            }

            // Claim receipts and do a few claim/unclaims in between
            for receipt in receipts {
                let leg_num =
                    u64::try_from(*receipt_legs.get(&(receipt.encode())).unwrap()).unwrap();
                let signer = &signers[dids
                    .iter()
                    .position(|&from| PortfolioId::default_portfolio(from) == receipt.from)
                    .unwrap()];
                for _ in 0..2 {
                    if random() {
                        assert_ok!(Settlement::claim_receipt(
                            signer.clone(),
                            instruction_counter,
                            ReceiptDetails {
                                receipt_uid: receipt.receipt_uid,
                                leg_id: leg_num,
                                signer: AccountKeyring::Alice.public(),
                                signature: OffChainSignature::from(
                                    AccountKeyring::Alice.sign(&receipt.encode())
                                ),
                                metadata: ReceiptMetadata::default()
                            }
                        ));
                        assert_ok!(Settlement::unclaim_receipt(
                            signer.clone(),
                            instruction_counter,
                            leg_num
                        ));
                    }
                }
                assert_ok!(Settlement::claim_receipt(
                    signer.clone(),
                    instruction_counter,
                    ReceiptDetails {
                        receipt_uid: receipt.receipt_uid,
                        leg_id: leg_num,
                        signer: AccountKeyring::Alice.public(),
                        signature: OffChainSignature::from(
                            AccountKeyring::Alice.sign(&receipt.encode())
                        ),
                        metadata: ReceiptMetadata::default()
                    }
                ));
            }

            let fail: bool = random();
            if fail {
                let mut rng = thread_rng();
                let i = rng.gen_range(0, 4);
                assert_ok!(Settlement::withdraw_affirmation(
                    signers[i].clone(),
                    instruction_counter,
                    default_portfolio_vec(dids[i]),
                    *legs_count.get(&dids[i]).unwrap_or(&0)
                ));
            }

            next_block();

            for i in 0..40 {
                for j in 0..4 {
                    if fail {
                        assert_eq!(
                            Portfolio::locked_assets(
                                PortfolioId::default_portfolio(dids[j]),
                                &tickers[i]
                            ),
                            0
                        );
                        assert_eq!(
                            Asset::balance_of(&tickers[i], dids[j]),
                            u128::try_from(
                                *balances
                                    .get(&(tickers[i], dids[j], "init").encode())
                                    .unwrap()
                            )
                            .unwrap()
                        );
                    } else {
                        assert_eq!(
                            Asset::balance_of(&tickers[i], dids[j]),
                            u128::try_from(
                                *balances
                                    .get(&(tickers[i], dids[j], "final").encode())
                                    .unwrap()
                            )
                            .unwrap()
                        );
                    }
                }
            }
        });
}

#[test]
fn claim_multiple_receipts_during_authorization() {
    ExtBuilder::default().build().execute_with(|| {
        let alice_signed = Origin::signed(AccountKeyring::Alice.public());
        let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
        let bob_signed = Origin::signed(AccountKeyring::Bob.public());
        let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
        let token_name = b"ACME";
        let ticker = Ticker::try_from(&token_name[..]).unwrap();
        let token_name2 = b"ACME2";
        let ticker2 = Ticker::try_from(&token_name2[..]).unwrap();
        let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
        init(token_name2, ticker2, AccountKeyring::Bob.public());

        let instruction_counter = Settlement::instruction_counter();
        let alice_init_balance = Asset::balance_of(&ticker, alice_did);
        let bob_init_balance = Asset::balance_of(&ticker, bob_did);
        let alice_init_balance2 = Asset::balance_of(&ticker2, alice_did);
        let bob_init_balance2 = Asset::balance_of(&ticker2, bob_did);

        let amount = 100u128;
        let legs = vec![
            Leg {
                from: PortfolioId::default_portfolio(alice_did),
                to: PortfolioId::default_portfolio(bob_did),
                asset: ticker,
                amount: amount,
            },
            Leg {
                from: PortfolioId::default_portfolio(alice_did),
                to: PortfolioId::default_portfolio(bob_did),
                asset: ticker2,
                amount: amount,
            },
        ];

        assert_ok!(Settlement::add_instruction(
            alice_signed.clone(),
            venue_counter,
            SettlementType::SettleOnAffirmation,
            None,
            None,
            legs.clone()
        ));

        assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
        assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
        assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
        assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

        let msg1 = Receipt {
            receipt_uid: 0,
            from: PortfolioId::default_portfolio(alice_did),
            to: PortfolioId::default_portfolio(bob_did),
            asset: ticker,
            amount: amount,
        };
        let msg2 = Receipt {
            receipt_uid: 0,
            from: PortfolioId::default_portfolio(alice_did),
            to: PortfolioId::default_portfolio(bob_did),
            asset: ticker2,
            amount: amount,
        };
        let msg3 = Receipt {
            receipt_uid: 1,
            from: PortfolioId::default_portfolio(alice_did),
            to: PortfolioId::default_portfolio(bob_did),
            asset: ticker2,
            amount: amount,
        };

        assert_noop!(
            Settlement::affirm_with_receipts(
                alice_signed.clone(),
                instruction_counter,
                vec![
                    ReceiptDetails {
                        receipt_uid: 0,
                        leg_id: 0,
                        signer: AccountKeyring::Alice.public(),
                        signature: OffChainSignature::from(
                            AccountKeyring::Alice.sign(&msg1.encode())
                        ),
                        metadata: ReceiptMetadata::default()
                    },
                    ReceiptDetails {
                        receipt_uid: 0,
                        leg_id: 0,
                        signer: AccountKeyring::Alice.public(),
                        signature: OffChainSignature::from(
                            AccountKeyring::Alice.sign(&msg2.encode())
                        ),
                        metadata: ReceiptMetadata::default()
                    },
                ],
                default_portfolio_vec(alice_did),
                10
            ),
            Error::ReceiptAlreadyClaimed
        );

        assert_ok!(Settlement::affirm_with_receipts(
            alice_signed.clone(),
            instruction_counter,
            vec![
                ReceiptDetails {
                    receipt_uid: 0,
                    leg_id: 0,
                    signer: AccountKeyring::Alice.public(),
                    signature: OffChainSignature::from(AccountKeyring::Alice.sign(&msg1.encode())),
                    metadata: ReceiptMetadata::default()
                },
                ReceiptDetails {
                    receipt_uid: 1,
                    leg_id: 1,
                    signer: AccountKeyring::Alice.public(),
                    signature: OffChainSignature::from(AccountKeyring::Alice.sign(&msg3.encode())),
                    metadata: ReceiptMetadata::default()
                },
            ],
            default_portfolio_vec(alice_did),
            10
        ));

        assert_eq!(
            Settlement::instruction_affirms_pending(instruction_counter),
            1
        );
        assert_eq!(
            Settlement::user_affirmations(
                PortfolioId::default_portfolio(alice_did),
                instruction_counter
            ),
            AffirmationStatus::Affirmed
        );
        assert_eq!(
            Settlement::user_affirmations(
                PortfolioId::default_portfolio(bob_did),
                instruction_counter
            ),
            AffirmationStatus::Pending
        );
        assert_eq!(
            Settlement::affirms_received(
                instruction_counter,
                PortfolioId::default_portfolio(alice_did)
            ),
            AffirmationStatus::Affirmed
        );
        assert_eq!(
            Settlement::affirms_received(
                instruction_counter,
                PortfolioId::default_portfolio(bob_did)
            ),
            AffirmationStatus::Unknown
        );
        assert_eq!(
            Settlement::instruction_leg_status(instruction_counter, 0),
            LegStatus::ExecutionToBeSkipped(AccountKeyring::Alice.public(), 0)
        );
        assert_eq!(
            Settlement::instruction_leg_status(instruction_counter, 1),
            LegStatus::ExecutionToBeSkipped(AccountKeyring::Alice.public(), 1)
        );
        assert_eq!(
            Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
            0
        );

        assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
        assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
        assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
        assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);

        set_current_block_number(1);

        assert_affirm_instruction_with_zero_leg!(bob_signed.clone(), instruction_counter, bob_did);

        // Advances block
        assert_instruction_execution!(
            assert_eq,
            Settlement::user_affirmations(
                PortfolioId::default_portfolio(alice_did),
                instruction_counter
            ),
            AffirmationStatus::Unknown
        );
        assert_eq!(
            Settlement::user_affirmations(
                PortfolioId::default_portfolio(bob_did),
                instruction_counter
            ),
            AffirmationStatus::Unknown
        );
        assert_eq!(
            Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
            0
        );
        assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
        assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
        assert_eq!(Asset::balance_of(&ticker2, alice_did), alice_init_balance2);
        assert_eq!(Asset::balance_of(&ticker2, bob_did), bob_init_balance2);
    });
}

#[test]
fn overload_settle_on_block() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let alice_signed = Origin::signed(AccountKeyring::Alice.public());
            let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
            let bob_signed = Origin::signed(AccountKeyring::Bob.public());
            let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
            let token_name = b"ACME";
            let ticker = Ticker::try_from(&token_name[..]).unwrap();
            let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
            let instruction_counter = Settlement::instruction_counter();
            let alice_init_balance = Asset::balance_of(&ticker, alice_did);
            let bob_init_balance = Asset::balance_of(&ticker, bob_did);
            let eve = AccountKeyring::Eve.public();
            let block_number = System::block_number() + 1;

            let legs = vec![
                Leg {
                    from: PortfolioId::default_portfolio(alice_did),
                    to: PortfolioId::default_portfolio(bob_did),
                    asset: ticker,
                    amount: 1u128,
                };
                500
            ];

            // Provide scope claim to multiple parties of the transaction.
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker, eve);

            for _ in 0..2 {
                assert_ok!(Settlement::add_instruction(
                    alice_signed.clone(),
                    venue_counter,
                    SettlementType::SettleOnBlock(block_number),
                    None,
                    None,
                    legs.clone()
                ));
                assert_ok!(Settlement::add_instruction(
                    alice_signed.clone(),
                    venue_counter,
                    SettlementType::SettleOnBlock(block_number + 1),
                    None,
                    None,
                    legs.clone()
                ));
            }

            for i in &[0u64, 1, 3] {
                assert_affirm_instruction!(
                    alice_signed.clone(),
                    instruction_counter + i,
                    alice_did,
                    500
                );
                assert_affirm_instruction_with_zero_leg!(
                    bob_signed.clone(),
                    instruction_counter + i,
                    bob_did
                );
            }

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);

            assert_eq!(2, scheduler::Agenda::<TestStorage>::get(block_number).len());
            assert_eq!(
                2,
                scheduler::Agenda::<TestStorage>::get(block_number + 1).len()
            );
            assert_eq!(
                0,
                scheduler::Agenda::<TestStorage>::get(block_number + 2).len()
            );

            next_block();
            // First Instruction should've settled
            assert_eq!(
                Asset::balance_of(&ticker, alice_did),
                alice_init_balance - 500
            );
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance + 500);
            assert_eq!(0, scheduler::Agenda::<TestStorage>::get(block_number).len());
            assert_eq!(
                3,
                scheduler::Agenda::<TestStorage>::get(block_number + 1).len()
            );
            assert_eq!(
                0,
                scheduler::Agenda::<TestStorage>::get(block_number + 2).len()
            );

            next_block();
            // Second instruction should've settled
            assert_eq!(
                Asset::balance_of(&ticker, alice_did),
                alice_init_balance - 1000
            );
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance + 1000);
            assert_eq!(
                0,
                scheduler::Agenda::<TestStorage>::get(block_number + 1).len()
            );
            assert_eq!(
                2,
                scheduler::Agenda::<TestStorage>::get(block_number + 2).len()
            );
            assert_eq!(
                0,
                scheduler::Agenda::<TestStorage>::get(block_number + 3).len()
            );

            next_block();
            // Fourth instruction should've settled
            assert_eq!(
                Asset::balance_of(&ticker, alice_did),
                alice_init_balance - 1500
            );
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance + 1500);
            assert_eq!(
                0,
                scheduler::Agenda::<TestStorage>::get(block_number + 2).len()
            );
            assert_eq!(
                1,
                scheduler::Agenda::<TestStorage>::get(block_number + 3).len()
            );
            assert_eq!(
                0,
                scheduler::Agenda::<TestStorage>::get(block_number + 4).len()
            );

            assert_noop!(
                Settlement::affirm_instruction(
                    alice_signed.clone(),
                    instruction_counter + 2,
                    default_portfolio_vec(alice_did),
                    1
                ),
                Error::InstructionSettleBlockPassed
            );

            next_block();
            // Third instruction should've settled (Failed due to missing auth)
            assert_eq!(
                Asset::balance_of(&ticker, alice_did),
                alice_init_balance - 1500
            );
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance + 1500);
            assert_eq!(
                0,
                scheduler::Agenda::<TestStorage>::get(block_number + 3).len()
            );
            assert_eq!(
                0,
                scheduler::Agenda::<TestStorage>::get(block_number + 4).len()
            );
            assert_eq!(
                0,
                scheduler::Agenda::<TestStorage>::get(block_number + 5).len()
            );
        });
}

#[test]
fn encode_receipt() {
    ExtBuilder::default().build().execute_with(|| {
        let token_name = [0x01u8];
        let ticker = Ticker::try_from(&token_name[..]).unwrap();
        let msg1 = Receipt {
            receipt_uid: 0,
            from: PortfolioId::default_portfolio(
                IdentityId::try_from(
                    "did:poly:0600000000000000000000000000000000000000000000000000000000000000",
                )
                .unwrap(),
            ),
            to: PortfolioId::default_portfolio(
                IdentityId::try_from(
                    "did:poly:0600000000000000000000000000000000000000000000000000000000000000",
                )
                .unwrap()
                .into(),
            ),
            asset: ticker,
            amount: 100u128,
        };
        println!("{:?}", AccountKeyring::Alice.sign(&msg1.encode()));
    });
}

#[test]
fn test_weights_for_settlement_transaction() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Dave.public()])
        .set_max_tms_allowed(4) // set maximum no. of tms an asset can have.
        .build()
        .execute_with(|| {
            let alice = AccountKeyring::Alice.public();
            let (alice_signed, alice_did) = make_account_without_cdd(alice).unwrap();

            let bob = AccountKeyring::Bob.public();
            let (bob_signed, bob_did) = make_account_without_cdd(bob).unwrap();

            let eve = AccountKeyring::Eve.public();
            let (eve_signed, eve_did) = make_account_without_cdd(eve).unwrap();

            let token_name = b"ACME";
            let ticker = Ticker::try_from(&token_name[..]).unwrap();

            let venue_counter = init(token_name, ticker, alice);
            let instruction_counter = Settlement::instruction_counter();

            let dave = AccountKeyring::Dave.public();

            // Get token Id.
            let ticker_id = Identity::get_token_did(&ticker).unwrap();

            // Remove existing rules
            assert_ok!(ComplianceManager::remove_compliance_requirement(
                alice_signed.clone(),
                ticker,
                1
            ));
            // Add claim rules for settlement
            assert_ok!(ComplianceManager::add_compliance_requirement(
                alice_signed.clone(),
                ticker,
                vec![
                    Condition::from_dids(
                        ConditionType::IsPresent(Claim::Accredited(ticker_id.into())),
                        &[eve_did]
                    ),
                    Condition::from_dids(
                        ConditionType::IsAbsent(Claim::BuyLockup(ticker_id.into())),
                        &[eve_did]
                    )
                ],
                vec![
                    Condition::from_dids(
                        ConditionType::IsPresent(Claim::Accredited(ticker_id.into())),
                        &[eve_did]
                    ),
                    Condition::from_dids(
                        ConditionType::IsAnyOf(vec![
                            Claim::BuyLockup(ticker_id.into()),
                            Claim::KnowYourCustomer(ticker_id.into())
                        ]),
                        &[eve_did]
                    )
                ]
            ));

            // Providing claim to sender and receiver
            // For Alice
            assert_add_claim!(
                eve_signed.clone(),
                alice_did,
                Claim::Accredited(ticker_id.into())
            );
            // For Bob
            assert_add_claim!(
                eve_signed.clone(),
                bob_did,
                Claim::Accredited(ticker_id.into())
            );
            assert_add_claim!(
                eve_signed.clone(),
                bob_did,
                Claim::KnowYourCustomer(ticker_id.into())
            );

            // Provide scope claim as well to pass through the transaction.
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker, dave);

            // Create instruction
            let legs = vec![Leg {
                from: PortfolioId::default_portfolio(alice_did),
                to: PortfolioId::default_portfolio(bob_did),
                asset: ticker,
                amount: 100u128,
            }];

            assert_ok!(Settlement::add_instruction(
                alice_signed.clone(),
                venue_counter,
                SettlementType::SettleOnAffirmation,
                None,
                None,
                legs.clone()
            ));

            assert_affirm_instruction_with_one_leg!(
                alice_signed.clone(),
                instruction_counter,
                alice_did
            );
            set_current_block_number(100);
            assert_affirm_instruction_with_zero_leg!(
                bob_signed.clone(),
                instruction_counter,
                bob_did
            );

            assert_ok!(
                Asset::_is_valid_transfer(
                    &ticker,
                    PortfolioId::default_portfolio(alice_did),
                    PortfolioId::default_portfolio(bob_did),
                    100,
                ),
                ERC1400_TRANSFER_SUCCESS
            );
        });
}

#[test]
fn cross_portfolio_settlement() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let alice_signed = Origin::signed(AccountKeyring::Alice.public());
            let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
            let bob_signed = Origin::signed(AccountKeyring::Bob.public());
            let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
            let name = PortfolioName::from([42u8].to_vec());
            let num = Portfolio::next_portfolio_number(&bob_did);
            assert_ok!(Portfolio::create_portfolio(
                bob_signed.clone(),
                name.clone()
            ));
            let token_name = b"ACME";
            let ticker = Ticker::try_from(&token_name[..]).unwrap();
            let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
            let instruction_counter = Settlement::instruction_counter();
            let alice_init_balance = Asset::balance_of(&ticker, alice_did);
            let bob_init_balance = Asset::balance_of(&ticker, bob_did);
            let amount = 100u128;
            let eve = AccountKeyring::Eve.public();

            // Provide scope claim to sender and receiver of the transaction.
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker, eve);

            // Instruction referencing a user defined portfolio is created
            assert_ok!(Settlement::add_instruction(
                alice_signed.clone(),
                venue_counter,
                SettlementType::SettleOnAffirmation,
                None,
                None,
                vec![Leg {
                    from: PortfolioId::default_portfolio(alice_did),
                    to: PortfolioId::user_portfolio(bob_did, num),
                    asset: ticker,
                    amount: amount
                }]
            ));
            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(
                Portfolio::default_portfolio_balance(alice_did, &ticker),
                alice_init_balance,
            );
            assert_eq!(
                Portfolio::default_portfolio_balance(bob_did, &ticker),
                bob_init_balance,
            );
            assert_eq!(Portfolio::user_portfolio_balance(bob_did, num, &ticker), 0,);
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                0
            );
            set_current_block_number(10);
            // Approved by Alice
            assert_affirm_instruction_with_one_leg!(
                alice_signed.clone(),
                instruction_counter,
                alice_did
            );
            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(
                Portfolio::default_portfolio_balance(alice_did, &ticker),
                alice_init_balance,
            );
            assert_eq!(
                Portfolio::default_portfolio_balance(bob_did, &ticker),
                bob_init_balance,
            );
            assert_eq!(Portfolio::user_portfolio_balance(bob_did, num, &ticker), 0,);
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                amount
            );
            // Bob fails to approve the instruction with a
            // different portfolio than the one specified in the instruction
            assert_instruction_execution!(
                assert_noop,
                Settlement::affirm_instruction(
                    bob_signed.clone(),
                    instruction_counter,
                    default_portfolio_vec(bob_did),
                    0
                ),
                Error::UnexpectedAffirmationStatus
            );

            next_block();
            // Bob approves the instruction with the correct portfolio
            assert_ok!(Settlement::affirm_instruction(
                bob_signed.clone(),
                instruction_counter,
                user_portfolio_vec(bob_did, num),
                0
            ));
            // Instruction should've settled
            assert_instruction_execution!(
                assert_eq,
                Asset::balance_of(&ticker, alice_did),
                alice_init_balance - amount
            );

            assert_eq!(
                Asset::balance_of(&ticker, bob_did),
                bob_init_balance + amount
            );
            assert_eq!(
                Portfolio::default_portfolio_balance(alice_did, &ticker),
                alice_init_balance - amount,
            );
            assert_eq!(
                Portfolio::default_portfolio_balance(bob_did, &ticker),
                bob_init_balance,
            );
            assert_eq!(
                Portfolio::user_portfolio_balance(bob_did, num, &ticker),
                amount,
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                0
            );
        });
}

#[test]
fn multiple_portfolio_settlement() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let alice_signed = Origin::signed(AccountKeyring::Alice.public());
            let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
            let bob_signed = Origin::signed(AccountKeyring::Bob.public());
            let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();
            let name = PortfolioName::from([42u8].to_vec());
            let alice_num = Portfolio::next_portfolio_number(&alice_did);
            let bob_num = Portfolio::next_portfolio_number(&bob_did);
            assert_ok!(Portfolio::create_portfolio(
                bob_signed.clone(),
                name.clone()
            ));
            assert_ok!(Portfolio::create_portfolio(
                alice_signed.clone(),
                name.clone()
            ));
            let token_name = b"ACME";
            let ticker = Ticker::try_from(&token_name[..]).unwrap();
            let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
            let instruction_counter = Settlement::instruction_counter();
            let alice_init_balance = Asset::balance_of(&ticker, alice_did);
            let bob_init_balance = Asset::balance_of(&ticker, bob_did);
            let amount = 100u128;
            let eve = AccountKeyring::Eve.public();

            // Provide scope claim to sender and receiver of the transaction.
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker, eve);

            // An instruction is created with multiple legs referencing multiple portfolios
            assert_ok!(Settlement::add_instruction(
                alice_signed.clone(),
                venue_counter,
                SettlementType::SettleOnAffirmation,
                None,
                None,
                vec![
                    Leg {
                        from: PortfolioId::user_portfolio(alice_did, alice_num),
                        to: PortfolioId::default_portfolio(bob_did),
                        asset: ticker,
                        amount: amount
                    },
                    Leg {
                        from: PortfolioId::default_portfolio(alice_did),
                        to: PortfolioId::user_portfolio(bob_did, bob_num),
                        asset: ticker,
                        amount: amount
                    }
                ]
            ));
            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(
                Portfolio::default_portfolio_balance(alice_did, &ticker),
                alice_init_balance,
            );
            assert_eq!(
                Portfolio::default_portfolio_balance(bob_did, &ticker),
                bob_init_balance,
            );
            assert_eq!(
                Portfolio::user_portfolio_balance(bob_did, bob_num, &ticker),
                0,
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                0
            );

            // Alice approves the instruction from her default portfolio
            assert_affirm_instruction_with_one_leg!(
                alice_signed.clone(),
                instruction_counter,
                alice_did
            );

            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(
                Portfolio::default_portfolio_balance(alice_did, &ticker),
                alice_init_balance,
            );
            assert_eq!(
                Portfolio::default_portfolio_balance(bob_did, &ticker),
                bob_init_balance,
            );
            assert_eq!(
                Portfolio::user_portfolio_balance(bob_did, bob_num, &ticker),
                0
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                amount
            );

            // Alice fails to approve the instruction from her user specified portfolio due to lack of funds
            assert_noop!(
                Settlement::affirm_instruction(
                    alice_signed.clone(),
                    instruction_counter,
                    user_portfolio_vec(alice_did, alice_num),
                    1
                ),
                Error::FailedToLockTokens
            );

            // Alice moves her funds to the correct portfolio
            assert_ok!(Portfolio::move_portfolio_funds(
                alice_signed.clone(),
                PortfolioId::default_portfolio(alice_did),
                PortfolioId::user_portfolio(alice_did, alice_num),
                vec![MovePortfolioItem { ticker, amount }]
            ));
            set_current_block_number(15);
            // Alice is now able to approve the instruction with the user portfolio
            assert_ok!(Settlement::affirm_instruction(
                alice_signed.clone(),
                instruction_counter,
                user_portfolio_vec(alice_did, alice_num),
                1
            ));
            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(
                Portfolio::default_portfolio_balance(alice_did, &ticker),
                alice_init_balance - amount,
            );
            assert_eq!(
                Portfolio::user_portfolio_balance(alice_did, alice_num, &ticker),
                amount,
            );
            assert_eq!(
                Portfolio::default_portfolio_balance(bob_did, &ticker),
                bob_init_balance,
            );
            assert_eq!(
                Portfolio::user_portfolio_balance(bob_did, bob_num, &ticker),
                0
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                amount
            );
            assert_eq!(
                Portfolio::locked_assets(
                    PortfolioId::user_portfolio(alice_did, alice_num),
                    &ticker
                ),
                amount
            );

            // Bob approves the instruction with both of his portfolios in a single transaction
            let portfolios_vec = vec![
                PortfolioId::default_portfolio(bob_did),
                PortfolioId::user_portfolio(bob_did, bob_num),
            ];

            next_block();
            assert_ok!(Settlement::affirm_instruction(
                bob_signed.clone(),
                instruction_counter,
                portfolios_vec,
                0
            ));

            // Instruction should've settled
            assert_instruction_execution!(
                assert_eq,
                Asset::balance_of(&ticker, alice_did),
                alice_init_balance - amount * 2
            );
            assert_eq!(
                Asset::balance_of(&ticker, bob_did),
                bob_init_balance + amount * 2
            );
            assert_eq!(
                Portfolio::default_portfolio_balance(alice_did, &ticker),
                alice_init_balance - amount * 2,
            );
            assert_eq!(
                Portfolio::default_portfolio_balance(bob_did, &ticker),
                bob_init_balance + amount,
            );
            assert_eq!(
                Portfolio::user_portfolio_balance(bob_did, bob_num, &ticker),
                amount,
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                0
            );
        });
}

#[test]
fn multiple_custodian_settlement() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let alice_signed = Origin::signed(AccountKeyring::Alice.public());
            let alice_did = register_keyring_account(AccountKeyring::Alice).unwrap();
            let bob_signed = Origin::signed(AccountKeyring::Bob.public());
            let bob_did = register_keyring_account(AccountKeyring::Bob).unwrap();

            // Create portfolios
            let name = PortfolioName::from([42u8].to_vec());
            let alice_num = Portfolio::next_portfolio_number(&alice_did);
            let bob_num = Portfolio::next_portfolio_number(&bob_did);
            assert_ok!(Portfolio::create_portfolio(
                bob_signed.clone(),
                name.clone()
            ));
            assert_ok!(Portfolio::create_portfolio(
                alice_signed.clone(),
                name.clone()
            ));

            // Give custody of Bob's user portfolio to Alice
            let auth_id = Identity::add_auth(
                bob_did,
                Signatory::from(alice_did),
                AuthorizationData::PortfolioCustody(PortfolioId::user_portfolio(bob_did, bob_num)),
                None,
            );
            assert_ok!(Identity::accept_authorization(
                alice_signed.clone(),
                auth_id
            ));

            // Create a token
            let token_name = b"ACME";
            let ticker = Ticker::try_from(&token_name[..]).unwrap();
            let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
            let instruction_counter = Settlement::instruction_counter();
            let alice_init_balance = Asset::balance_of(&ticker, alice_did);
            let bob_init_balance = Asset::balance_of(&ticker, bob_did);
            let amount = 100u128;
            let eve = AccountKeyring::Eve.public();

            // Provide scope claim to sender and receiver of the transaction.
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker, eve);

            assert_ok!(Portfolio::move_portfolio_funds(
                alice_signed.clone(),
                PortfolioId::default_portfolio(alice_did),
                PortfolioId::user_portfolio(alice_did, alice_num),
                vec![MovePortfolioItem { ticker, amount }]
            ));

            // An instruction is created with multiple legs referencing multiple portfolios
            assert_ok!(Settlement::add_instruction(
                alice_signed.clone(),
                venue_counter,
                SettlementType::SettleOnAffirmation,
                None,
                None,
                vec![
                    Leg {
                        from: PortfolioId::user_portfolio(alice_did, alice_num),
                        to: PortfolioId::default_portfolio(bob_did),
                        asset: ticker,
                        amount: amount
                    },
                    Leg {
                        from: PortfolioId::default_portfolio(alice_did),
                        to: PortfolioId::user_portfolio(bob_did, bob_num),
                        asset: ticker,
                        amount: amount
                    }
                ]
            ));
            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(
                Portfolio::default_portfolio_balance(alice_did, &ticker),
                alice_init_balance - amount,
            );
            assert_eq!(
                Portfolio::default_portfolio_balance(bob_did, &ticker),
                bob_init_balance,
            );
            assert_eq!(
                Portfolio::user_portfolio_balance(bob_did, bob_num, &ticker),
                0,
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                0
            );

            // Alice approves the instruction from both of her portfolios
            let portfolios_vec = vec![
                PortfolioId::default_portfolio(alice_did),
                PortfolioId::user_portfolio(alice_did, alice_num),
            ];
            set_current_block_number(10);
            assert_ok!(Settlement::affirm_instruction(
                alice_signed.clone(),
                instruction_counter,
                portfolios_vec.clone(),
                2
            ));
            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            assert_eq!(
                Portfolio::default_portfolio_balance(alice_did, &ticker),
                alice_init_balance - amount,
            );
            assert_eq!(
                Portfolio::default_portfolio_balance(bob_did, &ticker),
                bob_init_balance,
            );
            assert_eq!(
                Portfolio::user_portfolio_balance(bob_did, bob_num, &ticker),
                0
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                amount
            );
            assert_eq!(
                Portfolio::locked_assets(
                    PortfolioId::user_portfolio(alice_did, alice_num),
                    &ticker
                ),
                amount
            );

            // Alice transfers custody of her portfolios but it won't affect any already approved instruction
            let auth_id2 = Identity::add_auth(
                alice_did,
                Signatory::from(bob_did),
                AuthorizationData::PortfolioCustody(PortfolioId::user_portfolio(
                    alice_did, alice_num,
                )),
                None,
            );
            assert_ok!(Identity::accept_authorization(bob_signed.clone(), auth_id2));

            // Bob fails to approve the instruction with both of his portfolios since he doesn't have custody for the second one
            let portfolios_bob = vec![
                PortfolioId::default_portfolio(bob_did),
                PortfolioId::user_portfolio(bob_did, bob_num),
            ];
            assert_noop!(
                Settlement::affirm_instruction(
                    bob_signed.clone(),
                    instruction_counter,
                    portfolios_bob,
                    0
                ),
                PortfolioError::UnauthorizedCustodian
            );

            next_block();
            // Bob can approve instruction from the portfolio he has custody of
            assert_affirm_instruction_with_zero_leg!(
                bob_signed.clone(),
                instruction_counter,
                bob_did
            );

            // Alice fails to deny the instruction from both her portfolios since she doesn't have the custody
            assert_instruction_execution!(
                assert_noop,
                Settlement::withdraw_affirmation(
                    alice_signed.clone(),
                    instruction_counter,
                    portfolios_vec,
                    2
                ),
                PortfolioError::UnauthorizedCustodian
            );

            // Alice can deny instruction from the portfolio she has custody of
            assert_ok!(Settlement::withdraw_affirmation(
                alice_signed.clone(),
                instruction_counter,
                default_portfolio_vec(alice_did),
                1
            ));
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                0
            );

            // Alice can authorize instruction from remaining portfolios since she has the custody
            let portfolios_final = vec![
                PortfolioId::default_portfolio(alice_did),
                PortfolioId::user_portfolio(bob_did, bob_num),
            ];
            next_block();
            assert_ok!(Settlement::affirm_instruction(
                alice_signed.clone(),
                instruction_counter,
                portfolios_final,
                1
            ));

            // Instruction should've settled
            assert_instruction_execution!(
                assert_eq,
                Asset::balance_of(&ticker, alice_did),
                alice_init_balance - amount * 2
            );
            assert_eq!(
                Asset::balance_of(&ticker, bob_did),
                bob_init_balance + amount * 2
            );
            assert_eq!(
                Portfolio::default_portfolio_balance(alice_did, &ticker),
                alice_init_balance - amount * 2,
            );
            assert_eq!(
                Portfolio::default_portfolio_balance(bob_did, &ticker),
                bob_init_balance + amount,
            );
            assert_eq!(
                Portfolio::user_portfolio_balance(bob_did, bob_num, &ticker),
                amount,
            );
            assert_eq!(
                Portfolio::locked_assets(PortfolioId::default_portfolio(alice_did), &ticker),
                0
            );
        });
}

#[test]
fn reject_instruction() {
    ExtBuilder::default().build().execute_with(|| {
        let (alice_signed, alice_did) = make_account(AccountKeyring::Alice.public()).unwrap();
        let (bob_signed, bob_did) = make_account(AccountKeyring::Bob.public()).unwrap();
        let (charlie_signed, _) = make_account(AccountKeyring::Charlie.public()).unwrap();

        let token_name = b"ACME";
        let ticker = Ticker::try_from(&token_name[..]).unwrap();
        let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
        let amount = 100u128;

        let assert_user_affirmatons = |instruction_id, alice_status, bob_status| {
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(alice_did),
                    instruction_id
                ),
                alice_status
            );
            assert_eq!(
                Settlement::user_affirmations(
                    PortfolioId::default_portfolio(bob_did),
                    instruction_id
                ),
                bob_status
            );
        };

        let create_instruction = || {
            let instruction_id = Settlement::instruction_counter();
            set_current_block_number(10);
            assert_ok!(Settlement::add_and_affirm_instruction(
                alice_signed.clone(),
                venue_counter,
                SettlementType::SettleOnAffirmation,
                None,
                None,
                vec![Leg {
                    from: PortfolioId::default_portfolio(alice_did),
                    to: PortfolioId::default_portfolio(bob_did),
                    asset: ticker,
                    amount: amount
                }],
                default_portfolio_vec(alice_did)
            ));
            instruction_id
        };

        let instruction_counter = create_instruction();
        assert_user_affirmatons(
            instruction_counter,
            AffirmationStatus::Affirmed,
            AffirmationStatus::Pending,
        );
        assert_noop!(
            Settlement::reject_instruction(bob_signed.clone(), instruction_counter, vec![], 0),
            Error::NoPortfolioProvided
        );

        assert_noop!(
            Settlement::reject_instruction(
                charlie_signed.clone(),
                instruction_counter,
                default_portfolio_vec(bob_did),
                0
            ),
            PortfolioError::UnauthorizedCustodian
        );
        next_block();
        assert_ok!(Settlement::reject_instruction(
            alice_signed.clone(),
            instruction_counter,
            default_portfolio_vec(alice_did),
            1
        ));
        next_block();
        // Instruction should've been deleted
        assert_user_affirmatons(
            instruction_counter,
            AffirmationStatus::Unknown,
            AffirmationStatus::Unknown,
        );

        // Test that the receiver can also reject the instruction
        let instruction_counter2 = create_instruction();

        assert_ok!(Settlement::reject_instruction(
            bob_signed.clone(),
            instruction_counter2,
            default_portfolio_vec(bob_did),
            0
        ));
        next_block();
        // Instruction should've been deleted
        assert_user_affirmatons(
            instruction_counter2,
            AffirmationStatus::Unknown,
            AffirmationStatus::Unknown,
        );
    });
}

#[test]
fn dirty_storage_with_tx() {
    ExtBuilder::default()
        .cdd_providers(vec![AccountKeyring::Eve.public()])
        .build()
        .execute_with(|| {
            let (alice_signed, alice_did) = make_account(AccountKeyring::Alice.public()).unwrap();
            let (bob_signed, bob_did) = make_account(AccountKeyring::Bob.public()).unwrap();
            let token_name = b"ACME";
            let ticker = Ticker::try_from(&token_name[..]).unwrap();
            let venue_counter = init(token_name, ticker, AccountKeyring::Alice.public());
            let instruction_counter = Settlement::instruction_counter();
            let alice_init_balance = Asset::balance_of(&ticker, alice_did);
            let bob_init_balance = Asset::balance_of(&ticker, bob_did);
            let amount1 = 100u128;
            let amount2 = 50u128;
            let eve = AccountKeyring::Eve.public();

            // Provide scope claim to sender and receiver of the transaction.
            provide_scope_claim_to_multiple_parties(&[alice_did, bob_did], ticker, eve);

            assert_ok!(Settlement::add_instruction(
                alice_signed.clone(),
                venue_counter,
                SettlementType::SettleOnAffirmation,
                None,
                None,
                vec![
                    Leg {
                        from: PortfolioId::default_portfolio(alice_did),
                        to: PortfolioId::default_portfolio(bob_did),
                        asset: ticker,
                        amount: amount1
                    },
                    Leg {
                        from: PortfolioId::default_portfolio(bob_did),
                        to: PortfolioId::default_portfolio(alice_did),
                        asset: ticker,
                        amount: 0
                    },
                    Leg {
                        from: PortfolioId::default_portfolio(alice_did),
                        to: PortfolioId::default_portfolio(bob_did),
                        asset: ticker,
                        amount: amount2
                    }
                ]
            ));

            assert_affirm_instruction!(alice_signed.clone(), instruction_counter, alice_did, 2);
            assert_eq!(Asset::balance_of(&ticker, alice_did), alice_init_balance);
            assert_eq!(Asset::balance_of(&ticker, bob_did), bob_init_balance);
            set_current_block_number(5);
            assert_affirm_instruction_with_one_leg!(
                bob_signed.clone(),
                instruction_counter,
                bob_did
            );

            // Advances the block no. to execute the instruction.
            let total_amount = amount1 + amount2;
            assert_eq!(
                Settlement::instruction_affirms_pending(instruction_counter),
                0
            );
            next_block();
            assert_eq!(
                settlement::InstructionLegs::<TestStorage>::iter_prefix(instruction_counter)
                    .count(),
                0
            );

            // Ensure proper balance transfers
            assert_eq!(
                Asset::balance_of(&ticker, alice_did),
                alice_init_balance - total_amount
            );
            assert_eq!(
                Asset::balance_of(&ticker, bob_did),
                bob_init_balance + total_amount
            );
        });
}
