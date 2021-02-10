#![feature(bool_to_option)]

extern crate proc_macro;
extern crate syn;

#[macro_use]
extern crate quote;

use proc_macro::TokenStream;

mod deserialize_u8_strong_typed;
mod migrate;
mod ristretto_strong_typed;
mod serialize_u8_strong_typed;
mod slice_u8_strong_typed;
mod vec_u8_strong_typed;

use crate::{
    deserialize_u8_strong_typed::impl_deserialize_u8_strong_typed, migrate::impl_migrate,
    ristretto_strong_typed::impl_ristretto_strong_typed,
    serialize_u8_strong_typed::impl_serialize_u8_strong_typed,
    slice_u8_strong_typed::impl_slice_u8_strong_typed,
    vec_u8_strong_typed::impl_vec_u8_strong_typed,
};

/// Implements all utility method for *strong typed* based on `RistrettoPoint` inner type.
#[proc_macro_derive(RistrettoStrongTyped)]
pub fn ristretto_strong_typed_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_ristretto_strong_typed(&ast)
}

/// Implements all utility method for *strong typed* based on `Vec<u8>` inner type.
#[proc_macro_derive(VecU8StrongTyped)]
pub fn vec_u8_strong_typed_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_vec_u8_strong_typed(&ast)
}

/// Implements all utility method for *strong typed* based on `[u8]` inner type.
#[proc_macro_derive(SliceU8StrongTyped)]
pub fn slice_u8_strong_typed_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_slice_u8_strong_typed(&ast)
}

/// Implements Serialize for `U8` strong typed types.
#[proc_macro_derive(SerializeU8StrongTyped)]
pub fn serialize_u8_strong_typed_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_serialize_u8_strong_typed(&ast)
}

/// Implements Deserialize for `U8` strong typed types.
#[proc_macro_derive(DeserializeU8StrongTyped)]
pub fn deserialize_u8_strong_typed_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_deserialize_u8_strong_typed(&ast)
}

/// Implements `Migrate` for types structurally.
///
/// For example:
/// ```rust
/// #[derive(..., Encode, Migrate)]
/// struct Foo {
///     #[migrate]
///     pub bar: Bar,
///     pub baz: Baz,
/// }
/// ```
///
/// This will implement `Migrate for FooOld` and migrate `bar` but not `baz`.
/// Additionally, it will also define `FooOld` for you using `BarOld` instead of `Bar`.
///
/// Another form of `#[migrate]` is `#[migrate(Foo, Bar)]` which on
/// `field: Vec<(Foo, Bar, Baz)>` will produce `field: Vec<(FooOld, BarOld, Baz)>`.
///
/// Additionally, you may specify `#[migrate_from(TypeToReplaceWithInOld)]`
/// which will perform an exact replacement of the type in the generated old type.
///
/// In the context of an enum variant, you can also specify `#[migrate_from(OldVariant)]`, e.g.,
/// ```rust
/// #[derive(..., Encode, Migrate)]
/// enum Foo {
///     #[migrate_from(OldVariantName(u8))]
///     Variant,
/// }
/// ```
///
/// When you want control over the expression used for migration,
/// you can use `#[migrate_with($expr)]` on a given field.
/// For example, to write a migration that simply increments, you may write:
/// ```rust
/// #[derive(..., Encode, Migrate)]
/// struct Foo {
///     #[migrate_with(bar + 1)]
///     bar: u8,
/// }
/// ```
///
/// By default, `#[derive(Migrate)]` will use `type Context = polymesh_primitives::migrate::Empty`.
/// This can be changed by specifying a different type with `#[migrate_context(TheContextType)]`,
/// in which case the macro expansion will include `type Context = TheContextType`.
/// The associated type `Context` specifies what external data source, if any, to pass to `migrate`.
/// For more on `Context`, see the `Migrate` trait's documentation.
#[proc_macro_derive(
    Migrate,
    attributes(migrate, migrate_from, migrate_context, migrate_with)
)]
pub fn migrate_derive(input: TokenStream) -> TokenStream {
    impl_migrate(syn::parse(input).unwrap()).into()
}
