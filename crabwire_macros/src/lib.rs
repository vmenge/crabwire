use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::{format_ident, quote};
use syn::{
    Ident, ItemFn, Stmt, Token, Type, TypeReference,
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

    quote!(#item_fn).into()
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
