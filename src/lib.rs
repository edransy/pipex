//! # Pipex - Functional Pipeline Macro for Rust
//! 
//! Pipex is a powerful functional pipeline macro for Rust that combines synchronous, 
//! asynchronous, parallel, and streaming operations with extensible error handling.
//!
//! ## Features
//!
//! - **🔄 Sync Operations**: Chain regular synchronous transformations
//! - **⚡ Async Operations**: Handle asynchronous work with automatic await
//! - **🚀 Parallel Processing**: Leverage multiple CPU cores with Rayon
//! - **🛡️ Error Handling**: Extensible error handling strategies via proc macros
//! - **🔀 Mixed Workloads**: Seamlessly combine different operation types
//!
//! ## Quick Start
//!
//! ```rust
//! use pipex::pipex;
//!
//! let result = pipex!(
//!     vec![1, 2, 3, 4, 5]
//!     => |x| x * 2
//!     => |x| x + 1
//! );
//! // result contains: [Ok(3), Ok(5), Ok(7), Ok(9), Ok(11)]
//! ```
//!
//! ## Async Example
//!
//! ```rust,no_run
//! use pipex::pipex;
//!
//! async fn double_async(x: i32) -> Result<i32, String> {
//!     tokio::time::sleep(std::time::Duration::from_millis(1)).await;
//!     Ok(x * 2)
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let result = pipex!(
//!         vec![1, 2, 3, 4, 5]
//!         => async |x| { double_async(x).await }
//!         => |x| x + 1
//!     );
//!     // Process numbers asynchronously
//!     println!("Result: {:?}", result);
//! }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
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

// Generate the apply_strategy function with all built-in handlers and our custom one
#[allow(missing_docs)]
pub struct FirstErrorHandler;

impl<T, E> ErrorHandler<T, E> for FirstErrorHandler {
    fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
        // Find the first error and return it, or return empty vec if no errors
        results.into_iter()
            .find(|r| r.is_err())
            .map_or(Vec::new(), |e| vec![e])
    }
}

// Register all handlers including our new custom one
apply_strategy!(
    IgnoreHandler, 
    CollectHandler, 
    FailFastHandler, 
    LogAndIgnoreHandler,
    FirstErrorHandler
);


#[cfg(test)]
mod tests {
    use super::*;

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

    #[tokio::test]
    async fn test_basic_async_pipeline() {
        let result = pipex!(
            vec![1, 2, 4, 5]
            => async |x| { simple_double(x).await }
            => |x| x + 1
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
            => |x| x * 2
            => |x| x + 1
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

    #[cfg(feature = "parallel")]
    #[tokio::test]
    async fn test_parallel_pipeline() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => ||| |x| x * x  // Parallel squaring
            => |x| x + 1      // Sync add
        );
        
        assert_eq!(result.len(), 5);
        assert!(result.iter().all(|r| r.is_ok()));
        let values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        let expected = vec![2, 5, 10, 17, 26];
        assert_eq!(values, expected);
    }

    #[cfg(feature = "parallel")]
    #[tokio::test]
    async fn test_mixed_sync_async_parallel_pipeline() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_with_log_and_ignore(x).await } // Async with LogAndIgnoreHandler
            => ||| |x| x + 10                                     // Parallel: add 10
            => |x| x - 1                                          // Sync: subtract 1
        );
        
        let successful_count = result.iter().filter(|r| r.is_ok()).count();
        assert_eq!(successful_count, 4);
        
        let error_count = result.iter().filter(|r| r.is_err()).count();
        assert_eq!(error_count, 0);
    }

    #[test]
    fn test_first_error_handler() {
        let results = vec![
            Ok(1), 
            Ok(2), 
            Err("first error"), 
            Ok(4), 
            Err("second error")
        ];
        
        let handled = apply_strategy("FirstErrorHandler", results);
        assert_eq!(handled.len(), 1);
        assert!(handled[0].is_err());
        assert_eq!(*handled[0].as_ref().unwrap_err(), "first error");
    }
}
