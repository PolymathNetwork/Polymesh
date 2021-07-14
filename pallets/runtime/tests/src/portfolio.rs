use super::{
    assert_last_event,
    asset_test::{create_token, max_len_bytes},
    storage::{EventTest, System, TestStorage, User},
    ExtBuilder,
};
use frame_support::{assert_noop, assert_ok, StorageMap};
use frame_system::EventRecord;
use pallet_portfolio::{MovePortfolioItem, RawEvent};
use polymesh_common_utilities::balances::Memo;
use polymesh_common_utilities::portfolio::PortfolioSubTrait;
use polymesh_primitives::{
    AuthorizationData, AuthorizationError, PortfolioId, PortfolioName, PortfolioNumber, Signatory,
};
use test_client::AccountKeyring;

type Asset = pallet_asset::Module<TestStorage>;
type Error = pallet_portfolio::Error<TestStorage>;
type Identity = pallet_identity::Module<TestStorage>;
type Origin = <TestStorage as frame_system::Config>::Origin;
type Portfolio = pallet_portfolio::Module<TestStorage>;

fn create_portfolio() -> (User, PortfolioNumber) {
    let owner = User::new(AccountKeyring::Alice);
    let name = PortfolioName::from([42u8].to_vec());
    let num = Portfolio::next_portfolio_number(&owner.did);
    assert_eq!(num, PortfolioNumber(1));
    assert_ok!(Portfolio::create_portfolio(owner.origin(), name.clone()));
    assert_eq!(Portfolio::portfolios(&owner.did, num), name);
    (owner, num)
}

fn set_custodian_ok(current_custodian: User, new_custodian: User, portfolio_id: PortfolioId) {
    let auth_id = Identity::add_auth(
        current_custodian.did,
        Signatory::from(new_custodian.did),
        AuthorizationData::PortfolioCustody(portfolio_id),
        None,
    );
    assert_ok!(Portfolio::accept_portfolio_custody(
        new_custodian.origin(),
        auth_id
    ));
}

macro_rules! assert_owner_is_custodian {
    ($p:expr) => {{
        assert_eq!(Portfolio::portfolios_in_custody($p.did, $p), false);
        assert_eq!(
            pallet_portfolio::PortfolioCustodian::contains_key(&$p),
            false
        );
    }};
}

#[test]
fn portfolio_name_too_long() {
    ExtBuilder::default().build().execute_with(|| {
        let owner = User::new(AccountKeyring::Alice);
        let id = Portfolio::next_portfolio_number(owner.did);
        let create = |name| Portfolio::create_portfolio(owner.origin(), name);
        let rename = |name| Portfolio::rename_portfolio(owner.origin(), id, name);
        assert_too_long!(create(max_len_bytes(1)));
        assert_ok!(create(max_len_bytes(0)));
        assert_too_long!(rename(max_len_bytes(1)));
        assert_ok!(rename(b"".into()));
        assert_ok!(rename(max_len_bytes(0)));
    });
}

#[test]
fn can_create_rename_delete_portfolio() {
    ExtBuilder::default().build().execute_with(|| {
        let (owner, num) = create_portfolio();
        let new_name = PortfolioName::from([55u8].to_vec());
        assert_ok!(Portfolio::rename_portfolio(
            owner.origin(),
            num,
            new_name.clone()
        ));
        assert_eq!(
            Portfolio::next_portfolio_number(&owner.did),
            PortfolioNumber(2)
        );
        assert_eq!(Portfolio::portfolios(&owner.did, num), new_name);
        assert_ok!(Portfolio::delete_portfolio(owner.origin(), num));
    });
}

