use proc_macro2::TokenStream as TokenStream2;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned as _;
use syn::{
    visit_mut, Attribute, Data, DataEnum, DataStruct, DeriveInput, Expr, Ident, Index, Type,
    Variant,
};

/// Appends `Old` suffix to the `ident` returning the new appended `Ident`.
fn oldify_ident(ident: &Ident) -> Ident {
    Ident::new(&format!("{}Old", ident), ident.span())
}

/// Appends `Old` suffix to those identifiers in `ty` as instructed by `refs`.
fn oldify_type(ty: &mut Type, refs: Option<MigrateRefs>) {
    let refs = match refs {
        None => return,
        Some(MigrateRefs::Any) => None,
        Some(MigrateRefs::Listed(list)) => Some(list),
        Some(MigrateRefs::Exact(old_ty)) => {
            *ty = old_ty;
            return;
        }
    };
    struct Vis(Option<Vec<Ident>>);
    impl visit_mut::VisitMut for Vis {
        fn visit_ident_mut(&mut self, ident: &mut Ident) {
            match &self.0 {
                // Got `#[migrate]`; instructed oldify any identifier.
                None => {}
                // Got `#[migrate(TypeA, TypeB, ...)]`, so check if `ident` is one of those.
                Some(list)
                    if {
                        let ident = ident.to_string();
                        list.iter().any(|elem| elem == &ident)
                    } => {}
                _ => return,
            }
            *ident = oldify_ident(ident);
        }
    }
    visit_mut::visit_type_mut(&mut Vis(refs), ty);
}

/// Go over the given `fields`,
/// stripping any `#[migrate]` attributes on them (while noting them),
/// and then extending the fields in the output with the presence of `#[migrate]`.
fn fields_with_migration(fields: &mut syn::Fields) -> Vec<(bool, Option<Expr>, &syn::Field)> {
    let mut fields_vec = Vec::with_capacity(fields.len());
    for f in fields.iter_mut() {
        let refs = extract_migrate_refs(&mut f.attrs);
        let with = extract_parse_attr::<Expr>(&mut f.attrs, "migrate_with");
        let has_migrate = refs.is_some();
        oldify_type(&mut f.ty, refs);
        fields_vec.push((has_migrate, with, &*f));
    }
    fields_vec
}

