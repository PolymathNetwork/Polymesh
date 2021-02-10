use proc_macro::TokenStream;

/// Implements all utility method for *strong typed* based on `[u8]`.
pub(crate) fn impl_serialize_u8_strong_typed(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let gen = quote! {
        impl serde::Serialize for #name  {

            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.using_encoded(|bytes| sp_core::bytes::serialize(bytes, serializer))
            }
        }
    };

    gen.into()
}
