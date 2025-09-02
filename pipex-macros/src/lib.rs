//! # Pipex Macros
//! 
//! Procedural macros for the pipex crate, providing error handling strategies
//! and pipeline decorators for async and sync functions.

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, ItemFn, Type, ReturnType, GenericArgument, PathArguments,
    parse::Parse, parse::ParseStream, Error, Result as SynResult,
    visit_mut::{self, VisitMut}, Expr, Ident, Lit,
    spanned::Spanned,
};

/// Converts a string from snake_case to PascalCase.
fn to_pascal_case(s: &str) -> String {
    let mut pascal = String::new();
    let mut capitalize = true;
    for c in s.chars() {
        if c == '_' {
            capitalize = true;
        } else if capitalize {
            pascal.push(c.to_ascii_uppercase());
            capitalize = false;
        } else {
            pascal.push(c);
        }
    }
    pascal
}

/// A visitor that recursively checks a function for impurity and injects purity checks for function calls.
struct PurityCheckVisitor {
    errors: Vec<Error>,
}

impl VisitMut for PurityCheckVisitor {
    fn visit_expr_mut(&mut self, i: &mut Expr) {
        match i {
            Expr::Unsafe(e) => {
                self.errors.push(Error::new(
                    e.span(),
                    "impure `unsafe` block found in function marked as `pure`",
                ));
            }
            Expr::Macro(e) => {
                if e.mac.path.is_ident("asm") {
                    self.errors.push(Error::new(
                        e.span(),
                        "impure inline assembly found in function marked as `pure`",
                    ));
                }
            }
            Expr::MethodCall(e) => {
                self.errors.push(Error::new(
                    e.span(), "method calls are not supported in pure functions"
                ));
            }
            Expr::Call(call_expr) => {
                if let Expr::Path(expr_path) = &*call_expr.func {
                    let path = &expr_path.path;
                    if let Some(segment) = path.segments.last() {
                        if segment.ident == "Ok" || segment.ident == "Err" {
                            visit_mut::visit_expr_call_mut(self, call_expr);
                            return;
                        }
                    }

                    let mut zst_path = path.clone();
                    if let Some(last_segment) = zst_path.segments.last_mut() {
                        let ident_str = last_segment.ident.to_string();
                        let pascal_case_ident = to_pascal_case(&ident_str);
                        last_segment.ident = Ident::new(&pascal_case_ident, last_segment.ident.span());

                        // Create a simple compile-time purity check
                        let _fn_name = path.segments.last().unwrap().ident.to_string();
                        
                        // Manually recurse before we replace the node
                        visit_mut::visit_expr_call_mut(self, call_expr);
                        
                        let new_node = syn::parse_quote!({
                            {
                                // Compile-time purity check - generates a clear error if function isn't pure
                                let _ = || {
                                    fn _assert_pure_function<T: crate::traits::IsPure>(_: T) {}
                                    _assert_pure_function(#zst_path);
                                };
                                #call_expr
                            }
                        });
                        *i = new_node;
                        return; // Return to avoid visiting the new node and causing infinite recursion
                    }
                } else {
                    self.errors.push(Error::new_spanned(&call_expr.func, "closures and other complex function call expressions are not supported in pure functions"));
                }
            }
            _ => {}
        }
        
        // Default recursion for all other expression types.
        visit_mut::visit_expr_mut(self, i);
    }
}

/// The `pure` attribute macro.
///
/// This macro checks if a function is "pure". A function is pure if:
/// 1. It contains no `unsafe` blocks or inline assembly (`asm!`).
/// 2. It does not call any methods.
/// 3. It only calls other functions that are themselves marked `#[pure]`.
#[proc_macro_attribute]
pub fn pure(_args: TokenStream, item: TokenStream) -> TokenStream {
    let mut input_fn = parse_macro_input!(item as ItemFn);

    let mut visitor = PurityCheckVisitor { errors: vec![] };

    // Clone the function body's box, and visit the block inside.
    let mut new_body_box = input_fn.block.clone();
    visitor.visit_block_mut(&mut new_body_box);

    if !visitor.errors.is_empty() {
        let combined_errors = visitor.errors.into_iter().reduce(|mut a, b| {
            a.combine(b);
            a
        });
        if let Some(errors) = combined_errors {
            return errors.to_compile_error().into();
        }
    }

    // Replace the old body with the new one containing the checks.
    input_fn.block = new_body_box;

    // Generate the ZST and IsPure impl.
    let fn_name_str = input_fn.sig.ident.to_string();
    let zst_name = Ident::new(&to_pascal_case(&fn_name_str), input_fn.sig.ident.span());

    let expanded = quote! {
        #input_fn

        #[doc(hidden)]
        struct #zst_name;
        #[doc(hidden)]
        impl crate::traits::IsPure for #zst_name {}
    };

    TokenStream::from(expanded)
}

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

/// Memoization configuration for the `#[memoized]` attribute
struct MemoizedArgs {
    capacity: Option<usize>,
}

impl Parse for MemoizedArgs {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let mut capacity = None;
        
