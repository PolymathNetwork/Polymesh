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

//! Defines a trait and implementations for storage migration.

use codec::{Decode, Encode};
use frame_support::migration::{put_storage_value, StorageIterator, StorageKeyIterator};
use frame_support::storage::unhashed::kill_prefix;
use frame_support::{ReversibleStorageHasher, StorageHasher, Twox128};
use sp_std::vec::Vec;

/// A migration error type.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub enum MigrationError<T> {
    /// Error during decodification of raw key.
    DecodeKey(Vec<u8>),
    /// Wrapper of the Error in the map function.
    Map(T),
}

/// A data type which is migrating through `migrate` to a new type as defined by `Into`.
pub trait Migrate: Decode {
    /// The new type to migrate into.
    type Into: Encode;

    /// An external context / data source to feed into the migration process.
    /// This could e.g., be per key, or some sort of global data.
    type Context;

    /// Migrate the current type to `Into` if possible,
    /// using the given `context` as an external data source.
    ///
    /// For simplicity, we assume that migrations are fallible in the type system,
    /// although they may in fact not be for certain data types.
    fn migrate(self, context: Self::Context) -> Option<Self::Into>;
}

/// The empty context, used by default unless a context is specified.
#[derive(Copy, Clone, Default)]
pub struct Empty;

impl<T: Migrate> Migrate for Vec<T>
where
    T::Context: Clone,
{
    type Into = Vec<T::Into>;
    type Context = T::Context;

    fn migrate(self, context: Self::Context) -> Option<Self::Into> {
        // Heuristic: let's assume migration is successful for everything.
        let mut vec = Vec::with_capacity(self.len());
        for old in self {
            vec.push(old.migrate(context.clone())?);
        }
        Some(vec)
    }
}

impl<T: Migrate> Migrate for Option<T> {
    type Into = Option<T::Into>;
    type Context = T::Context;

    fn migrate(self, context: Self::Context) -> Option<Self::Into> {
        match self {
            None => None,
            Some(val) => Some(val.migrate(context)),
        }
    }
}

/// Migrate the values with old type `T` in `module::item` to `T::Into`.
///
/// Migrations resulting in `old.migrate() == None` are silently dropped from storage.
pub fn migrate_map<T: Migrate, C: FnMut(&[u8]) -> T::Context>(
    module: &[u8],
    item: &[u8],
    derive_context: C,
) {
    migrate_map_rename::<T, C>(module, module, item, item, derive_context)
}

/// Migrate the values with old type `T` in `module::item` to `T::Into` in `module::new_item`.
///
/// Migrations resulting in `old.migrate() == None` are silently dropped from storage.
pub fn migrate_map_rename<T: Migrate, C: FnMut(&[u8]) -> T::Context>(
    module: &[u8],
    new_module: &[u8],
    item: &[u8],
    new_item: &[u8],
    mut derive_context: C,
) {
    StorageIterator::<T>::new(module, item)
        .drain()
        .filter_map(|(key, old)| {
            let new = old.migrate(derive_context(&key))?;
            Some((key, new))
        })
        .for_each(|(key, new)| put_storage_value(new_module, new_item, &key, new));
}

/// Migrate the key & value of a map `KO, VO` to key & value of type `KN, VN` via `map`.
/// The map is located in `module::item` and the new map will migrate to `module::new_item`.
/// This assumes that the hashers are all the same, which is the common case.
pub fn migrate_map_keys_and_value<VO, VN, H, KO, KN, F>(
    module: &[u8],
    item: &[u8],
    new_item: &[u8],
    mut map: F,
) where
    F: FnMut(KO, VO) -> Option<(KN, VN)>,
    VO: Decode + Encode,
    VN: Encode + Decode,
    H: ReversibleStorageHasher,
    KO: Decode,
    KN: Decode + Encode,
{
    StorageKeyIterator::<KO, VO, H>::new(module, item)
        .drain()
        .filter_map(|(key, val)| map(key, val))
        .for_each(|(kn, vn)| {
            let kn = kn.using_encoded(H::hash);
            let kn = kn.as_ref();
            put_storage_value(module, new_item, &kn, vn);
        })
}

/// Decode, if possible, the keys of a double map,
/// with K1 (hashed with `H1`) & K2 (hashed with `H2`) as keys from `raw`.
pub fn decode_double_key<
    H1: ReversibleStorageHasher,
    K1: Decode,
    H2: ReversibleStorageHasher,
    K2: Decode,
