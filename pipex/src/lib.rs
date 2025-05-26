//! # Pipex
//! 
//! A powerful functional pipeline macro for Rust that combines synchronous, asynchronous, 
//! parallel, and streaming operations with extensible error handling via proc macros.

// Re-export the proc macros with clear, non-conflicting names
pub use pipex_macros::{
    pipex_ignore as pipex_ignore, 
    pipex_collect as pipex_collect, 
    pipex_fail_fast as pipex_fail_fast, 
    pipex_retry as pipex_retry
};

// Re-export dependencies
pub use futures;
pub use rayon; 
pub use tokio;
pub use async_trait::async_trait;

/// Trait for custom error handling strategies
pub trait ErrorHandler<T, E> {
    type Output;
    fn handle_results(results: Vec<Result<T, E>>) -> Self::Output;
}

/// Ignore errors strategy
pub struct IgnoreHandler;
impl<T, E> ErrorHandler<T, E> for IgnoreHandler {
    type Output = Vec<T>;
    fn handle_results(results: Vec<Result<T, E>>) -> Vec<T> {
        results.into_iter().filter_map(Result::ok).collect()
    }
}

/// Collect all strategy  
pub struct CollectHandler;
impl<T, E> ErrorHandler<T, E> for CollectHandler {
    type Output = Vec<Result<T, E>>;
    fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
        results
    }
}

/// Fail fast strategy
pub struct FailFastHandler;
impl<T, E> ErrorHandler<T, E> for FailFastHandler {
    type Output = Result<Vec<T>, E>;
    fn handle_results(results: Vec<Result<T, E>>) -> Result<Vec<T>, E> {
        let mut successes = Vec::new();
        for result in results {
            match result {
                Ok(value) => successes.push(value),
                Err(e) => return Err(e),
            }
        }
        Ok(successes)
    }
}