#[test]
fn cannot_delete_portfolio_with_asset() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(1); // This is needed to enable events.

        let (owner, num) = create_portfolio();
        let (ticker, token) = create_token(owner);
        let owner_default_portfolio = PortfolioId::default_portfolio(owner.did);
        let owner_user_portfolio = PortfolioId::user_portfolio(owner.did, num);

        // Move funds to new portfolio
        let move_amount = token.total_supply / 2;
        assert_ok!(Portfolio::move_portfolio_funds(
            owner.origin(),
            owner_default_portfolio,
            owner_user_portfolio,
            vec![MovePortfolioItem {
                ticker,
                amount: move_amount,
                memo: None,
            }]
        ));
        // check MovedBetweenPortfolios event
        assert_last_event!(
            EventTest::pallet_portfolio(RawEvent::MovedBetweenPortfolios(
                did, from, to, i_ticker, i_amount, i_memo
            )),
            did == &owner.did
                && from == &owner_default_portfolio
                && to == &owner_user_portfolio
                && i_ticker == &ticker
                && i_amount == &move_amount
                && i_memo.is_none()
        );
        let ensure_balances = |default_portfolio_balance, user_portfolio_balance| {
            assert_eq!(
                Portfolio::default_portfolio_balance(owner.did, &ticker),
                default_portfolio_balance
            );
            assert_eq!(
                Portfolio::user_portfolio_balance(owner.did, num, &ticker),
                user_portfolio_balance
            );
        };
        ensure_balances(token.total_supply - move_amount, move_amount);

        // Cannot delete portfolio as it's non-empty.
        let delete = || Portfolio::delete_portfolio(owner.origin(), num);
        assert_noop!(delete(), Error::PortfolioNotEmpty);
        ensure_balances(token.total_supply - move_amount, move_amount);

        // Remove remaining funds.
        assert_ok!(Portfolio::move_portfolio_funds(
            owner.origin(),
            owner_user_portfolio,
            owner_default_portfolio,
            vec![MovePortfolioItem {
                ticker,
                amount: move_amount,
                memo: None,
            }]
        ));
        ensure_balances(token.total_supply, 0);

        // And now we can delete.
        assert_ok!(delete());
    });
}

#[test]
fn can_move_asset_from_portfolio() {
    ExtBuilder::default()
        .build()
        .execute_with(|| do_move_asset_from_portfolio(None));
}

#[test]
fn can_move_asset_from_portfolio_with_memo() {
    ExtBuilder::default()
        .build()
        .execute_with(|| do_move_asset_from_portfolio(Some(Memo::from("Test memo"))));
}

fn do_move_asset_from_portfolio(memo: Option<Memo>) {
    System::set_block_number(1); // This is needed to enable events.

    let (owner, num) = create_portfolio();
    let bob = User::new(AccountKeyring::Bob);
    let (ticker, token) = create_token(owner);
    assert_eq!(
        Portfolio::default_portfolio_balance(owner.did, &ticker),
        token.total_supply,
    );
    assert_eq!(
        Portfolio::user_portfolio_balance(owner.did, num, &ticker),
        0,
    );

    let owner_default_portfolio = PortfolioId::default_portfolio(owner.did);
    let owner_user_portfolio = PortfolioId::user_portfolio(owner.did, num);

    // Attempt to move more than the total supply.
    assert_noop!(
        Portfolio::move_portfolio_funds(
            owner.origin(),
            owner_default_portfolio,
            owner_user_portfolio,
            vec![MovePortfolioItem {
                ticker,
                amount: token.total_supply * 2,
                memo: memo.clone()
            }]
        ),
        Error::InsufficientPortfolioBalance
    );
    assert_noop!(
        Portfolio::ensure_portfolio_transfer_validity(
            &owner_default_portfolio,
            &owner_user_portfolio,
            &ticker,
            &(token.total_supply * 2),
        ),
        Error::InsufficientPortfolioBalance
    );

    // Attempt to move to the same portfolio.
    assert_noop!(
        Portfolio::move_portfolio_funds(
            owner.origin(),
            owner_default_portfolio,
            owner_default_portfolio,
            vec![MovePortfolioItem {
                ticker,
                amount: 1,
                memo: memo.clone()
            }]
        ),
        Error::DestinationIsSamePortfolio
    );
    assert_noop!(
        Portfolio::ensure_portfolio_transfer_validity(
            &owner_default_portfolio,
            &owner_default_portfolio,
            &ticker,
            &1,
        ),
        Error::DestinationIsSamePortfolio
    );

    // Attempt to move to a non-existent portfolio.
    assert_noop!(
        Portfolio::ensure_portfolio_transfer_validity(
            &owner_default_portfolio,
            &PortfolioId::user_portfolio(owner.did, PortfolioNumber(666)),
            &ticker,
            &1,
        ),
        Error::PortfolioDoesNotExist
    );

    // Attempt to move by another identity.
    assert_noop!(
        Portfolio::move_portfolio_funds(
            bob.origin(),
            owner_default_portfolio,
            owner_user_portfolio,
            vec![MovePortfolioItem {
                ticker,
                amount: 1,
                memo: memo.clone()
            }]
        ),
        Error::UnauthorizedCustodian
    );

    // Move an amount within bounds.
    let move_amount = token.total_supply / 2;
    assert_ok!(Portfolio::move_portfolio_funds(
        owner.origin(),
        owner_default_portfolio,
        owner_user_portfolio,
        vec![MovePortfolioItem {
            ticker,
            amount: move_amount,
            memo: memo.clone()
        }]
    ));
    // check MovedBetweenPortfolios event
    assert_last_event!(
        EventTest::pallet_portfolio(RawEvent::MovedBetweenPortfolios(
            did, from, to, i_ticker, i_amount, i_memo
        )),
        did == &owner.did
            && from == &owner_default_portfolio
            && to == &owner_user_portfolio
            && i_ticker == &ticker
            && i_amount == &move_amount
            && i_memo == &memo
    );
    assert_ok!(Portfolio::ensure_portfolio_transfer_validity(
        &owner_default_portfolio,
        &owner_user_portfolio,
        &ticker,
        &move_amount,
    ));
    assert_eq!(
        Portfolio::default_portfolio_balance(owner.did, &ticker),
        token.total_supply - move_amount,
    );
    assert_eq!(
        Portfolio::user_portfolio_balance(owner.did, num, &ticker),
        move_amount,
    );
}