>(
    raw: &[u8],
) -> Option<(K1, K2)> {
    let mut unhashed_key = H1::reverse(&raw);
    let k1 = K1::decode(&mut unhashed_key).ok()?;
    let mut raw_k2 = H2::reverse(unhashed_key);
    let k2 = K2::decode(&mut raw_k2).ok()?;
    Some((k1, k2))
}

/// Encode keys of a double map `k1` (using hasher `H1) & `k2` (using hasher `H2`).
pub fn encode_double_key<H1: StorageHasher, K1: Encode, H2: StorageHasher, K2: Encode>(
    k1: K1,
    k2: K2,
) -> Vec<u8> {
    let k1 = k1.using_encoded(H1::hash);
    let k2 = k2.using_encoded(H2::hash);
    let k1 = k1.as_ref();
    let k2 = k2.as_ref();
    let mut key = Vec::with_capacity(k1.len() + k2.len());
    key.extend_from_slice(k1);
    key.extend_from_slice(k2);
    key
}

/// Migrate the keys and values of a double map `K1, K2` + `V1` to
/// keys and values of type `KN1, KN2` + `V2` via `map`.
/// The `map` function may fail, in which entries are dropped silently.
/// The double map is located in `module::item`.
/// `H1` and `H2` are the hashers used with `K1` and `K2` respectively.
pub fn migrate_double_map<V1, V2, H1, K1, H2, K2, KN1, KN2, F>(
    module: &[u8],
    item: &[u8],
    mut map: F,
) where
    F: FnMut(K1, K2, V1) -> Option<(KN1, KN2, V2)>,
    V1: Decode,
    V2: Encode,
    H1: ReversibleStorageHasher,
    H2: ReversibleStorageHasher,
    K1: Decode,
    K2: Decode,
    KN1: Encode,
    KN2: Encode,
{
    let old_map = StorageIterator::<V1>::new(module, item)
        .drain()
        .collect::<Vec<(Vec<u8>, _)>>();

    for (key, value) in old_map.into_iter().filter_map(|(raw_key, value)| {
        let (k1, k2) = decode_double_key::<H1, _, H2, _>(&raw_key)?;
        let (kn1, kn2, value) = map(k1, k2, value)?;
        Some((encode_double_key::<H1, _, H2, _>(kn1, kn2), value))
    }) {
        put_storage_value(module, item, &key, value);
    }
}

/// Migrate the values of a double map indexed by keys `K1` and `K2`.
/// The `map` function transform previous value `V1` into a `V2`.
/// The double map is located in `module::item`.
/// `H1` and `H2` are the hashers used with `K1` and `K2` respectively.
///
/// It is an optimization to avoid `collect` and it also allows the caller to manage errors during
/// the migration.
pub fn migrate_double_map_only_values<'a, V1, V2, H1, K1, H2, K2, F, E>(
    module: &'a [u8],
    item: &'a [u8],
    f: F,
) -> impl 'a + Iterator<Item = Result<(), MigrationError<E>>>
where
    F: 'a + Fn(K1, K2, V1) -> Result<V2, E>,
    K1: Decode,
    K2: Decode,
    H1: ReversibleStorageHasher,
    H2: ReversibleStorageHasher,
    V1: 'a + Decode,
    V2: Encode,
    E: Encode + Decode,
{
    StorageIterator::<V1>::new(module, item).map(move |(raw_key, value)| {
        let (k1, k2) = decode_double_key::<H1, K1, H2, K2>(&raw_key)
            .ok_or_else(|| MigrationError::DecodeKey(raw_key.clone().into()))?;
        let new_value = f(k1, k2, value).map_err(|e| MigrationError::Map(e))?;
        put_storage_value(module, item, &raw_key, new_value);

        Ok(())
    })
}

/// Kill a storage item from a module
pub fn kill_item(module: &[u8], item: &[u8]) {
    let mut prefix = [0u8; 32];
    prefix[0..16].copy_from_slice(&Twox128::hash(module));
    prefix[16..32].copy_from_slice(&Twox128::hash(item));
    kill_prefix(&prefix)
}

/// Moves a single or double map storage item under a new module prefix and removes the map from
/// the old module prefix.
///
/// Migrations mapping to `None` are silently dropped from storage.
pub fn move_map_rename_module<T: Decode + Encode>(
    old_module: &[u8],
    new_module: &[u8],
    item: &[u8],
) {
    StorageIterator::<T>::new(old_module, item)
        .drain()
        .filter_map(|(key, val)| Some((key, val)))
        .for_each(|(key, val)| put_storage_value(new_module, item, &key, val));
}
