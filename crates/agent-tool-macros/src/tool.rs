use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Expr, ImplItem, ItemImpl, LitStr, MetaNameValue, Path, Token, parse::Parser,
    punctuated::Punctuated,
};

pub(crate) fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let values = Punctuated::<MetaNameValue, Token![,]>::parse_terminated.parse2(args)?;
    let mut name: Option<LitStr> = None;
    let mut description: Option<LitStr> = None;
    let mut parameters: Option<Expr> = None;
    let mut runtime: Option<Path> = None;
    let mut scope: Option<LitStr> = None;
    let mut seen = std::collections::HashSet::new();
    for value in values {
        let key = value
            .path
            .get_ident()
            .ok_or_else(|| syn::Error::new_spanned(&value.path, "expected a tool option name"))?
            .to_string();
        if !seen.insert(key.clone()) {
            return Err(syn::Error::new_spanned(value, "duplicate tool option"));
        }
        // Parse values separately so malformed annotations report source spans.
        let expression = value.value;
        match key.as_str() {
            "name" => name = Some(syn::parse2(quote!(#expression))?),
            "description" => description = Some(syn::parse2(quote!(#expression))?),
            "parameters" => parameters = Some(expression),
            "runtime" => runtime = Some(syn::parse2(quote!(#expression))?),
            "scope" => scope = Some(syn::parse2(quote!(#expression))?),
            _ => return Err(syn::Error::new_spanned(value.path, "unknown tool option")),
        }
    }
    let name = name.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "tool requires name = \"...\"",
        )
    })?;
    let text = name.value();
    if text.is_empty()
        || text.len() > 64
        || !text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(syn::Error::new_spanned(
            &name,
            "tool name must be 1–64 ASCII letters, digits or underscores",
        ));
    }
    let description = description.unwrap_or_else(|| name.clone());
    let scope = scope.unwrap_or_else(|| syn::parse_quote!("business"));
    if !matches!(scope.value().as_str(), "business" | "management" | "shared") {
        return Err(syn::Error::new_spanned(
            scope,
            "scope must be business, management or shared",
        ));
    }
    let runtime = runtime.unwrap_or_else(|| syn::parse_quote!(::agent_runtime));
    let parameters = parameters
        .unwrap_or_else(|| syn::parse_quote!(#runtime::tools::json::json!({"type":"object"})));
    let implementation: ItemImpl = syn::parse2(item)?;
    if implementation.trait_.is_some()
        || !implementation.generics.params.is_empty()
        || implementation.generics.where_clause.is_some()
    {
        return Err(syn::Error::new_spanned(
            &implementation,
            "tool requires a non-generic inherent impl",
        ));
    }
    let methods = implementation
        .items
        .iter()
        .filter_map(|i| {
            if let ImplItem::Fn(f) = i {
                Some(f)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let constructor = methods
        .iter()
        .find(|f| f.sig.ident == "new")
        .ok_or_else(|| {
            syn::Error::new_spanned(
                &implementation,
                "tool requires fn new(context: Arc<ToolContext>) -> Option<Self>",
            )
        })?;
    if constructor.sig.asyncness.is_some()
        || constructor.sig.inputs.len() != 1
        || constructor.sig.receiver().is_some()
        || !constructor.sig.generics.params.is_empty()
    {
        return Err(syn::Error::new_spanned(
            &constructor.sig,
            "tool new must be synchronous with one context argument",
        ));
    }
    let execute=methods.iter().find(|f|f.sig.ident=="execute").ok_or_else(||syn::Error::new_spanned(&implementation,"tool requires async fn execute(&self, args: &Value, session: &mut ToolSession) -> Result<Value, String>"))?;
    if execute.sig.asyncness.is_none()
        || execute.sig.inputs.len() != 3
        || !execute
            .sig
            .receiver()
            .is_some_and(|r| r.reference.is_some() && r.mutability.is_none())
        || !execute.sig.generics.params.is_empty()
    {
        return Err(syn::Error::new_spanned(
            &execute.sig,
            "tool execute must be async with &self, arguments and session",
        ));
    }
    let ty = &implementation.self_ty;
    Ok(quote! {
        #implementation
        impl #runtime::tools::Tool for #ty {
            fn definition(&self) -> #runtime::tools::ToolDefinition {
                #runtime::tools::ToolDefinition::new(#name,#description,#parameters)
            }
            fn execute<'a>(&'a self, args: &'a #runtime::tools::json::Value, session: &'a mut #runtime::tools::ToolSession) -> #runtime::tools::ToolFuture<'a> {
                ::std::boxed::Box::pin(<#ty>::execute(self,args,session))
            }
        }
        #runtime::tools::registrations::submit! {
            #runtime::tools::ToolRegistration {
                scope: #scope,
                create: |context| {
                    <#ty>::new(context).map(|tool| ::std::sync::Arc::new(tool) as ::std::sync::Arc<dyn #runtime::tools::Tool>)
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn implementation() -> TokenStream {
        quote! {
            impl Example {
                fn new(context: Arc<ToolContext>)->Option<Self>{Some(Self{context})}
                async fn execute(&self,args:&Value,session:&mut ToolSession)->Result<Value,String>{Ok(args.clone())}
            }
        }
    }
    #[test]
    fn expands_constructor_and_adapter() {
        let result = expand(quote!(name = "example"), implementation()).unwrap();
        syn::parse2::<syn::File>(result.clone()).unwrap();
        let text = result.to_string();
        assert!(text.contains("submit"));
        assert!(text.contains("ToolRegistration"));
        assert!(text.contains("Box :: pin"));
    }
    #[test]
    fn rejects_invalid_annotations_and_signatures() {
        for args in [
            quote!(),
            quote!(name = "bad.name"),
            quote!(name = "x", name = "y"),
            quote!(name = "x", unknown = true),
            quote!(name = "x", scope = "unknown"),
        ] {
            assert!(expand(args, implementation()).is_err());
        }
        assert!(expand(quote!(name = "x"), quote!(impl Example {})).is_err());
        assert!(
            expand(
                quote!(name = "x"),
                quote!(
                    impl<T> Example<T> {}
                )
            )
            .is_err()
        );
        assert!(expand(quote!(name = "x"), quote!(impl Tool for Example {})).is_err());
        assert!(expand(quote!(name="x"),quote!(impl Example {
            fn new()->Self {Self}
            async fn execute(&self,args:&Value,session:&mut ToolSession)->Result<Value,String>{Ok(args.clone())}
        })).is_err());
    }
}