        // Parse optional arguments like: capacity = 1000
        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(syn::Ident) {
                let ident: Ident = input.parse()?;
                if ident == "capacity" {
                    input.parse::<syn::Token![=]>()?;
                    let lit: Lit = input.parse()?;
                    if let Lit::Int(lit_int) = lit {
                        capacity = Some(lit_int.base10_parse()?);
                    } else {
                        return Err(Error::new_spanned(lit, "capacity must be an integer"));
                    }
                } else {
                    return Err(Error::new_spanned(ident, "unknown attribute argument"));
                }
                
                // Handle optional comma
                if input.peek(syn::Token![,]) {
                    input.parse::<syn::Token![,]>()?;
                }
            } else {
                return Err(lookahead.error());
            }
        }
        
        Ok(MemoizedArgs { capacity })
    }
}

impl Default for MemoizedArgs {
    fn default() -> Self {
        Self { capacity: Some(1000) } // Default capacity
    }
}

/// The `memoized` attribute macro for automatic function memoization.
///
/// This macro provides automatic memoization for functions, caching results based on input parameters.
/// It's designed to work perfectly with `#[pure]` functions since pure functions are safe to memoize.
///
/// # Features
/// - Thread-safe caching using DashMap
/// - Configurable cache capacity
/// - Automatic cache key generation from function parameters
/// - Zero-cost abstraction when memoization feature is disabled
///
/// # Arguments
/// - `capacity` (optional): Maximum number of entries to cache (default: 1000)
///
/// # Requirements
/// - Function parameters must implement `Clone + std::hash::Hash + Eq`
/// - Return type must implement `Clone`
/// - Requires the "memoization" feature to be enabled
///
/// # Examples
///
/// ```rust,ignore
/// use pipex_macros::{pure, memoized};
///
/// #[pure]
/// #[memoized]
/// fn fibonacci(n: u64) -> u64 {
///     if n <= 1 { n } else { fibonacci(n-1) + fibonacci(n-2) }
/// }
///
/// #[pure]
/// #[memoized(capacity = 500)]
/// fn expensive_calculation(x: i32, y: i32) -> i32 {
///     // Some expensive computation
///     x * y + (x ^ y)
/// }
/// ```
#[proc_macro_attribute]
pub fn memoized(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = if args.is_empty() {
        MemoizedArgs::default()
    } else {
        parse_macro_input!(args as MemoizedArgs)
    };
    
    let input_fn = parse_macro_input!(item as ItemFn);
    
    // Extract function information
    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_inputs = &input_fn.sig.inputs;
    let fn_output = &input_fn.sig.output;
    let fn_generics = &input_fn.sig.generics;
    let where_clause = &input_fn.sig.generics.where_clause;
    let fn_asyncness = &input_fn.sig.asyncness;
    
    // Generate cache name
    let cache_name = Ident::new(&format!("{}_CACHE", fn_name.to_string().to_uppercase()), fn_name.span());
    
    // Extract parameter names and types for key generation
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
    
    // Generate original function name
    let original_fn_name = Ident::new(&format!("{}_original", fn_name), fn_name.span());
    
    // Determine cache capacity
    let capacity = args.capacity.unwrap_or(1000);
    
    // Extract return type for cache value
    let return_type = match &input_fn.sig.output {
        ReturnType::Default => quote! { () },
        ReturnType::Type(_, ty) => quote! { #ty },
    };
    
    // Generate cache key type - tuple of all parameter types
    let key_type = if param_names.is_empty() {
        quote! { () }
    } else {
        let param_types: Vec<_> = input_fn.sig.inputs.iter().filter_map(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                Some(&pat_type.ty)
            } else {
                None
            }
        }).collect();
        
        if param_types.len() == 1 {
            quote! { #(#param_types)* }
        } else {
            quote! { (#(#param_types),*) }
        }
    };
    
    // Generate cache key creation
    let key_creation = if param_names.is_empty() {
        quote! { () }
    } else if param_names.len() == 1 {
        let param = &param_names[0];
        quote! { #param.clone() }
    } else {
        quote! { (#(#param_names.clone()),*) }
    };
    
    // Generate function call
    let fn_call = if fn_asyncness.is_some() {
        quote! { #original_fn_name(#(#param_names),*).await }
    } else {
        quote! { #original_fn_name(#(#param_names),*) }
    };
    
    let fn_body = &input_fn.block;
    
    let expanded = quote! {
        // Original function implementation
        #fn_asyncness fn #original_fn_name #fn_generics (#fn_inputs) #fn_output #where_clause
        #fn_body
        
        // Memoized wrapper function
        #fn_vis #fn_asyncness fn #fn_name #fn_generics (#fn_inputs) #fn_output #where_clause {
            #[cfg(feature = "memoization")]
            {
                use std::sync::Arc;
                
                // Thread-safe cache using DashMap
                static #cache_name: crate::once_cell::sync::Lazy<crate::dashmap::DashMap<#key_type, #return_type>> = crate::once_cell::sync::Lazy::new(|| {
                    crate::dashmap::DashMap::with_capacity(#capacity)
                });
                
                let cache = &#cache_name;
                
                let key = #key_creation;
                
                // Check cache first
                if let Some(cached_result) = cache.get(&key) {
                    return cached_result.clone();
                }
                
                // Compute result and cache it
                let result = #fn_call;
                
                // Only cache if we haven't exceeded capacity
                if cache.len() < #capacity {
                    cache.insert(key, result.clone());
                }
                
                result
            }
            
            #[cfg(not(feature = "memoization"))]
            {
                // When memoization is disabled, just call the original function
                #fn_call
            }
        }
    };
    
    TokenStream::from(expanded)
}



// No tests in proc macro crate - they can't use the macros defined here
// Tests will be in the main pipex crate or integration tests