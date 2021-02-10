// Copyright (C) 2019-2020 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

// Modified by Polymath Inc - 13rd March 2020
// - Charge fee from the identity in the signed extension
// - Introduce `ChargeTxFee` trait to compute and charge transaction fee for Multisig.
// - Tips have been removed.

//! # Transaction Payment Module
//!
//! This module provides the basic logic needed to pay the absolute minimum amount needed for a
//! transaction to be included. This includes:
//!   - _weight fee_: A fee proportional to amount of weight a transaction consumes.
//!   - _length fee_: A fee proportional to the encoded length of the transaction.
//!   - _tip_: An optional tip. Tip increases the priority of the transaction, giving it a higher
//!     chance to be included by the transaction queue.
//!
//! Additionally, this module allows one to configure:
//!   - The mapping between one unit of weight to one unit of fee via [`Trait::WeightToFee`].
//!   - A means of updating the fee for the next block, via defining a multiplier, based on the
//!     final state of the chain at the end of the previous block. This can be configured via
//!     [`Trait::FeeMultiplierUpdate`]

#![cfg_attr(not(feature = "std"), no_std)]

pub mod runtime_dispatch_info;
pub use runtime_dispatch_info::RuntimeDispatchInfo;

use codec::{Decode, Encode};
use frame_support::ensure;
use frame_support::{
    decl_module, decl_storage,
    dispatch::DispatchResult,
    traits::{Currency, ExistenceRequirement, Get, Imbalance, OnUnbalanced, WithdrawReason},
    weights::{
        DispatchClass, DispatchInfo, GetDispatchInfo, Pays, PostDispatchInfo, Weight,
        WeightToFeeCoefficient, WeightToFeePolynomial,
    },
};
use polymesh_common_utilities::traits::transaction_payment::{CddAndFeeDetails, ChargeTxFee};
use polymesh_primitives::TransactionError;
use sp_runtime::{
    traits::{
        Convert, DispatchInfoOf, Dispatchable, PostDispatchInfoOf, SaturatedConversion, Saturating,
        SignedExtension, Zero,
    },
    transaction_validity::{
        InvalidTransaction, TransactionPriority, TransactionValidity, TransactionValidityError,
        ValidTransaction,
    },
    FixedPointNumber, FixedPointOperand, FixedU128, Perquintill,
};
use sp_std::prelude::*;

/// Fee multiplier.
pub type Multiplier = FixedU128;

type BalanceOf<T> =
    <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
    <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::NegativeImbalance;

/// A struct to update the weight multiplier per block. It implements `Convert<Multiplier,
/// Multiplier>`, meaning that it can convert the previous multiplier to the next one. This should
/// be called on `on_finalize` of a block, prior to potentially cleaning the weight data from the
/// system module.
///
/// given:
/// 	s = previous block weight
/// 	s'= ideal block weight
/// 	m = maximum block weight
///		diff = (s - s')/m
///		v = 0.00001
///		t1 = (v * diff)
///		t2 = (v * diff)^2 / 2
///	then:
/// 	next_multiplier = prev_multiplier * (1 + t1 + t2)
///
/// Where `(s', v)` must be given as the `Get` implementation of the `T` generic type. Moreover, `M`
/// must provide the minimum allowed value for the multiplier. Note that a runtime should ensure
/// with tests that the combination of this `M` and `V` is not such that the multiplier can drop to
/// zero and never recover.
///
/// note that `s'` is interpreted as a portion in the _normal transaction_ capacity of the block.
/// For example, given `s' == 0.25` and `AvailableBlockRatio = 0.75`, then the target fullness is
/// _0.25 of the normal capacity_ and _0.1875 of the entire block_.
///
/// This implementation implies the bound:
/// - `v ≤ p / k * (s − s')`
/// - or, solving for `p`: `p >= v * k * (s - s')`
///
/// where `p` is the amount of change over `k` blocks.
///
/// Hence:
/// - in a fully congested chain: `p >= v * k * (1 - s')`.
/// - in an empty chain: `p >= v * k * (-s')`.
///
/// For example, when all blocks are full and there are 28800 blocks per day (default in `substrate-node`)
/// and v == 0.00001, s' == 0.1875, we'd have:
///
/// p >= 0.00001 * 28800 * 0.8125
/// p >= 0.234
///
/// Meaning that fees can change by around ~23% per day, given extreme congestion.
///
/// More info can be found at:
/// https://w3f-research.readthedocs.io/en/latest/polkadot/Token%20Economics.html
pub struct TargetedFeeAdjustment<T, S, V, M>(sp_std::marker::PhantomData<(T, S, V, M)>);

