use proc_macro::TokenStream;

/// Implements all utility method for *strong typed* based on `[u8]`.
pub(crate) fn impl_deserialize_u8_strong_typed(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let gen = quote! {

        impl<'de> serde::Deserialize<'de> for #name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let r = sp_core::bytes::deserialize(deserializer)?;
                Decode::decode(&mut &r[..])
                    .map_err(|e| serde::de::Error::custom(format!("Decode error: {}", e)))
            }
        }
    };

    gen.into()
}
