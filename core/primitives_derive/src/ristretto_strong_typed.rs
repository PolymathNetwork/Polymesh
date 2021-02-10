use proc_macro::TokenStream;

/// Implements all utility method for *strong typed* based on `[u8]`.
pub(crate) fn impl_ristretto_strong_typed(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let gen = quote! {
        impl #name {
            pub fn to_bytes(&self) -> [u8;32] {
                self.0.compress().to_bytes()
            }
        }
    };

    gen.into()
}
