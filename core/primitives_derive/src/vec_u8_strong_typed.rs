use proc_macro::TokenStream;

/// Implements utility method for *strong typed* based on `Vec<u8>`.
pub(crate) fn impl_vec_u8_strong_typed(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let gen = quote! {
        impl #name {

            /// Returns the number of elements.
            #[inline]
            pub fn len(&self) -> usize {
                self.0.len()
            }

            /// Extracts a slice containing the entire vector.
            #[inline]
            pub fn as_slice(&self) -> &[u8] {
                &self.0
            }

            /// Extracts the internal vector.
            #[inline]
            pub fn as_vec(&self) -> &Vec<u8> {
                &self.0
            }
        }

        impl<T: AsRef<[u8]>> From<T> for #name {
            #[inline]
            fn from(r: T) -> Self {
                #name(r.as_ref().to_vec())
            }
        }

        impl sp_std::ops::Deref for #name {
            type Target = [u8];

            #[inline]
            fn deref(&self) -> &Self::Target {
                self.0.deref()
            }
        }

        impl sp_std::ops::DerefMut for #name {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                self.0.deref_mut()
            }
        }
    };

    gen.into()
}
