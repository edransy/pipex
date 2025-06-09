//! A pipeline processing library for Rust
//! 
//! This crate provides a powerful macro-based system for creating data processing
//! pipelines with various error handling strategies.
#![no_std]
#![deny(missing_docs)]
#![warn(clippy::all)]

// Core modules
mod result;
pub mod traits;

// Re-export public API
pub use result::PipexResult;
pub use traits::{PipelineOp, ErrorHandler, IntoPipelineItem};


// Re-export the proc macros
pub use pipex_macros::error_strategy;


/// Ignore errors strategy - replaces errors with default values
pub struct IgnoreHandler;

/// Collect errors strategy - keeps all results including errors
pub struct CollectHandler;

/// FailFast handler - marks all as errors if any error is found
pub struct FailFastHandler;

impl<T, E, const N: usize> ErrorHandler<T, E, N> for IgnoreHandler {
    fn handle_results(mut results: [Result<T, E>; N]) -> [Result<T, E>; N] {
        for result in results.iter_mut() {
            if result.is_err() {
                // Safety: This is safe because we're only replacing error variants with a default value
                *result = Ok(unsafe { core::mem::zeroed() });
            }
        }
        results
    }
}

impl<T, E, const N: usize> ErrorHandler<T, E, N> for CollectHandler {
    fn handle_results(results: [Result<T, E>; N]) -> [Result<T, E>; N] {
        results
    }
}

impl<T, E, const N: usize> ErrorHandler<T, E, N> for FailFastHandler 
where
    E: Copy
{
    fn handle_results(mut results: [Result<T, E>; N]) -> [Result<T, E>; N] {
        // First find if there's an error and get its value
        let first_err = match results.iter().find_map(|r| r.as_ref().err()) {
            Some(err) => *err,
            None => return results,
        };

        // Then propagate the error to all results
        for result in results.iter_mut() {
            *result = Err(first_err);
        }
        
        results
    }
}

/// Apply an error handling strategy to results
pub fn apply_strategy<T, E, const N: usize>(
    strategy_name: &str,
    results: [Result<T, E>; N]
) -> [Result<T, E>; N]
where
    E: Copy,
{
    match strategy_name {
        "IgnoreHandler" => IgnoreHandler::handle_results(results),
        "CollectHandler" => CollectHandler::handle_results(results),
        "FailFastHandler" => FailFastHandler::handle_results(results),
        _ => results,
    }
}

/// Macro to apply an error handling strategy to a function
#[macro_export]
macro_rules! with_strategy {
    ($strategy:ident, $func:expr) => {{
        let result = $func;
        $crate::apply_strategy(stringify!($strategy), result)
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! pipex {
    // Entry point
    ($input:expr $(=> $($rest:tt)+)?) => {{
        let input_array = $input;
        let initial_results = core::array::from_fn(|i| Ok(input_array[i]));
        pipex!(@process initial_results $(=> $($rest)+)?)
    }};

    // SYNC step
    (@process $input:expr => |$var:ident| $body:expr $(=> $($rest:tt)+)?) => {{
        let mut sync_results = $input;
        let len = sync_results.len();
        
        for i in 0..len {
            sync_results[i] = match sync_results[i] {
                Ok($var) => ($body).into_pipeline_item(),
                Err(e) => Err(e),
            };
        }
        
        pipex!(@process sync_results $(=> $($rest)+)?)
    }};

    // Terminal case
    (@process $input:expr) => {
        $input
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Copy, Clone, PartialEq)]
    enum TestError {
        Error1,
        Error2,
    }

    const TEST_INPUT: [i32; 3] = [1, 2, 3];

    #[test]
    fn test_basic_pipeline() {
        let result: [Result<i32, TestError>; 3] = pipex!(
            TEST_INPUT
            => |x| Ok(x * 2)
            => |x| Ok(x + 1)
        );
        
        assert!(result.iter().all(|r| r.is_ok()));
        for (i, r) in result.iter().enumerate() {
            assert_eq!(r.unwrap(), TEST_INPUT[i] * 2 + 1);
        }
    }

    #[test]
    fn test_error_handling() {
        let result: [Result<i32, TestError>; 3] = pipex!(
            TEST_INPUT
            => |x| if x == 2 { Err(TestError::Error1) } else { Ok(x * 2) }
        );
        
        assert!(result[0].is_ok());
        assert!(result[1].is_err());
        assert!(result[2].is_ok());
    }

    #[test]
    fn test_ignore_handler() {
        let result: [Result<i32, TestError>; 3] = pipex!(
            TEST_INPUT
            => |x| if x == 2 { Err(TestError::Error2) } else { Ok(x * 2) }
        );
        
        let handled = IgnoreHandler::handle_results(result);
        assert!(handled.iter().all(|r| r.is_ok()));
    }

    #[test]
    fn test_collect_handler() {
        let result: [Result<i32, TestError>; 3] = pipex!(
            TEST_INPUT
            => |x| if x == 2 { Err(TestError::Error1) } else { Ok(x * 2) }
        );
        
        let handled = CollectHandler::handle_results(result);
        assert_eq!(handled.iter().filter(|r| r.is_err()).count(), 1);
    }

    #[test]
    fn test_fail_fast() {
        let result: [Result<i32, TestError>; 3] = pipex!(
            TEST_INPUT
            => |x| if x == 2 { Err(TestError::Error1) } else { Ok(x * 2) }
        );
        
        let handled = FailFastHandler::handle_results(result);
        assert!(handled.iter().all(|r| r.is_err()));
        assert!(handled.iter().all(|r| r == &Err(TestError::Error1)));
    }

    #[test]
    fn test_with_strategy() {
        let result: [Result<i32, TestError>; 3] = with_strategy!(IgnoreHandler, {
            pipex!(
                TEST_INPUT
                => |x| if x == 2 { Err(TestError::Error1) } else { Ok(x * 2) }
            )
        });
        
        assert!(result.iter().all(|r| r.is_ok()));
    }

    #[test]
    fn test_error_skip_pipeline() {
        let result: [Result<i32, TestError>; 3] = pipex!(
            TEST_INPUT
            => |x| if x == 2 { Err(TestError::Error1) } else { Ok(x) }
            => |x| Ok(x + 100)  // This step should be skipped for errors
        );
        
        assert_eq!(result[0], Ok(101));     // First value processed normally
        assert_eq!(result[1], Err(TestError::Error1));  // Error preserved, not modified
        assert_eq!(result[2], Ok(103));     // Third value processed normally
    }
}