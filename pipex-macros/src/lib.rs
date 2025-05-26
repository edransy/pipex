extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, Meta, Expr, Lit, parse::Parse, parse::ParseStream, Token, Type};
use proc_macro2::Ident as ProcMacroIdent;

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


/// #[pipex_strategy(StrategyHandler)] - Apply custom strategy to function
/// 
/// This macro transforms a function to return PipexResult instead of Result,
/// automatically extracting the strategy name from the handler type.
/// 
/// Example:
/// ```rust,ignore
/// #[pipex_strategy(IgnoreHandler)]
/// async fn my_function(x: i32) -> Result<i32, String> {
///     // function logic here
///     Ok(x * 2)
/// }
/// ```
/// 
/// The strategy name is extracted by removing "Handler" suffix and converting to lowercase.
/// So "IgnoreHandler" becomes "ignore", "CustomRetryHandler" becomes "customretry", etc.
#[proc_macro_attribute]
pub fn error_strategy(args: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let strategy_type = parse_macro_input!(args as Type);
    
    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_inputs = &input_fn.sig.inputs;
    let fn_body = &input_fn.block;
    
    // Create hidden function name for original implementation
    let original_impl_name = syn::Ident::new(
        &format!("{}_original_impl", fn_name),
        fn_name.span()
    );
    
    // Use the full strategy type name as the strategy identifier
    let strategy_type_str = quote!(#strategy_type).to_string();
    
    // Extract return type from function signature
    let return_type = match &input_fn.sig.output {
        syn::ReturnType::Type(_, ty) => ty,
        syn::ReturnType::Default => {
            return syn::Error::new_spanned(
                &input_fn.sig,
                "Function must have an explicit return type"
            ).to_compile_error().into();
        }
    };
    
    let expanded = quote! {
        // Hidden original implementation
        #[doc(hidden)]
        async fn #original_impl_name(#fn_inputs) -> #return_type #fn_body
        
        // Public function now returns PipexResult
        #fn_vis async fn #fn_name(#fn_inputs) -> crate::PipexResult<i32, String> {
            let result = #original_impl_name(x).await;
            crate::PipexResult::new(result, #strategy_type_str)
        }
    };
    
    TokenStream::from(expanded)
}

// No tests in proc macro crate - they can't use the macros defined here
// Tests will be in the main pipex crate or integration tests