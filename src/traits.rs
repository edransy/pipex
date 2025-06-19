//! Core traits for pipeline functionality

// use crate::PipexResult;

/// Trait for handling pipeline results
pub trait PipelineResultHandler<T, E, const N: usize> {
    /// Handle the results according to the strategy
    fn handle_pipeline_results(self) -> [Result<T, E>; N];
}

/// Default implementation for arrays of Results
impl<T, E, const N: usize> PipelineResultHandler<T, E, N> for [Result<T, E>; N] 
where
    E: Copy  // Add Copy bound for error type
{
    fn handle_pipeline_results(self) -> [Result<T, E>; N] {
        self
    }
}

/// Trait for extracting successful values from results
pub trait ExtractSuccessful<T, E, const N: usize> {
    /// Extract only successful values into a new array
    /// Returns a tuple of the array and the count of successful items
    fn extract_successful(self) -> ([T; N], usize);
}

impl<T, E, const N: usize> ExtractSuccessful<T, E, N> for [Result<T, E>; N] {
    fn extract_successful(self) -> ([T; N], usize) {
        let mut successful: [T; N] = unsafe { core::mem::zeroed() };
        let mut count = 0;
        
        for (_i, result) in self.into_iter().enumerate() {
            if let Ok(value) = result {
                successful[count] = value;
                count += 1;
            }
        }
        
        (successful, count)
    }
}

/// Trait to convert various result types into standard Result
/// 
/// This trait provides a uniform interface for converting different
/// result-like types into standard `Result<T, E>`.
pub trait IntoResult<T, E> {
    /// Convert into a standard Result
    fn into_result(self) -> Result<T, E>;
}

/// Trait to create errors that match the async function's return type
/// 
/// This trait is used internally by the pipeline macro to create
/// error values that have the same type as the function's return type.
pub trait CreateError<E> {
    /// Create an error value from the given error
    fn create_error(error_msg: E) -> Self;
}

#[doc(hidden)]
/// Core trait for pipeline operations
pub trait PipelineOp<T, E, const N: usize> {
    type Output;
    fn process(input: [Result<T, E>; N]) -> Self::Output;
}

#[doc(hidden)]
/// Trait for converting types into pipeline items
pub trait IntoPipelineItem<T, E> {
    fn into_pipeline_item(self) -> Result<T, E>;
}

/// Implementation for Result types
impl<T, E> IntoPipelineItem<T, E> for Result<T, E> {
    fn into_pipeline_item(self) -> Result<T, E> {
        self
    }
}

/// Error handling trait for pipeline operations
pub trait ErrorHandler<T, E, const N: usize> {
    /// Handle an array of Results according to the strategy
    fn handle_results(results: [Result<T, E>; N]) -> [Result<T, E>; N];
}

/// Ignore errors strategy
pub struct IgnoreHandler;

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

/// Collect errors strategy
pub struct CollectHandler;

impl<T, E, const N: usize> ErrorHandler<T, E, N> for CollectHandler {
    fn handle_results(results: [Result<T, E>; N]) -> [Result<T, E>; N] {
        results
    }
}

/// FailFast handler
pub struct FailFastHandler;

impl<T, E, const N: usize> ErrorHandler<T, E, N> for FailFastHandler 
where
    E: Copy  // Add Copy bound for error type
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

/// A marker trait for pure functions.
pub trait IsPure {}

// // Implementations for Vec<T>
// impl<T> ExtractSuccessful<T> for Vec<T> {
//     fn extract_successful(self) -> Vec<T> {
//         self
//     }
// }

// // Implementations for Vec<Result<T, E>>
// impl<T, E> ExtractSuccessful<T> for Vec<Result<T, E>> {
//     fn extract_successful(self) -> Vec<T> {
//         self.into_iter().filter_map(|r| r.ok()).collect()
//     }
// }

// // Implementations for Result<T, E>
// impl<T, E> IntoResult<T, E> for Result<T, E> {
//     fn into_result(self) -> Result<T, E> {
//         self
//     }
// }

// impl<T, E> CreateError<E> for Result<T, E> {
//     fn create_error(error_msg: E) -> Self {
//         Err(error_msg)
//     }
// }

// // Implementations for PipexResult<T, E>
// impl<T, E> IntoResult<T, E> for PipexResult<T, E> {
//     fn into_result(self) -> Result<T, E> {
//         self.result
//     }
// }

// impl<T, E> CreateError<E> for PipexResult<T, E> {
//     fn create_error(error_msg: E) -> Self {
//         PipexResult::new(Err(error_msg), "preserve_error")
//     }
// }

// // PipelineResultHandler implementation for Vec<PipexResult<T, E>>
// impl<T, E> PipelineResultHandler<T, E> for Vec<PipexResult<T, E>> 
// where
//     T: 'static + Clone,
//     E: core::fmt::Debug + 'static + Clone,
// {
//     fn handle_pipeline_results(self) -> Vec<Result<T, E>> {
//         if let Some(first) = self.first() {
//             let strategy_name = first.strategy_name;
//             let inner_results: Vec<Result<T, E>> = self
//                 .into_iter()
//                 .map(|pipex_result| pipex_result.result)
//                 .collect();
            
//             #[cfg(test)]
//             {
//                 crate::tests::apply_strategy(strategy_name, inner_results)
//             }
//             #[cfg(not(test))]
//             {
//                 crate::apply_strategy(strategy_name, inner_results)
//             }
//         } else {
//             vec![]
//         }
//     }
// }

// // PipelineResultHandler implementation for Vec<Result<T, E>>
// impl<T, E> PipelineResultHandler<T, E> for Vec<Result<T, E>> 
// where
//     E: std::fmt::Debug,
// {
//     fn handle_pipeline_results(self) -> Vec<Result<T, E>> {
//         // Regular Results - no strategy, return as-is
//         self
//     }
// }
