//! # Pipex Macros
//! 
//! Procedural macros for the pipex crate, providing error handling strategies
//! and pipeline decorators for async and sync functions.

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, ItemFn, Type, ReturnType, GenericArgument, PathArguments,
    parse::Parse, parse::ParseStream, Error, Result as SynResult
};

/// Parser for attribute arguments
struct AttributeArgs {
    strategy_type: Type,
}

impl Parse for AttributeArgs {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let strategy_type: Type = input.parse()?;
        Ok(AttributeArgs { strategy_type })
    }
}

/// Extract the inner types from Result<T, E>
fn extract_result_types(return_type: &Type) -> SynResult<(Type, Type)> {
    if let Type::Path(type_path) = return_type {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Result" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if args.args.len() == 2 {
                        if let (
                            GenericArgument::Type(ok_type),
                            GenericArgument::Type(err_type)
                        ) = (&args.args[0], &args.args[1]) {
                            return Ok((ok_type.clone(), err_type.clone()));
                        }
                    }
                }
            }
        }
    }
    
    Err(Error::new_spanned(
        return_type,
        "Expected function to return Result<T, E>"
    ))
}

/// The `error_strategy` attribute macro
/// 
/// This macro transforms a function that returns `Result<T, E>` into one that 
/// returns `PipexResult<T, E>`, allowing the pipex library to apply the specified
/// error handling strategy. Works with both sync and async functions.
/// 
/// # Arguments
/// 
/// * `strategy` - The error handling strategy type (e.g., `IgnoreHandler`, `CollectHandler`)
/// 
/// # Examples
/// 
/// ```rust,ignore
/// use pipex_macros::error_strategy;
/// 
/// // Async function
/// #[error_strategy(IgnoreHandler)]
/// async fn process_item_async(x: i32) -> Result<i32, String> {
///     if x % 2 == 0 {
///         Ok(x * 2)
///     } else {
///         Err("Odd number".to_string())
///     }
/// }
/// 
/// // Sync function  
/// #[error_strategy(CollectHandler)]
/// fn process_item_sync(x: i32) -> Result<i32, String> {
///     if x % 2 == 0 {
///         Ok(x * 2)
///     } else {
///         Err("Odd number".to_string())
///     }
/// }
/// ```
/// 
/// The generated function will automatically wrap the result in a `PipexResult`
/// with the specified strategy name, allowing the pipeline to handle errors
/// according to the strategy.
#[proc_macro_attribute]
pub fn error_strategy(args: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let args = parse_macro_input!(args as AttributeArgs);
    
    let strategy_type = args.strategy_type;
    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_inputs = &input_fn.sig.inputs;
    let fn_body = &input_fn.block;
    let fn_asyncness = &input_fn.sig.asyncness;
    let fn_generics = &input_fn.sig.generics;
    let where_clause = &input_fn.sig.generics.where_clause;
    
    // Extract return type and validate it's Result<T, E>
    let (ok_type, err_type) = match &input_fn.sig.output {
        ReturnType::Type(_, ty) => {
            match extract_result_types(ty) {
                Ok(types) => types,
                Err(e) => return e.to_compile_error().into(),
            }
        }
        ReturnType::Default => {
            return Error::new_spanned(
                &input_fn.sig,
                "Function must return Result<T, E>"
            ).to_compile_error().into();
        }
    };
    
    // Create hidden function name for original implementation
    let original_impl_name = syn::Ident::new(
        &format!("{}_original_impl", fn_name),
        fn_name.span()
    );
    
    // Use the strategy type name as the strategy identifier
    let strategy_name = quote!(#strategy_type).to_string();
    
    // Extract parameter names for the function call
    let param_names: Vec<_> = input_fn.sig.inputs.iter().filter_map(|arg| {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                Some(&pat_ident.ident)
            } else {
                None
            }
        } else {
            None
        }
    }).collect();
    
    // Generate different code based on whether function is async or sync
    let function_call = if fn_asyncness.is_some() {
        // Async function - use .await
        quote! { #original_impl_name(#(#param_names),*).await }
    } else {
        // Sync function - no .await
        quote! { #original_impl_name(#(#param_names),*) }
    };
    
    let expanded = quote! {
        #[doc(hidden)]
        #fn_asyncness fn #original_impl_name #fn_generics (#fn_inputs) -> Result<#ok_type, #err_type> #where_clause
        #fn_body
        
        #fn_vis #fn_asyncness fn #fn_name #fn_generics (#fn_inputs) -> crate::PipexResult<#ok_type, #err_type> #where_clause {
            let result = #function_call;
            crate::PipexResult::new(result, #strategy_name)
        }
    };
    
    TokenStream::from(expanded)
}

// No tests in proc macro crate - they can't use the macros defined here
// Tests will be in the main pipex crate or integration tests