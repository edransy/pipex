//! A pipeline processing library for Rust
//! 
//! This crate provides a powerful macro-based system for creating data processing
//! pipelines with various error handling strategies.

#![deny(missing_docs)]
#![warn(clippy::all)]

// Core modules
mod result;
mod traits;
mod handlers;
mod macros;

// Re-export public API
pub use result::PipexResult;
pub use traits::{PipelineResultHandler, ExtractSuccessful, IntoResult, CreateError};
pub use handlers::{
    ErrorHandler, IgnoreHandler, CollectHandler, FailFastHandler, LogAndIgnoreHandler
};

// Re-export the proc macros
pub use pipex_macros::error_strategy;

// Conditional re-exports based on features
#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub use futures;

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub use tokio;

#[cfg(feature = "parallel")]
#[cfg_attr(docsrs, doc(cfg(feature = "parallel")))]
pub use rayon;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::any::{Any, TypeId};

// Registry for strategy functions
static STRATEGY_REGISTRY: OnceLock<Mutex<HashMap<String, Box<dyn Any + Send + Sync>>>> = OnceLock::new();

/// Register a custom strategy handler for specific types
pub fn register_strategy<T, E>(
    name: &str,
    handler: fn(Vec<Result<T, E>>) -> Vec<Result<T, E>>
) where
    T: 'static,
    E: std::fmt::Debug + 'static,
{
    let registry = STRATEGY_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));
    let mut registry = registry.lock().unwrap();
    
    let type_id = (TypeId::of::<T>(), TypeId::of::<E>());
    let key = format!("{}_{:?}", name, type_id);
    
    registry.insert(key, Box::new(handler));
}

/// Try to call a registered strategy (fix ownership issue by borrowing)
fn try_registered_strategy<T, E>(strategy_name: &str, results: &Vec<Result<T, E>>) -> Option<Vec<Result<T, E>>>
where
    T: 'static + Clone,
    E: std::fmt::Debug + 'static + Clone,
{
    let registry = STRATEGY_REGISTRY.get()?;
    let registry = registry.lock().unwrap();
    
    let type_id = (TypeId::of::<T>(), TypeId::of::<E>());
    let key = format!("{}_{:?}", strategy_name, type_id);
    
    if let Some(handler_any) = registry.get(&key) {
        if let Some(handler) = handler_any.downcast_ref::<fn(Vec<Result<T, E>>) -> Vec<Result<T, E>>>() {
            return Some(handler(results.clone()));
        }
    }
    
    None
}

