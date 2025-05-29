//! Core traits for pipeline functionality

use crate::PipexResult;
use crate::handlers::ErrorHandler;  // Import the ErrorHandler trait

// Import apply_strategy from tests module when testing
#[cfg(test)]
use crate::tests::apply_strategy;

/// Trait to handle pipeline results uniformly
/// 
/// This trait provides a common interface for handling different types of
/// pipeline results, whether they are regular `Result`s or `PipexResult`s
/// with associated error handling strategies.
pub trait PipelineResultHandler<T, E> 
where
    E: std::fmt::Debug,
{
    /// Handle pipeline results according to their type and strategy
    fn handle_pipeline_results(self) -> Vec<Result<T, E>>;
}

/// Trait to extract successful values from pipeline results
/// 
/// This trait provides a convenient way to extract only the successful
/// values from a collection of results, ignoring any errors.
/// 
/// # Examples
/// 
/// ```rust
/// use pipex::ExtractSuccessful;
/// 
/// let results = vec![Ok(1), Err("error"), Ok(3)];
/// let successes: Vec<i32> = results.extract_successful();
/// assert_eq!(successes, vec![1, 3]);
/// ```
pub trait ExtractSuccessful<T> {
    /// Extract only the successful values, discarding errors
    fn extract_successful(self) -> Vec<T>;
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

// Implementations for Vec<T>
impl<T> ExtractSuccessful<T> for Vec<T> {
    fn extract_successful(self) -> Vec<T> {
        self
    }
}

// Implementations for Vec<Result<T, E>>
impl<T, E> ExtractSuccessful<T> for Vec<Result<T, E>> {
    fn extract_successful(self) -> Vec<T> {
        self.into_iter().filter_map(|r| r.ok()).collect()
    }
}

// Implementations for Result<T, E>
impl<T, E> IntoResult<T, E> for Result<T, E> {
    fn into_result(self) -> Result<T, E> {
        self
    }
}

impl<T, E> CreateError<E> for Result<T, E> {
    fn create_error(error_msg: E) -> Self {
        Err(error_msg)
    }
}

// Implementations for PipexResult<T, E>
impl<T, E> IntoResult<T, E> for PipexResult<T, E> {
    fn into_result(self) -> Result<T, E> {
        self.result
    }
}

impl<T, E> CreateError<E> for PipexResult<T, E> {
    fn create_error(error_msg: E) -> Self {
        PipexResult::new(Err(error_msg), "preserve_error")
    }
}

// Default apply_strategy for non-test builds
#[cfg(not(test))]
fn apply_strategy_fallback<T, E>(strategy_name: &str, results: Vec<Result<T, E>>) -> Vec<Result<T, E>>
where
    E: std::fmt::Debug,
{
    match strategy_name {
        "IgnoreHandler" => crate::IgnoreHandler::handle_results(results),
        "CollectHandler" => crate::CollectHandler::handle_results(results),
        "FailFastHandler" => crate::FailFastHandler::handle_results(results),
        "LogAndIgnoreHandler" => crate::LogAndIgnoreHandler::handle_results(results),
        _ => {
            eprintln!("Warning: Unknown strategy '{}'. Call apply_strategies! to register custom handlers.", strategy_name);
            results
        }
    }
}

// PipelineResultHandler implementation for Vec<PipexResult<T, E>>
impl<T, E> PipelineResultHandler<T, E> for Vec<PipexResult<T, E>> 
where
    E: std::fmt::Debug,
{
    fn handle_pipeline_results(self) -> Vec<Result<T, E>> {
        if let Some(first) = self.first() {
            let strategy_name = first.strategy_name;
            let inner_results: Vec<Result<T, E>> = self
                .into_iter()
                .map(|pipex_result| pipex_result.result)
                .collect();
            
            // Use test module's apply_strategy when testing, fallback otherwise
            #[cfg(test)]
            {
                crate::tests::apply_strategy(strategy_name, inner_results)
            }
            #[cfg(not(test))]
            {
                apply_strategy_fallback(strategy_name, inner_results)
            }
        } else {
            vec![]
        }
    }
}

// PipelineResultHandler implementation for Vec<Result<T, E>>
impl<T, E> PipelineResultHandler<T, E> for Vec<Result<T, E>> 
where
    E: std::fmt::Debug,
{
    fn handle_pipeline_results(self) -> Vec<Result<T, E>> {
        // Regular Results - no strategy, return as-is
        self
    }
} 