/// Quote the given `fields`, assumed to be a variant/product,
/// into a pair of a destructuring (unpacking) pattern
/// and a piece of a struct/variant initialization expression.
fn quote_pack_unpack(
    fields: &[(bool, Option<Expr>, &syn::Field)],
) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    fn pack(
        with: &Option<Expr>,
        migrate: &bool,
        field: impl quote::ToTokens,
        var: &Ident,
    ) -> TokenStream2 {
        match (with, migrate) {
            (Some(with), _) => quote!( #field: #with ),
            (None, true) => quote!( #field: #var.migrate(context.clone().into())? ),
            (None, false) => quote!( #field: #var ),
        }
    }
    fields
        .iter()
        .enumerate()
        .map(|(index, (migrate, with, field))| match &field.ident {
            Some(ident) => (quote!( #ident ), pack(with, migrate, &ident, &ident)),
            None => {
                let span = field.ty.span();
                let index = index as u32;
                let idx = Index { index, span };
                let var = Ident::new(&format!("idx{}", index), span);
                (quote!( #idx: #var ), pack(with, migrate, idx, &var))
            }
        })
        .unzip()
}

/// Implements `#[derive(Migrate)]`.
pub(crate) fn impl_migrate(mut input: DeriveInput) -> TokenStream2 {
    let name = input.ident;
    input.ident = oldify_ident(&name);
    let old_name = &input.ident;

    // Extract the `Context` type. If it's not available, use `Empty`.
    // Also ensure that there's a `From<Self::Context> for Empty` implementation available.
    let context = match extract_parse_attr::<Type>(&mut input.attrs, "migrate_context") {
        None => quote!(polymesh_primitives::migrate::Empty),
        Some(ty) => quote!( #ty ),
    };

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let migration = match &mut input.data {
        Data::Union(_) => {
            return quote! {
                compile_error!("cannot derive `Migrate` for unions");
            }
        }
        Data::Struct(DataStruct { ref mut fields, .. }) => {
            // Handle `#[migrate]`s and old-ifying types
            // and then interpolate unpacking & packing.
            let (unpack, pack) = quote_pack_unpack(&fields_with_migration(fields));
            quote!( match self { Self { #(#unpack,)* } => Self::Into { #(#pack,)* } } )
        }
        Data::Enum(DataEnum {
            ref mut variants, ..
        }) => {
            // Same for each variant, as-if it were a struct.
            let arm = variants.iter_mut().map(|variant| {
                let new_ident = variant.ident.clone();
                let ((unpack, pack), old_ident) =
                    match extract_parse_attr::<Variant>(&mut variant.attrs, "migrate_from") {
                        Some(mut old_var) => {
                            let (unpack, _) =
                                quote_pack_unpack(&fields_with_migration(&mut old_var.fields));
                            let (_, pack) =
                                quote_pack_unpack(&fields_with_migration(&mut variant.fields));
                            let pair = (unpack, pack);
                            *variant = old_var;
                            (pair, &variant.ident)
                        }
                        None => (
                            quote_pack_unpack(&fields_with_migration(&mut variant.fields)),
                            &new_ident,
                        ),
                    };
                quote!( Self::#new_ident { #(#unpack,)* } => Self::Into::#old_ident { #(#pack,)* } )
            });
            quote!( match self { #(#arm,)* } )
        }
    };

    quote! {
        #[derive(::codec::Decode)]
        #input

        impl #impl_generics polymesh_primitives::migrate::Migrate
        for #old_name #ty_generics
        #where_clause {
            type Into = #name #ty_generics;
            type Context = #context;
            #[allow(unused_variables)]
            fn migrate(self, context: Self::Context) -> Option<Self::Into> { Some(#migration) }
        }
    }
}

/// Semantic representation of the `#[migrate]` attribute.
enum MigrateRefs {
    /// Derived from `#[migrate]`.
    /// Any identifier in the type of the field should be migrated.
    Any,
    /// Derived from `#[migrate(ident, ident, ...)]`.
    /// Only those identifiers in the list and which match in the type of the field should be migrated.
    Listed(Vec<Ident>),
    /// Do an exact replacement of the field type with the one given in `#[migrate_from(Type)]`.
    Exact(Type),
}

/// Returns information about any `#[migrate]` or `#[migrate_from]` attributes.
/// We also strip those attributes while at it.
///
/// The form `#[migrate = ".."]` does qualify.
fn extract_migrate_refs(attrs: &mut Vec<Attribute>) -> Option<MigrateRefs> {
    find_strip_attr(attrs, |attr| {
        // Only care about `migrate{_from}`, and remove all of those, irrespective of form.
        let ident_str = attr.path.get_ident().map(|i| i.to_string());
        Some(match ident_str.as_deref() {
            // Got exactly `#[migrate]`.
            // User doesn't wish to specify which types to migrate, so assume all.
            Some("migrate") if attr.tokens.is_empty() => MigrateRefs::Any,
            // Got `#[migrate(ident, ident, ...)]` or maybe `#[migrate = "..."]`.
            Some("migrate") => {
                MigrateRefs::Listed(
                    attr.parse_args_with(|ps: ParseStream| {
                        // User only wants to oldify the given identifiers.
                        // Applies in e.g., `field: Vec<Foo>` where `Foo` is being migrated
                        // but `Vec` shouldn't be renamed as it is a container of `Foo`s.
                        ps.parse_terminated::<_, syn::Token![,]>(Ident::parse)
                            .map(|iter| iter.into_iter().collect())
                    })
                    .unwrap(),
                )
            }
            // Expect and parse `#[migrate_from($ty)]`.
            Some("migrate_from") => MigrateRefs::Exact(attr.parse_args().unwrap()),
            _ => return None,
        })
    })
}

fn extract_parse_attr<T: syn::parse::Parse>(attrs: &mut Vec<Attribute>, name: &str) -> Option<T> {
    find_strip_attr(attrs, |attr| {
        attr.path.is_ident(name).then(|| attr.parse_args().unwrap())
    })
}

/// Execute mapping predicate `find` on all the `attrs`.
/// The finding of the last match will be returned if any,
/// and all matching attributes are removed in `attrs`.
fn find_strip_attr<T>(
    attrs: &mut Vec<Attribute>,
    mut find: impl FnMut(&Attribute) -> Option<T>,
) -> Option<T> {
    let mut thing = None;
    attrs.retain(|attr| match find(attr) {
        None => true,
        x @ Some(_) => {
            thing = x;
            false
        }
    });
    thing
}