/// Apply strategy - checks registry first, then built-ins
pub fn apply_strategy<T, E>(strategy_name: &str, results: Vec<Result<T, E>>) -> Vec<Result<T, E>>
where
    T: 'static + Clone,
    E: std::fmt::Debug + 'static + Clone,
{
    // Try registered strategies first (pass by reference to avoid move)
    if let Some(result) = try_registered_strategy(strategy_name, &results) {
        return result;
    }
    
    // Fall back to built-ins (now we can still use results since we didn't move it)
    match strategy_name {
        "IgnoreHandler" => IgnoreHandler::handle_results(results),
        "CollectHandler" => CollectHandler::handle_results(results),
        "FailFastHandler" => FailFastHandler::handle_results(results),
        "LogAndIgnoreHandler" => LogAndIgnoreHandler::handle_results(results),
        _ => {
            eprintln!("Warning: Unknown strategy '{}'. Use register_strategy() to register custom handlers.", strategy_name);
            results
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub struct FirstErrorHandler;

    impl<T, E> ErrorHandler<T, E> for FirstErrorHandler {
        fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
            results.into_iter()
                .find(|r| r.is_err())
                .map_or(Vec::new(), |e| vec![e])
        }
    }

    // Register the handler for our test types
    use std::sync::Once;
    static INIT: Once = Once::new();
    
    fn setup() {
        INIT.call_once(|| {
            register_strategies!( FirstErrorHandler for <i32, String> );
        });
    }

    pub fn apply_strategy<T, E>(strategy_name: &str, results: Vec<Result<T, E>>) -> Vec<Result<T, E>>
    where
        T: 'static + Clone,
        E: std::fmt::Debug + 'static + Clone,
    {
        setup(); // Ensure registration
        crate::apply_strategy(strategy_name, results)
    }

    // Basic test functions
    async fn simple_double(x: i32) -> Result<i32, String> {
        if x == 3 {
            Err("failed on 3".to_string())
        } else {
            Ok(x * 2)
        }
    }
    
    // Test functions with strategy decorators
    #[error_strategy(CollectHandler)]
    async fn process_and_collect(x: i32) -> Result<i32, String> {
        if x == 3 {
            Err("failed on 3".to_string())
        } else {
            Ok(x * 2)
        }
    }
    
    #[error_strategy(IgnoreHandler)]
    async fn process_and_ignore(x: i32) -> Result<i32, String> {
        if x == 3 {
            Err("failed on 3".to_string())
        } else {
            Ok(x * 2)
        }
    }

    #[error_strategy(FailFastHandler)]
    async fn process_with_failfast(x: i32) -> Result<i32, String> {
        if x == 3 {
            Err("failed on 3".to_string())
        } else {
            Ok(x * 2)
        }
    }

    #[error_strategy(LogAndIgnoreHandler)]
    async fn process_with_log_and_ignore(x: i32) -> Result<i32, String> {
        if x == 3 {
            Err("failed on 3".to_string())
        } else {
            Ok(x * 2)
        }
    }

    #[error_strategy(FirstErrorHandler)]
    async fn process_with_first_error(x: i32) -> Result<i32, String> {
        if x == 3 {
            Err("failed on 3".to_string())
        } else {
            Ok(x * 2)
        }
    }

    #[tokio::test]
    async fn test_basic_async_pipeline() {
        let result = pipex!(
            vec![1, 2, 4, 5]
            => async |x| { simple_double(x).await }
            => |x| Ok::<i32, String>(x + 1)
        );
        
        assert_eq!(result.len(), 4);
        assert!(result.iter().all(|r| r.is_ok()));
        let values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(values, vec![3, 5, 9, 11]);
    }

    #[tokio::test]
    async fn test_sync_pipeline() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => |x| Ok::<i32, String>(x * 2)
            => |x| Ok::<i32, String>(x + 1)
        );
        
        assert_eq!(result.len(), 5);
        assert!(result.iter().all(|r| r.is_ok()));
        let values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(values, vec![3, 5, 7, 9, 11]);
    }

    #[tokio::test]
    async fn test_strategy_collect() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_and_collect(x).await }
        );
        
        // Should collect all results including errors
        assert!(result.iter().any(|r| r.is_err()));
        assert_eq!(result.len(), 5);
        eprintln!("result: {:?}", result);
    }

    #[tokio::test]
    async fn test_strategy_ignore() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_and_ignore(x).await }
        );
        
        // Should ignore errors, only return successes
        assert_eq!(result.len(), 4); // 3 is filtered out
        assert!(result.iter().all(|r| r.is_ok()));
        eprintln!("result: {:?}", result);
    }

    #[tokio::test]
    async fn test_strategy_log_and_ignore() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_with_log_and_ignore(x).await }
        );
        
        // Should log errors and ignore them, only return successes
        assert_eq!(result.len(), 4); // 3 is filtered out but logged
        assert!(result.iter().all(|r| r.is_ok()));
        
        let values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(values, vec![2, 4, 8, 10]); // 1*2, 2*2, 4*2, 5*2
    }

    #[tokio::test]
    async fn test_strategy_failfast() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5, 3]
            => async |x| { process_with_failfast(x).await }
        );
        
        // Should fail fast - return only errors
        assert!(result.iter().all(|r| r.is_err()));
        assert_eq!(result.len(), 2); // Two instances of 3
        eprintln!("result: {:?}", result);
    }

    #[tokio::test]
    async fn test_custom_first_error_handler() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_with_first_error(x).await }
        );
        
        // Should return only the first error
        assert_eq!(result.len(), 1);
        assert!(result[0].is_err());
        eprintln!("FirstErrorHandler result: {:?}", result);
    }

    #[cfg(feature = "parallel")]
    #[tokio::test]
    async fn test_parallel_pipeline() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => ||| |x| Ok::<i32, String>(x * 2)
        );
        
        assert_eq!(result.len(), 5);
        assert!(result.iter().all(|r| r.is_ok()));

        eprintln!("result: {:?}", result);
        // let mut values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        // values.sort(); // Parallel processing might change order
        // assert_eq!(values, vec![3, 5, 7, 9, 11]);
    }

    // Test sync function with strategy decorator
    #[error_strategy(IgnoreHandler)]
    fn sync_process_and_ignore(x: i32) -> Result<i32, String> {
        if x == 3 { Err("failed on 3".to_string()) }
        else { Ok(x * 2) }
    }

    #[test]
    fn test_sync_step_with_error_strategy() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => |x| sync_process_and_ignore(x)
            => |x| Ok::<i32, String>(x + 1)
        );
        
        // Should ignore errors due to IgnoreHandler strategy, only return successes
        assert_eq!(result.len(), 4); // 3 is filtered out by IgnoreHandler
        assert!(result.iter().all(|r| r.is_ok()));

        eprintln!("result: {:?}", result);
        
        let values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        let expected_values = vec![3, 5, 9, 11];
        let mut actual_values = values;
        actual_values.sort(); // Ensure order for comparison
        assert_eq!(actual_values, expected_values); 
    }

    #[tokio::test]
    async fn test_mixed_sync_and_async_pipeline() {
        // Using an existing async function with its own error strategy
        // process_and_collect is #[error_strategy(CollectHandler)]
        // It returns Err for input 3, Ok(x*2) otherwise.

        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => |x| sync_process_and_ignore(x) // IgnoreHandler drops item 3
            => |x| Ok::<i32, String>(x - 1)
            => async |x| { process_and_collect(x).await } // CollectHandler collect error item 3
        );

        assert_eq!(result.len(), 4, "Expected 4 results after IgnoreHandler dropped one item.");
        eprintln!("Mixed sync-async-sync pipeline (Ignore then Collect) result: {:?}", result);

        let expected_values = vec![Ok(2), Err("failed on 3".to_string()), Ok(14), Ok(18)]; // (2*2-1), (4*2-1), (8*2-1), (10*2-1)
        assert_eq!(result, expected_values, "Final values do not match expected values.");
    }
}
