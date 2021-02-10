use polymesh_primitives::IdentityId;

use frame_support::weights::{ClassifyDispatch, DispatchClass, Pays, PaysFee, WeighData, Weight};
use sp_std::{cmp::max, vec::Vec};

/// It supports fee calculation when a transaction is made in batch mode (for a group of items).
/// The total fee is maximum between:
///     - `per_item_weight` multiplied by number of items of one or more parameters.
///     - and `min_weight`. It ensures a cost if number of items is 0, or you want a minimum threshold.
///
pub struct BatchDispatchInfo {
    pub dispatch_type: DispatchClass,
    pub per_item_weight: Weight,
    pub min_weight: Weight,
}

impl BatchDispatchInfo {
    pub fn new_normal(per_item: Weight, min: Weight) -> Self {
        Self::new(DispatchClass::Normal, per_item, min)
    }

    pub fn new_operational(per_item: Weight, min: Weight) -> Self {
        Self::new(DispatchClass::Operational, per_item, min)
    }

    pub fn new(dispatch_type: DispatchClass, per_item_weight: Weight, min_weight: Weight) -> Self {
        BatchDispatchInfo {
            dispatch_type,
            per_item_weight,
            min_weight,
        }
    }
}

impl<T> ClassifyDispatch<T> for BatchDispatchInfo {
    fn classify_dispatch(&self, _: T) -> DispatchClass {
        self.dispatch_type
    }
}

impl<T> PaysFee<T> for BatchDispatchInfo {
    fn pays_fee(&self, _target: T) -> Pays {
        Pays::Yes
    }
}

/// It adds support to any function like `fn x( _: IdentityId, items: Vec<_>)
type IdentityAndVecParams<'a, T> = (&'a IdentityId, &'a Vec<T>);

impl<'a, T> WeighData<IdentityAndVecParams<'a, T>> for BatchDispatchInfo {
    /// The weight is calculated base on the number of elements of the second parameter of the
    /// call.
    fn weigh_data(&self, params: IdentityAndVecParams<'a, T>) -> Weight {
        max(
            self.min_weight,
            self.per_item_weight * params.1.len() as Weight,
        )
    }
}
