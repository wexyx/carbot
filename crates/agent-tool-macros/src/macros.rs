mod tool;

use proc_macro::TokenStream;

/// Generate the Tool adapter and inventory constructor from an inherent impl.
#[proc_macro_attribute]
pub fn tool(args: TokenStream, item: TokenStream) -> TokenStream {
    tool::expand(args.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
