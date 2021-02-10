use polymesh_primitives::IdentityId;

use frame_support::weights::DispatchInfo;
use sp_runtime::transaction_validity::{InvalidTransaction, TransactionValidity};

// Polymesh note: This was specifically added for Polymesh
pub trait CddAndFeeDetails<AccountId, Call> {
    fn get_valid_payer(
        call: &Call,
        caller: &AccountId,
    ) -> Result<Option<AccountId>, InvalidTransaction>;
    fn clear_context();
    fn set_payer_context(payer: Option<AccountId>);
    fn get_payer_from_context() -> Option<AccountId>;
    fn set_current_identity(did: &IdentityId);
}

// Polymesh note: This was specifically added for Polymesh
pub trait ChargeTxFee {
    fn charge_fee(len: u32, info: DispatchInfo) -> TransactionValidity;
}