/// Main pipeline macro
#[macro_export]
macro_rules! pipex {
    // Entry point
    ($input:expr $(=> $($rest:tt)+)?) => {{
        pipex!(@process $input $(=> $($rest)+)?)
    }};

    // SYNC step
    (@process $input:expr => |$var:ident| $body:expr $(=> $($rest:tt)+)?) => {{
        let iter_result = $input.into_iter().map(|$var| $body);
        pipex!(@process iter_result $(=> $($rest)+)?)
    }};

    // ASYNC step  
    (@process $input:expr => async |$var:ident| $body:block $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            async {
                $crate::futures::future::join_all(input.into_iter().map(|$var| async move $body)).await
            }.await
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // ASYNC with error handling - simplified version that works with default behavior
    (@process $input:expr => async? |$var:ident| $fn_name:ident($($args:expr),*) $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            async {
                // Default fail-fast behavior for now
                let mut results = Vec::new();
                for item in input {
                    let $var = item;
                    match $fn_name($($args),*).await {
                        Ok(value) => results.push(value),
                        Err(e) => return Err(e),
                    }
                }
                Ok(results)
            }.await
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // ASYNC with error handling - block syntax (for inline async blocks)
    (@process $input:expr => async? |$var:ident| $body:block $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            async {
                // Default fail-fast behavior
                let mut results = Vec::new();
                for item in input {
                    let $var = item;
                    match async move $body.await {
                        Ok(value) => results.push(value),
                        Err(e) => return Err(e),
                    }
                }
                Ok(results)
            }.await
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // Terminal case
    (@process $input:expr) => {{
        pipex!(@ensure_vec $input)
    }};

    // Helper to ensure we have a Vec when needed
    (@ensure_vec $input:expr) => {{
        $input.into_iter().collect::<Vec<_>>()
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test basic functionality without proc macros first
    async fn simple_double(x: i32) -> Result<i32, String> {
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
        );
        
        // This should work since we skip 3
        assert_eq!(result, vec![Ok(2), Ok(4), Ok(8), Ok(10)]);
    }

    #[tokio::test]
    async fn test_sync_pipeline() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => |x| x * 2
            => |x| x + 1
        );
        
        assert_eq!(result, vec![3, 5, 7, 9, 11]);
    }

    // #[tokio::test]
    // async fn test_async_with_error_handling_success() {
    //     let result = pipex!(
    //         vec![1, 2, 4, 5]  // Skip 3 to avoid errors
    //         => async? |x| { simple_double(x).await }
    //     );
        
    //     // Should succeed with fail-fast default behavior
    //     assert_eq!(result, Ok(vec![2, 4, 8, 10]));
    // }

    // #[tokio::test]
    // async fn test_async_with_error_handling_failure() {
    //     let result = pipex!(
    //         vec![1, 2, 3, 4, 5]  // Include 3 which will fail
    //         => async? |x| { simple_double(x).await }
    //     );
        
    //     // Should fail fast on 3
    //     assert!(result.is_err());
    // }

    // Functions with proc macro attributes - use explicit names to avoid conflicts
    #[pipex_collect]
    async fn process_with_collect(x: i32) -> Result<i32, String> {
        eprintln!("process_with_ignore_retry called with x = {}", x);
        if x == 3 {
            eprintln!("Failing on x = 3");
            Err("failed on 3".to_string())
        } else {
            let result = x * 2;
            eprintln!("Success for x = {}, result = {}", x, result);
            Ok(result)
        }
    }

    #[pipex_ignore]
    async fn process_with_ignore(x: i32) -> Result<i32, String> {
        if x == 3 {
            Err("failed on 3".to_string())
        } else {
            Ok(x * 2)
        }
    }

    #[pipex_fail_fast(retry = 5)]
    async fn process_with_fail_fast_retry(x: i32) -> Result<i32, String> {
        if x == 999 {
            Err("should not happen".to_string())
        } else {
            Ok(x * 3)
        }
    }

    #[tokio::test]
    async fn test_proc_macro_functions_work() {
        // Test that the proc macro functions compile and work normally
        let result = process_with_collect(5).await;
        assert_eq!(result, Ok(10));

        let result = process_with_fail_fast_retry(2).await;
        assert_eq!(result, Ok(6));
    }

    #[tokio::test]
    async fn test_basic_async_error_function_call_with_collect() {
        eprintln!("\n=== Starting test_basic_async_error_function_call ===");
        
        // First try without the error-causing value
        let input_success = vec![1, 2, 4, 5];
        eprintln!("\nTesting with success case: {:?}", input_success);
        let result_success = pipex!(
            input_success
            => async |x| {
                eprintln!("Processing value: {}", x);
                let res = process_with_collect(x).await;
                eprintln!("Result for {}: {:?}", x, res);
                res
            }
        );
        eprintln!("Success case result: {:?}", result_success);

        // Then try with the error-causing value
        let input_with_error = vec![1, 2, 3, 4, 5];
        eprintln!("\nTesting with error case: {:?}", input_with_error);
        let result_with_error = pipex!(
            input_with_error
            => async |x| { process_with_collect(x).await }
        );
        eprintln!("Error case result: {:?}", result_with_error);
        
        // Since we're using collect strategy, we should see all results including errors
        assert!(result_with_error.iter().any(|r| r.is_err()));
    }


    #[tokio::test]
    async fn test_basic_async_error_function_call_with_ignore() {
        eprintln!("\n=== Starting test_basic_async_error_function_call ===");
        
        // First try without the error-causing value
        let input_success = vec![1, 2, 4, 5];
        eprintln!("\nTesting with success case: {:?}", input_success);
        let result_success = pipex!(
            input_success
            => async |x| {
                eprintln!("Processing value: {}", x);
                let res = process_with_ignore(x).await;
                eprintln!("Result for {}: {:?}", x, res);
                res
            }
        );
        eprintln!("Success case result: {:?}", result_success);

        // Then try with the error-causing value
        let input_with_error = vec![1, 2, 3, 4, 5];
        eprintln!("\nTesting with error case: {:?}", input_with_error);
        let result_with_error = pipex!(
            input_with_error
            => async |x| { process_with_ignore(x).await }
        );
        eprintln!("Error case result: {:?}", result_with_error);
        
        // Since we're using collect strategy, we should see all results including errors
        assert!(result_with_error.iter().any(|r| r.is_err()));
    }

    // #[tokio::test]
    // async fn test_basic_async_error_function_call_with_failure() {
    //     // Test that default fail-fast behavior works
    //     let result = pipex!(
    //         vec![1, 2, 3, 4, 5]  // Include 3 which will fail
    //         => async? |x| process_with_ignore_retry(x)
    //     );
        
    //     assert!(result.is_err());
    // }

    // #[tokio::test]
    // async fn test_chaining_after_async_error_handling() {
    //     // Test that we can chain operations after async?
    //     let result = pipex!(
    //         vec![1, 2, 4, 5]  // Skip 3 to avoid errors
    //         => async? |x| process_with_ignore_retry(x)
    //         => |x| match x {
    //             Ok(values) => values.into_iter().map(|v| v + 1).collect(),
    //             Err(_) => vec![]
    //         }
    //     );

    //     assert_eq!(result, vec![3, 5, 9, 11]);
    // }
}