#[test]
fn can_lock_unlock_assets() {
    ExtBuilder::default().build().execute_with(|| {
        let (owner, num) = create_portfolio();
        let (ticker, token) = create_token(owner);
        assert_eq!(
            Portfolio::default_portfolio_balance(owner.did, &ticker),
            token.total_supply,
        );

        let owner_default_portfolio = PortfolioId::default_portfolio(owner.did);
        let owner_user_portfolio = PortfolioId::user_portfolio(owner.did, num);

        // Lock half of the tokens
        let lock_amount = token.total_supply / 2;
        assert_ok!(Portfolio::lock_tokens(
            &owner_default_portfolio,
            &ticker,
            &lock_amount
        ));

        assert_eq!(
            Portfolio::default_portfolio_balance(owner.did, &ticker),
            token.total_supply,
        );
        assert_eq!(
            Portfolio::locked_assets(owner_default_portfolio, &ticker),
            lock_amount,
        );

        assert_noop!(
            Portfolio::move_portfolio_funds(
                owner.origin(),
                owner_default_portfolio,
                owner_user_portfolio,
                vec![MovePortfolioItem {
                    ticker,
                    amount: token.total_supply,
                    memo: None,
                }]
            ),
            Error::InsufficientPortfolioBalance
        );

        // Transfer for unlocked tokens succeeds
        assert_ok!(Portfolio::move_portfolio_funds(
            owner.origin(),
            owner_default_portfolio,
            owner_user_portfolio,
            vec![MovePortfolioItem {
                ticker,
                amount: lock_amount,
                memo: None,
            }]
        ));
        assert_eq!(
            Portfolio::default_portfolio_balance(owner.did, &ticker),
            token.total_supply - lock_amount,
        );
        assert_eq!(
            Portfolio::user_portfolio_balance(owner.did, num, &ticker),
            lock_amount,
        );
        assert_eq!(
            Portfolio::locked_assets(owner_default_portfolio, &ticker),
            lock_amount,
        );

        // Transfer of any more tokens fails
        assert_noop!(
            Portfolio::move_portfolio_funds(
                owner.origin(),
                owner_default_portfolio,
                owner_user_portfolio,
                vec![MovePortfolioItem {
                    ticker,
                    amount: 1,
                    memo: None
                }]
            ),
            Error::InsufficientPortfolioBalance
        );

        // Unlock tokens
        assert_ok!(Portfolio::unlock_tokens(
            &owner_default_portfolio,
            &ticker,
            &lock_amount
        ));

        assert_eq!(
            Portfolio::default_portfolio_balance(owner.did, &ticker),
            token.total_supply - lock_amount,
        );
        assert_eq!(
            Portfolio::user_portfolio_balance(owner.did, num, &ticker),
            lock_amount,
        );
        assert_eq!(
            Portfolio::locked_assets(owner_default_portfolio, &ticker),
            0,
        );

        // Transfer of all tokens succeeds since there is no lock anymore
        assert_ok!(Portfolio::move_portfolio_funds(
            owner.origin(),
            owner_default_portfolio,
            owner_user_portfolio,
            vec![MovePortfolioItem {
                ticker,
                amount: token.total_supply - lock_amount,
                memo: None,
            }]
        ));
        assert_eq!(Portfolio::default_portfolio_balance(owner.did, &ticker), 0,);
        assert_eq!(
            Portfolio::user_portfolio_balance(owner.did, num, &ticker),
            token.total_supply,
        );
        assert_eq!(
            Portfolio::locked_assets(owner_default_portfolio, &ticker),
            0,
        );
    });
}

