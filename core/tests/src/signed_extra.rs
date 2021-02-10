use pallet_group as group;
use pallet_identity as identity;
use polymesh_primitives::{AccountId, IdentityId, Index, InvestorUid};
use polymesh_runtime_develop::{
    runtime::{Call, SignedExtra},
    Runtime,
};

use frame_support::{weights::DispatchClass, weights::DispatchInfo};
use frame_system::{CheckEra, CheckGenesis, CheckNonce, CheckSpecVersion, CheckTxVersion};
use sp_io::TestExternalities;
use sp_runtime::{generic, traits::SignedExtension};
use sp_std::convert::From;

use test_client::AccountKeyring;

pub fn make_call() -> (<Runtime as frame_system::Trait>::Call, usize) {
    (Call::System(frame_system::Call::remark(vec![])), 10)
}

/// Generate a `SignedExtra` value as it is defined in `Runtime`.
/// It ensures that `Runtime` is using:
///     - Transaction `priority` == `tip`.
///     - Only `Operational` transactions could have `tip` != 0.
///     - `Normal` transactions have `priority` == 0, as `tip` == 0.
fn make_signed_extra(current_block: u64, period: u64, nonce: Index, tip: u128) -> SignedExtra {
    (
        CheckSpecVersion::<Runtime>::new(),
        CheckTxVersion::<Runtime>::new(),
        CheckGenesis::<Runtime>::new(),
        CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
        CheckNonce::<Runtime>::from(nonce),
        polymesh_extensions::CheckWeight::<Runtime>::new(),
        pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
        pallet_permissions::StoreCallMetadata::<Runtime>::new(),
    )
}

/// Create the minimun storage for `Runtime` to validate transactions.
/// It is needed because almost any call requires an account with DID and a valid CDD claim.
/// In this genesis configuration, we define the following:
///     - `Alice`, `Bob`, and `Charlie` accounts are created with `1_000_000` balance.
///     - Those accounts have DIDs (starting from 0), and auto-generated CDD claims.
///     - Those accounts are added as CDD providers, so auto-generated CDD claims are valid.
fn make_min_storage() -> Result<TestExternalities, String> {
    let accounts = [
        AccountKeyring::Alice.to_account_id(),
        AccountKeyring::Bob.to_account_id(),
        AccountKeyring::Charlie.to_account_id(),
    ];
    let identities = accounts
        .iter()
        .enumerate()
        .map(|(idx, acc)| {
            let did = IdentityId::from(idx as u128);
            let uid = InvestorUid::from(did.as_ref());

            (acc.clone(), did, did, uid, None)
        })
        .collect::<Vec<_>>();
    let did = identities
        .iter()
        .map(|(_acc, did, ..)| did.clone())
        .collect::<Vec<_>>();

    let mut storage = frame_system::GenesisConfig::default().build_storage::<Runtime>()?;

    // Balances
    pallet_balances::GenesisConfig::<Runtime> {
        balances: accounts
            .iter()
            .map(|acc| (acc.clone(), 1_000_000))
            .collect::<Vec<_>>(),
    }
    .assimilate_storage(&mut storage)?;

    // Sudo
    pallet_sudo::GenesisConfig::<Runtime> {
        key: accounts[0].clone(),
    }
    .assimilate_storage(&mut storage)?;

    // Identity
    identity::GenesisConfig::<Runtime> {
        identities: identities,
        ..Default::default()
    }
    .assimilate_storage(&mut storage)?;

    // CDD service
    group::GenesisConfig::<Runtime, group::Instance2> {
        active_members_limit: u32::MAX,
        active_members: did,
        ..Default::default()
    }
    .assimilate_storage(&mut storage)?;

    Ok(TestExternalities::new(storage))
}

#[test]
fn normal_tx_ext() -> Result<(), String> {
    make_min_storage()?.execute_with(normal_tx)
}

/// This test ensures the following rules are true for current `runtime::SignedExtra`:
///   - Normal transactions can not have a tip.
///   - Priority of any transaction is its own tip.
fn normal_tx() -> Result<(), String> {
    let user = AccountKeyring::Alice.to_account_id();
    let (call, len) = make_call();
    let info = DispatchInfo {
        weight: 100,
        ..Default::default()
    };

    // Normat Tx with tip. Expected an error.
    let sign_extra = make_signed_extra(0, 10, 0, 42u128.into());
    let tx_validity = sign_extra.validate(&user, &call, &info, len);
    assert!(tx_validity.is_err());

    // Normal TX without any tip.
    let sign_extra = make_signed_extra(0, 10, 0, 0u128.into());
    let tx_validity = sign_extra
        .validate(&user, &call, &info, len)
        .expect("Tx should be valid");
    assert_eq!(tx_validity.priority, 0);
    Ok(())
}

#[test]
fn operational_tx_ext() -> Result<(), String> {
    make_min_storage()?.execute_with(operational_tx)
}

/// This test double-checks the following statements:
///     - Operational transactions can have tip != 0.
///     - Priority of any transaction is its own tip.
fn operational_tx() -> Result<(), String> {
    let user: AccountId = AccountKeyring::Alice.public().into();
    let (call, len) = make_call();
    let info = DispatchInfo {
        weight: 100,
        class: DispatchClass::Operational,
        ..Default::default()
    };

    // Operational TX with tip.
    let tip = 42u128;
    let sign_extra = make_signed_extra(0, 10, 0, tip.into());
    let tx_validity = sign_extra
        .validate(&user, &call, &info, len)
        .expect("Tx should be valid");
    assert_eq!(tx_validity.priority as u128, tip);

    // Operational TX without any tip.
    let sign_extra = make_signed_extra(0, 10, 0, 0u128.into());
    let tx_validity = sign_extra
        .validate(&user, &call, &info, len)
        .expect("Tx should be valid");
    assert_eq!(tx_validity.priority, 0);
    Ok(())
}