/// Something that can convert the current multiplier to the next one.
pub trait MultiplierUpdate: Convert<Multiplier, Multiplier> {
    /// Minimum multiplier
    fn min() -> Multiplier;
    /// Target block saturation level
    fn target() -> Perquintill;
    /// Variability factor
    fn variability() -> Multiplier;
}

impl MultiplierUpdate for () {
    fn min() -> Multiplier {
        Default::default()
    }
    fn target() -> Perquintill {
        Default::default()
    }
    fn variability() -> Multiplier {
        Default::default()
    }
}

impl<T, S, V, M> MultiplierUpdate for TargetedFeeAdjustment<T, S, V, M>
where
    T: frame_system::Trait,
    S: Get<Perquintill>,
    V: Get<Multiplier>,
    M: Get<Multiplier>,
{
    fn min() -> Multiplier {
        M::get()
    }
    fn target() -> Perquintill {
        S::get()
    }
    fn variability() -> Multiplier {
        V::get()
    }
}

impl<T, S, V, M> Convert<Multiplier, Multiplier> for TargetedFeeAdjustment<T, S, V, M>
where
    T: frame_system::Trait,
    S: Get<Perquintill>,
    V: Get<Multiplier>,
    M: Get<Multiplier>,
{
    fn convert(previous: Multiplier) -> Multiplier {
        // Defensive only. The multiplier in storage should always be at most positive. Nonetheless
        // we recover here in case of errors, because any value below this would be stale and can
        // never change.
        let min_multiplier = M::get();
        let previous = previous.max(min_multiplier);

        // the computed ratio is only among the normal class.
        let normal_max_weight = <T as frame_system::Trait>::AvailableBlockRatio::get()
            * <T as frame_system::Trait>::MaximumBlockWeight::get();
        let normal_block_weight = <frame_system::Module<T>>::block_weight()
            .get(frame_support::weights::DispatchClass::Normal)
            .min(normal_max_weight);

        let s = S::get();
        let v = V::get();

        let target_weight = (s * normal_max_weight) as u128;
        let block_weight = normal_block_weight as u128;

        // determines if the first_term is positive
        let positive = block_weight >= target_weight;
        let diff_abs = block_weight.max(target_weight) - block_weight.min(target_weight);

        // defensive only, a test case assures that the maximum weight diff can fit in Multiplier
        // without any saturation.
        let diff = Multiplier::saturating_from_rational(diff_abs, normal_max_weight.max(1));
        let diff_squared = diff.saturating_mul(diff);

        let v_squared_2 = v.saturating_mul(v) / Multiplier::saturating_from_integer(2);

        let first_term = v.saturating_mul(diff);
        let second_term = v_squared_2.saturating_mul(diff_squared);

        if positive {
            let excess = first_term
                .saturating_add(second_term)
                .saturating_mul(previous);
            previous.saturating_add(excess).max(min_multiplier)
        } else {
            // Defensive-only: first_term > second_term. Safe subtraction.
            let negative = first_term
                .saturating_sub(second_term)
                .saturating_mul(previous);
            previous.saturating_sub(negative).max(min_multiplier)
        }
    }
}

pub trait Trait: frame_system::Trait {
    /// The currency type in which fees will be paid.
    type Currency: Currency<Self::AccountId> + Send + Sync;

    /// Handler for the unbalanced reduction when taking transaction fees.
    type OnTransactionPayment: OnUnbalanced<NegativeImbalanceOf<Self>>;

    /// The fee to be paid for making a transaction; the per-byte portion.
    type TransactionByteFee: Get<BalanceOf<Self>>;

    /// Convert a weight value into a deductible fee based on the currency type.
    type WeightToFee: WeightToFeePolynomial<Balance = BalanceOf<Self>>;

    /// Update the multiplier of the next block, based on the previous block's weight.
    type FeeMultiplierUpdate: MultiplierUpdate;

    // Polymesh note: This was specifically added for Polymesh
    /// Fetch the signatory to charge fee from. Also sets fee payer and identity in context.
    type CddHandler: CddAndFeeDetails<Self::AccountId, Self::Call>;
}