#[test]
fn can_take_custody_of_portfolios() {
    ExtBuilder::default().build().execute_with(|| {
        let (owner, num) = create_portfolio();
        let bob = User::new(AccountKeyring::Bob);

        let owner_default_portfolio = PortfolioId::default_portfolio(owner.did);
        let owner_user_portfolio = PortfolioId::user_portfolio(owner.did, num);

        let has_custody = |u: User| Portfolio::portfolios_in_custody(u.did, owner_user_portfolio);

        // Custody of all portfolios is with the owner identity by default
        assert_ok!(Portfolio::ensure_portfolio_custody(
            owner_default_portfolio,
            owner.did
        ));
        assert_ok!(Portfolio::ensure_portfolio_custody(
            owner_user_portfolio,
            owner.did
        ));
        assert_eq!(
            Portfolio::portfolio_custodian(owner_default_portfolio),
            None
        );
        assert_eq!(Portfolio::portfolio_custodian(owner_user_portfolio), None);
        assert!(!has_custody(bob));

        // Bob can not issue authorization for custody transfer of a portfolio they don't have custody of
        let add_auth = |from: User, target: User| {
            let auth = AuthorizationData::PortfolioCustody(owner_user_portfolio);
            Identity::add_auth(from.did, Signatory::from(target.did), auth, None)
        };

        let auth_id = add_auth(bob, bob);
        assert_noop!(
            Portfolio::accept_portfolio_custody(bob.origin(), auth_id),
            AuthorizationError::Unauthorized
        );

        // Can not accept an invalid auth
        assert_noop!(
            Portfolio::accept_portfolio_custody(bob.origin(), auth_id + 1),
            AuthorizationError::Invalid
        );

        // Can accept a valid custody transfer auth
        let auth_id = add_auth(owner, bob);
        assert_ok!(Portfolio::accept_portfolio_custody(bob.origin(), auth_id));

        assert_ok!(Portfolio::ensure_portfolio_custody(
            owner_default_portfolio,
            owner.did
        ));
        assert_ok!(Portfolio::ensure_portfolio_custody(
            owner_user_portfolio,
            bob.did
        ));
        assert_noop!(
            Portfolio::ensure_portfolio_custody(owner_user_portfolio, owner.did),
            Error::UnauthorizedCustodian
        );
        assert_eq!(
            Portfolio::portfolio_custodian(owner_default_portfolio),
            None
        );
        assert_eq!(
            Portfolio::portfolio_custodian(owner_user_portfolio),
            Some(bob.did)
        );
        assert!(has_custody(bob));

        // Owner can not issue authorization for custody transfer of a portfolio they don't have custody of
        let auth_id = add_auth(owner, owner);
        assert_noop!(
            Portfolio::accept_portfolio_custody(owner.origin(), auth_id),
            AuthorizationError::Unauthorized
        );

        // Bob transfers portfolio custody back to Alice.
        set_custodian_ok(bob, owner, owner_user_portfolio);
        // The mapping is removed which means the owner is the custodian.
        assert_owner_is_custodian!(owner_user_portfolio);
    });
}

#[test]
fn quit_portfolio_custody() {
    ExtBuilder::default().build().execute_with(|| {
        let (alice, num) = create_portfolio();
        let bob = User::new(AccountKeyring::Bob);
        let user_portfolio = PortfolioId::user_portfolio(alice.did, num);

        assert_noop!(
            Portfolio::quit_portfolio_custody(bob.origin(), user_portfolio),
            Error::UnauthorizedCustodian
        );
        set_custodian_ok(alice, bob, user_portfolio);
        assert_ok!(Portfolio::quit_portfolio_custody(
            bob.origin(),
            user_portfolio
        ));
        // The mapping is removed which means the owner is the custodian.
        assert_owner_is_custodian!(user_portfolio);
    });
}
