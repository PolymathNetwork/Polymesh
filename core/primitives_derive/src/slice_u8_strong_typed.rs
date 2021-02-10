use proc_macro::TokenStream;

/// Implements all utility method for *strong typed* based on `[u8]`.
pub(crate) fn impl_slice_u8_strong_typed(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let gen = quote! {
        impl #name {
            /// Returns the number of elements.
            #[inline]
            pub fn len(&self) -> usize {
                self.0.len()
            }

            /// Extracts a slice containing the entire content.
            #[inline]
            pub fn as_slice(&self) -> &[u8] {
                &self.0[..]
            }

            /// Returns an iterator over the internal [u8].
            #[inline]
            pub fn iter(&self) -> sp_std::slice::Iter<'_,u8> {
                self.0.iter()
            }

            /// Display as HEX data.
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                for byte in self.0.iter() {
                    f.write_fmt(format_args!("{:02x}", byte))?;
                }
                Ok(())
            }
        }

        impl From<&[u8]> for #name {
            fn from(s: &[u8]) -> Self {
                let mut v = #name::default();
                let limit = sp_std::cmp::min(v.len(), s.len());

                let inner = &mut v.0[..limit];
                inner.copy_from_slice( &s[..limit]);

                v
            }
        }

        impl From<&str> for #name {
            #[inline]
            fn from(s: &str) -> Self {
                Self::from(s.as_bytes())
            }
        }

        impl AsRef<[u8]> for #name {
            #[inline]
            fn as_ref(&self) -> &[u8] {
                &self.0[..]
            }
        }

        impl sp_std::ops::Deref for #name {
            type Target = [u8];

            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.0[..]
            }
        }

        impl sp_std::ops::DerefMut for #name {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0[..]
            }
        }

        impl core::fmt::Display for #name {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.fmt(f)
            }
        }

        impl sp_std::fmt::Debug for #name {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.fmt(f)
            }
        }
    };

    gen.into()
}