decl_storage! {
    trait Store for Module<T: Trait> as TransactionPayment {
        pub NextFeeMultiplier get(fn next_fee_multiplier): Multiplier = Multiplier::saturating_from_integer(1);
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        /// The fee to be paid for making a transaction; the per-byte portion.
        const TransactionByteFee: BalanceOf<T> = T::TransactionByteFee::get();

        /// The polynomial that is applied in order to derive fee from weight.
        const WeightToFee: Vec<WeightToFeeCoefficient<BalanceOf<T>>> =
            T::WeightToFee::polynomial().to_vec();

         // Polymesh specific change: Fee multiplier update has been disabled for the testnet.

        fn integrity_test() {
            // given weight == u64, we build multipliers from `diff` of two weight values, which can
            // at most be MaximumBlockWeight. Make sure that this can fit in a multiplier without
            // loss.
            use sp_std::convert::TryInto;
            assert!(
                <Multiplier as sp_runtime::traits::Bounded>::max_value() >=
                Multiplier::checked_from_integer(
                    <T as frame_system::Trait>::MaximumBlockWeight::get().try_into().unwrap()
                ).unwrap(),
            );

            // This is the minimum value of the multiplier. Make sure that if we collapse to this
            // value, we can recover with a reasonable amount of traffic. For this test we assert
            // that if we collapse to minimum, the trend will be positive with a weight value
            // which is 1% more than the target.
            let min_value = T::FeeMultiplierUpdate::min();
            let mut target =
                T::FeeMultiplierUpdate::target() *
                (T::AvailableBlockRatio::get() * T::MaximumBlockWeight::get());

            // add 1 percent;
            let addition = target / 100;
            if addition == 0 {
                // this is most likely because in a test setup we set everything to ().
                return;
            }
            target += addition;

            sp_io::TestExternalities::new_empty().execute_with(|| {
                <frame_system::Module<T>>::set_block_limits(target, 0);
                let next = T::FeeMultiplierUpdate::convert(min_value);
                assert!(next > min_value, "The minimum bound of the multiplier is too low. When \
                    block saturation is more than target by 1% and multiplier is minimal then \
                    the multiplier doesn't increase."
                );
            })
        }
    }
}

impl<T: Trait> Module<T>
where
    BalanceOf<T>: FixedPointOperand,
{
    /// Query the data that we know about the fee of a given `call`.
    ///
    /// As this module is not and cannot be aware of the internals of a signed extension, it only
    /// interprets the extrinsic as some encoded value and accounts for its weight
    /// and length, the runtime's extrinsic base weight, and the current fee multiplier.
    ///
    /// All dispatchables must be annotated with weight and will have some fee info. This function
    /// always returns.
    pub fn query_info<Extrinsic: GetDispatchInfo>(
        unchecked_extrinsic: Extrinsic,
        len: u32,
    ) -> RuntimeDispatchInfo<BalanceOf<T>>
    where
        T: Send + Sync,
        BalanceOf<T>: Send + Sync,
        T::Call: Dispatchable<Info = DispatchInfo>,
    {
        // NOTE: we can actually make it understand `ChargeTransactionPayment`, but would be some hassle
        // for sure. We have to make it aware of the index of `ChargeTransactionPayment` in `Extra`.
        // Alternatively, we could actually execute the tx's per-dispatch and record the balance of the
        // sender before and after the pipeline.. but this is way too much hassle for a very very little
        // potential gain in the future.
        let dispatch_info = <Extrinsic as GetDispatchInfo>::get_dispatch_info(&unchecked_extrinsic);

        let partial_fee = Self::compute_fee(len, &dispatch_info, 0u32.into());
        let DispatchInfo { weight, class, .. } = dispatch_info;

        RuntimeDispatchInfo {
            weight,
            class,
            partial_fee,
        }
    }

    /// Compute the final fee value for a particular transaction.
    ///
    /// The final fee is composed of:
    ///   - `base_fee`: This is the minimum amount a user pays for a transaction. It is declared
    ///     as a base _weight_ in the runtime and converted to a fee using `WeightToFee`.
    ///   - `len_fee`: The length fee, the amount paid for the encoded length (in bytes) of the
    ///     transaction.
    ///   - `weight_fee`: This amount is computed based on the weight of the transaction. Weight
    ///     accounts for the execution time of a transaction.
    ///   - `targeted_fee_adjustment`: This is a multiplier that can tune the final fee based on
    ///     the congestion of the network.
    ///   - (Optional) `tip`: If included in the transaction, the tip will be added on top. Only
    ///     signed transactions can have a tip.Although it will always be zero in Polymesh, keeping
    ///     the tip as parameter to reduce the change in the apis.
    ///
    /// The base fee and adjusted weight and length fees constitute the _inclusion fee,_ which is
    /// the minimum fee for a transaction to be included in a block.
    ///
    /// ```ignore
    /// inclusion_fee = base_fee + len_fee + [targeted_fee_adjustment * weight_fee];
    /// final_fee = inclusion_fee + tip;
    /// ```
    pub fn compute_fee(len: u32, info: &DispatchInfoOf<T::Call>, tip: BalanceOf<T>) -> BalanceOf<T>
    where
        T::Call: Dispatchable<Info = DispatchInfo>,
    {
        Self::compute_fee_raw(len, info.weight, tip, info.pays_fee)
    }

    /// Compute the actual post dispatch fee for a particular transaction.
    ///
    /// Identical to `compute_fee` with the only difference that the post dispatch corrected
    /// weight is used for the weight fee calculation.
    pub fn compute_actual_fee(
        len: u32,
        info: &DispatchInfoOf<T::Call>,
        post_info: &PostDispatchInfoOf<T::Call>,
        tip: BalanceOf<T>,
    ) -> BalanceOf<T>
    where
        T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    {
        Self::compute_fee_raw(
            len,
            post_info.calc_actual_weight(info),
            tip,
            post_info.pays_fee(info),
        )
    }

    fn compute_fee_raw(
        len: u32,
        weight: Weight,
        tip: BalanceOf<T>,
        pays_fee: Pays,
    ) -> BalanceOf<T> {
        if pays_fee == Pays::Yes {
            let len = <BalanceOf<T>>::from(len);
            let per_byte = T::TransactionByteFee::get();

            // length fee. this is not adjusted.
            let fixed_len_fee = per_byte.saturating_mul(len);

            // the adjustable part of the fee.
            let unadjusted_weight_fee = Self::weight_to_fee(weight);
            let multiplier = Self::next_fee_multiplier();

            // final adjusted weight fee.
            let adjusted_weight_fee = multiplier.saturating_mul_int(unadjusted_weight_fee);

            let base_fee = Self::weight_to_fee(T::ExtrinsicBaseWeight::get());
            base_fee
                .saturating_add(fixed_len_fee)
                .saturating_add(adjusted_weight_fee)
                .saturating_add(tip)
        } else {
            tip
        }
    }

    fn weight_to_fee(weight: Weight) -> BalanceOf<T> {
        // cap the weight to the maximum defined in runtime, otherwise it will be the
        // `Bounded` maximum of its data type, which is not desired.
        let capped_weight = weight.min(<T as frame_system::Trait>::MaximumBlockWeight::get());
        T::WeightToFee::calc(&capped_weight)
    }

    /// Polymesh-Note :- Change for the supporting the test
    #[cfg(debug_assertions)]
    pub fn put_next_fee_multiplier(m: Multiplier) {
        NextFeeMultiplier::put(m)
    }
}

