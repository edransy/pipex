extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, Meta, Expr, Lit, parse::Parse, parse::ParseStream, Token};

/// A simple parser for our attribute arguments
struct AttributeArgs {
    args: Vec<Meta>,
}

impl Parse for AttributeArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = Vec::new();
        
        while !input.is_empty() {
            args.push(input.parse::<Meta>()?);
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        
        Ok(AttributeArgs { args })
    }
}

/// Parse common attributes like retry, timeout from attribute arguments
fn parse_common_attributes(attrs: &[Meta]) -> (Option<usize>, Option<u64>) {
    let mut retry_count = None;
    let mut timeout_ms = None;
    
    for attr in attrs {
        match attr {
            Meta::NameValue(nv) => {
                if nv.path.is_ident("retry") {
                    if let Expr::Lit(expr_lit) = &nv.value {
                        if let Lit::Int(lit_int) = &expr_lit.lit {
                            retry_count = Some(lit_int.base10_parse().unwrap_or(1));
                        }
                    }
                } else if nv.path.is_ident("timeout") {
                    if let Expr::Lit(expr_lit) = &nv.value {
                        if let Lit::Int(lit_int) = &expr_lit.lit {
                            timeout_ms = Some(lit_int.base10_parse().unwrap_or(5000));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    
    (retry_count, timeout_ms)
}

/// Parse attribute arguments in the form #[attr(key = value, key2 = value2)]
fn parse_attr_args(input: TokenStream) -> Vec<Meta> {
    if input.is_empty() {
        return vec![];
    }
    
    match syn::parse::<AttributeArgs>(input) {
        Ok(parsed) => parsed.args,
        Err(_) => vec![],
    }
}

/// Generate a configuration constant for a function
fn generate_function_with_config(
    input_fn: ItemFn, 
    strategy: &str, 
    retry_count: Option<usize>,
    timeout_ms: Option<u64>
) -> TokenStream {
    let fn_name = input_fn.sig.ident.to_string();
    let fn_ident = &input_fn.sig.ident;
    let fn_inputs = &input_fn.sig.inputs;
    let fn_output = &input_fn.sig.output;
    let fn_body = &input_fn.block;
    let fn_vis = &input_fn.vis;
    let fn_async = &input_fn.sig.asyncness;
    
    // Generate a unique configuration constant name
    let config_const_name = syn::Ident::new(
        &format!("__{}_PIPEX_CONFIG", fn_name.to_uppercase()), 
        proc_macro2::Span::call_site()
    );
    
    let retry_count = retry_count.unwrap_or(1);
    let timeout_ms = timeout_ms.unwrap_or(5000);
    
    let expanded = quote! {
        // Original function
        #fn_vis #fn_async fn #fn_ident(#fn_inputs) #fn_output #fn_body
        
        // Configuration constant that pipex! can read
        #[doc(hidden)]
        pub const #config_const_name: (&str, &str, usize, u64) = 
            (#fn_name, #strategy, #retry_count, #timeout_ms);
    };
    
    TokenStream::from(expanded)
}

/// #[pipex_ignore] - Mark function to use ignore error strategy
/// Usage: #[pipex_ignore] or #[pipex_ignore(retry = 3)] or #[pipex_ignore(retry = 3, timeout = 1000)]
#[proc_macro_attribute]
pub fn pipex_ignore(args: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let attrs = parse_attr_args(args);
    
    let (retry_count, timeout_ms) = parse_common_attributes(&attrs);
    
    generate_function_with_config(input_fn, "ignore", retry_count, timeout_ms)
}

/// #[pipex_collect] - Mark function to use collect error strategy  
/// Usage: #[pipex_collect] or #[pipex_collect(retry = 2)] or #[pipex_collect(retry = 2, timeout = 500)]
#[proc_macro_attribute]
pub fn pipex_collect(args: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let attrs = parse_attr_args(args);
    
    let (retry_count, timeout_ms) = parse_common_attributes(&attrs);
    
    generate_function_with_config(input_fn, "collect", retry_count, timeout_ms)
}

/// #[pipex_fail_fast] - Mark function to use fail fast error strategy
/// Usage: #[pipex_fail_fast] or #[pipex_fail_fast(retry = 5)] or #[pipex_fail_fast(retry = 5, timeout = 2000)]
#[proc_macro_attribute]
pub fn pipex_fail_fast(args: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let attrs = parse_attr_args(args);
    
    let (retry_count, timeout_ms) = parse_common_attributes(&attrs);
    
    generate_function_with_config(input_fn, "fail_fast", retry_count, timeout_ms)
}

/// #[pipex_retry] - Generic retry wrapper that can be combined with other strategies
/// Usage: #[pipex_retry(retry = 3)] or #[pipex_retry(retry = 3, timeout = 1000)]
#[proc_macro_attribute]  
pub fn pipex_retry(args: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let attrs = parse_attr_args(args);
    
    let (retry_count, timeout_ms) = parse_common_attributes(&attrs);
    
    generate_function_with_config(input_fn, "retry", retry_count, timeout_ms)
}

// No tests in proc macro crate - they can't use the macros defined here
// Tests will be in the main pipex crate or integration tests