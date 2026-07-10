use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::{format_ident, quote};
use syn::{
    Attribute, Ident, ItemFn, LitStr, Stmt, Token, Type, TypeReference,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
};

struct InjectArgs {
    items: Punctuated<InjectArg, Token![,]>,
}

struct InjectArg {
    name: Ident,
    ty: Type,
}

impl Parse for InjectArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self {
            items: Punctuated::parse_terminated(input)?,
        })
    }
}

impl Parse for InjectArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty = input.parse()?;

        Ok(Self { name, ty })
    }
}

/// Inject dependencies from the global registry into a function.
///
/// Each argument must be a shared reference, such as `foo: &Foo`. The macro
/// fetches `Foo` from the global registry and binds it before your function
/// body runs.
///
/// ```rust,ignore
/// use crabwire::{Registry, inject, register};
///
/// struct Config {
///     name: &'static str,
/// }
///
/// #[inject(config: &Config)]
/// fn app_name() -> &'static str {
///     config.name
/// }
///
/// register!(Registry::new().insert(Config { name: "demo" }));
///
/// assert_eq!(app_name(), "demo");
/// ```
#[proc_macro_attribute]
pub fn inject(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InjectArgs);
    let mut item_fn = parse_macro_input!(item as ItemFn);

    let runtime = crabwire_path();

    let mut injected_stmts = Vec::<Stmt>::new();
    let mut injected_docs = Vec::<Attribute>::new();

    for arg in args.items {
        let name = arg.name;
        let ty = arg.ty;

        match &ty {
            Type::Reference(TypeReference {
                mutability: None,
                elem,
                ..
            }) => {
                injected_stmts.push(parse_quote! {
                    #[allow(clippy::borrowed_box)]
                    let #name: #ty = #runtime::macro_utils::global_get::<#elem>()
                        .unwrap_or_else(|error| panic!("{}", error));
                });

                let doc = LitStr::new(
                    &format!("- `{name}: {}`", format_type_for_docs(&ty)),
                    proc_macro2::Span::call_site(),
                );
                injected_docs.push(parse_quote!(#[doc = #doc]));
            }
            _ => {
                injected_stmts.push(parse_quote! {
                    compile_error!("crabwire #[inject] arguments must be shared references, for example #[inject(foo: &Foo)]");
                });
            }
        }
    }

    injected_stmts.append(&mut item_fn.block.stmts);
    item_fn.block.stmts = injected_stmts;

    if !injected_docs.is_empty() {
        let heading = LitStr::new("Injected dependencies:", proc_macro2::Span::call_site());
        let blank = LitStr::new("", proc_macro2::Span::call_site());

        let mut attrs = Vec::with_capacity(item_fn.attrs.len() + injected_docs.len() + 2);
        attrs.push(parse_quote!(#[doc = #heading]));
        attrs.append(&mut injected_docs);
        attrs.push(parse_quote!(#[doc = #blank]));
        attrs.append(&mut item_fn.attrs);
        item_fn.attrs = attrs;
    }

    quote!(#item_fn).into()
}

fn format_type_for_docs(ty: &Type) -> String {
    quote!(#ty)
        .to_string()
        .replace("& ", "&")
        .replace(" :: ", "::")
        .replace(" < ", "<")
        .replace(" >", ">")
        .replace(" ,", ",")
        .replace("< ", "<")
        .replace(" >", ">")
}

fn crabwire_path() -> proc_macro2::TokenStream {
    match crate_name("crabwire") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = format_ident!("{}", name);
            quote!(::#ident)
        }
        Err(_) => quote!(::crabwire),
    }
}

#[cfg(test)]
mod tests {
    use super::format_type_for_docs;
    use syn::{Type, parse_quote};

    #[test]
    fn formats_reference_types_without_extra_reference_whitespace() {
        let ty: Type = parse_quote!(&Config);

        assert_eq!(format_type_for_docs(&ty), "&Config");
    }

    #[test]
    fn formats_nested_reference_types_without_token_spacing_noise() {
        let ty: Type = parse_quote!(&Box<dyn Logger>);

        assert_eq!(format_type_for_docs(&ty), "&Box<dyn Logger>");
    }
}