impl<T> Convert<Weight, BalanceOf<T>> for Module<T>
where
    T: Trait,
    BalanceOf<T>: FixedPointOperand,
{
    /// Compute the fee for the specified weight.
    ///
    /// This fee is already adjusted by the per block fee adjustment factor and is therefore the
    /// share that the weight contributes to the overall fee of a transaction. It is mainly
    /// for informational purposes and not used in the actual fee calculation.
    fn convert(weight: Weight) -> BalanceOf<T> {
        NextFeeMultiplier::get().saturating_mul_int(Self::weight_to_fee(weight))
    }
}

/// Require the transactor pay for themselves and maybe include a tip to gain additional priority
/// in the queue.
#[derive(Encode, Decode, Clone, Eq, PartialEq)]
pub struct ChargeTransactionPayment<T: Trait + Send + Sync>(#[codec(compact)] BalanceOf<T>);

impl<T: Trait + Send + Sync> ChargeTransactionPayment<T>
where
    T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    BalanceOf<T>: Send + Sync + FixedPointOperand,
{
    /// utility constructor. Used only in client/factory code.
    pub fn from(fee: BalanceOf<T>) -> Self {
        Self(fee)
    }

    fn withdraw_fee(
        &self,
        call: &T::Call,
        who: &T::AccountId,
        info: &DispatchInfoOf<T::Call>,
        len: usize,
    ) -> Result<(BalanceOf<T>, Option<NegativeImbalanceOf<T>>), TransactionValidityError> {
        let fee = Module::<T>::compute_fee(len as u32, info, self.0);

        // Only mess with balances if fee is not zero.
        if fee.is_zero() {
            return Ok((fee, None));
        }

        let payer_key =
            T::CddHandler::get_valid_payer(call, &who)?.ok_or(InvalidTransaction::Payment)?;

        let imbalance = T::Currency::withdraw(
            &payer_key,
            fee,
            WithdrawReason::TransactionPayment.into(),
            ExistenceRequirement::KeepAlive,
        )
        .map_err(|_| InvalidTransaction::Payment)?;
        T::CddHandler::set_payer_context(Some(payer_key));
        Ok((fee, Some(imbalance)))
    }

    /// Tip in `DispatchClass::Normal` transactions MUST be zero.
    fn ensure_valid_tip(
        &self,
        info: &DispatchInfoOf<T::Call>,
    ) -> Result<BalanceOf<T>, TransactionValidityError> {
        if let DispatchClass::Normal = info.class {
            ensure!(
                self.0 == Zero::zero(),
                TransactionValidityError::Invalid(InvalidTransaction::Custom(
                    TransactionError::ZeroTip as u8
                )),
            );
        }

        Ok(self.0)
    }
}

impl<T: Trait + Send + Sync> sp_std::fmt::Debug for ChargeTransactionPayment<T> {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        write!(f, "ChargeTransactionPayment<{:?}>", self.0)
    }
    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        Ok(())
    }
}

