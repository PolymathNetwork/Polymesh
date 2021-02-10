use crate::*;

use frame_benchmarking::benchmarks;
use frame_support::ensure;
use polymesh_primitives::{DispatchableName, PalletName};
use sp_std::{iter, prelude::*};

const MAX_PALLET_NAME_LENGTH: u32 = 512;
const MAX_DISPATCHABLE_NAME_LENGTH: u32 = 1024;

fn make_name(m: u32) -> Vec<u8> {
    iter::repeat(b'x').take(m as usize).collect::<Vec<_>>()
}

benchmarks! {
    _ {}

    set_call_metadata {
        let pallet_name: PalletName = make_name(MAX_PALLET_NAME_LENGTH).into();
        let pallet_name_exp = pallet_name.clone();
        let dispatchable_name: DispatchableName = make_name(MAX_DISPATCHABLE_NAME_LENGTH).into();
        let dispatchable_name_exp = dispatchable_name.clone();

    }: {
        StoreCallMetadata::<T>::set_call_metadata(pallet_name, dispatchable_name);
    }
    verify {
        ensure!(CurrentPalletName::get() == pallet_name_exp, "Unexpected pallet name");
        ensure!(CurrentDispatchableName::get() == dispatchable_name_exp, "Unexpected dispatchable name");
    }

    clear_call_metadata {
        let pallet_name: PalletName = make_name(MAX_PALLET_NAME_LENGTH).into();
        let dispatchable_name: DispatchableName = make_name(MAX_DISPATCHABLE_NAME_LENGTH).into();
        StoreCallMetadata::<T>::set_call_metadata(pallet_name, dispatchable_name);
    }: {
        StoreCallMetadata::<T>::clear_call_metadata();
    }
    verify {
        ensure!(CurrentPalletName::exists() == false, "Pallet name should not be exist");
        ensure!(CurrentDispatchableName::exists() == false, "Dispatchable name should not be exist");
    }
}
