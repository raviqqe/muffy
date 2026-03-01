//! Macros for document validation.

use proc_macro::TokenStream;

/// Generates HTML validation functions.
#[proc_macro]
pub fn validation() -> TokenStream {
    // TODO Generate validation functions of `validate_element` for HTML against the `Element` type
    // in the `muffy-document` crate.

    quote! {}
}