impl<T: Trait + Send + Sync> SignedExtension for ChargeTransactionPayment<T>
where
    BalanceOf<T>: Send + Sync + From<u64> + FixedPointOperand,
    T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
    const IDENTIFIER: &'static str = "ChargeTransactionPayment";
    type AccountId = T::AccountId;
    type Call = T::Call;
    type AdditionalSigned = ();
    type Pre = (
        BalanceOf<T>,
        Self::AccountId,
        Option<NegativeImbalanceOf<T>>,
        BalanceOf<T>,
    );
    fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
        Ok(())
    }

    // Polymesh note: Almost all of this function was re written to enforce zero tip and charge fee to proper payer.
    fn validate(
        &self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> TransactionValidity {
        let tip = self.ensure_valid_tip(info)?;

        let (_fee, _) = self.withdraw_fee(call, who, info, len)?;
        let mut r = ValidTransaction::default();
        // NOTE: The priority of TX is just its `tip`, to ensure that operational one can be
        // priorized and normal TX will follow the FIFO (defined by its `insertion_id`).
        r.priority = tip.saturated_into::<TransactionPriority>();
        Ok(r)
    }

    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        let tip = self.ensure_valid_tip(info)?;
        let (fee, imbalance) = self.withdraw_fee(call, who, info, len)?;
        Ok((tip, who.clone(), imbalance, fee))
    }

    fn post_dispatch(
        pre: Self::Pre,
        info: &DispatchInfoOf<Self::Call>,
        post_info: &PostDispatchInfoOf<Self::Call>,
        len: usize,
        _result: &DispatchResult,
    ) -> Result<(), TransactionValidityError> {
        let (tip, _who, imbalance, fee) = pre;
        if let Some(payed) = imbalance {
            let actual_fee = Module::<T>::compute_actual_fee(len as u32, info, post_info, tip);
            let refund = fee.saturating_sub(actual_fee);
            if let Some(payer_account) = T::CddHandler::get_payer_from_context() {
                let actual_payment =
                    match T::Currency::deposit_into_existing(&payer_account, refund) {
                        // The refund cannot be larger than the up front payed max weight.
                        // `PostDispatchInfo::calc_unspent` guards against such a case.
                        Ok(refund_imbalance) => payed
                            .offset(refund_imbalance)
                            .map_err(|_| InvalidTransaction::Payment)?,
                        // We do not recreate the account using the refund. The up front payment
                        // is gone in that case.
                        Err(_) => payed,
                    };
                T::OnTransactionPayment::on_unbalanced(actual_payment);
            }
        }
        // It clears the identity and payer in the context after transaction.
        T::CddHandler::clear_context();
        Ok(())
    }
}

// Polymesh note: This was specifically added for Polymesh
impl<T: Trait> ChargeTxFee for Module<T>
where
    BalanceOf<T>: FixedPointOperand,
    T::Call: Dispatchable<Info = DispatchInfo>,
{
    fn charge_fee(len: u32, info: DispatchInfoOf<T::Call>) -> TransactionValidity {
        let fee = Self::compute_fee(len as u32, &info, 0u32.into());
        if let Some(account) = T::CddHandler::get_payer_from_context() {
            let imbalance = T::Currency::withdraw(
                &account,
                fee,
                WithdrawReason::TransactionPayment.into(),
                ExistenceRequirement::KeepAlive,
            )
            .map_err(|_| InvalidTransaction::Payment)?;
            T::OnTransactionPayment::on_unbalanced(imbalance);
        }
        Ok(ValidTransaction::default())
    }
}
