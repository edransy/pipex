//! Core traits for pipeline functionality

use crate::PipexResult;

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

// PipelineResultHandler implementation for Vec<PipexResult<T, E>>
impl<T, E> PipelineResultHandler<T, E> for Vec<PipexResult<T, E>> 
where
    T: 'static + Clone,
    E: std::fmt::Debug + 'static + Clone,
{
    fn handle_pipeline_results(self) -> Vec<Result<T, E>> {
        if let Some(first) = self.first() {
            let strategy_name = first.strategy_name;
            let inner_results: Vec<Result<T, E>> = self
                .into_iter()
                .map(|pipex_result| pipex_result.result)
                .collect();
            
            #[cfg(test)]
            {
                crate::tests::apply_strategy(strategy_name, inner_results)
            }
            #[cfg(not(test))]
            {
                crate::apply_strategy(strategy_name, inner_results)
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

#[doc(hidden)]
pub trait IntoPipelineItem {
    type OutputValue;
    type PipelineItem;

    fn into_pipeline_item(self) -> Self::PipelineItem;
}

#[doc(hidden)]
impl<T: 'static, E: std::fmt::Debug> IntoPipelineItem for Result<T, E> {
    type OutputValue = T;
    type PipelineItem = Result<T, String>;

    fn into_pipeline_item(self) -> Result<T, String> {
        self.map_err(|e| format!("{:?}", e))
    }
}

#[doc(hidden)]
impl<T: 'static, E: std::fmt::Debug> IntoPipelineItem for PipexResult<T, E> {
    type OutputValue = T;
    type PipelineItem = PipexResult<T, String>;

    fn into_pipeline_item(self) -> PipexResult<T, String> {
        PipexResult {
            result: self.result.map_err(|e| format!("{:?}", e)), 
            strategy_name: self.strategy_name,
        }
    }
}

/// Marker trait for pure functions.
/// 
/// This trait is automatically implemented by the `#[pure]` macro for functions
/// that are verified to be pure. Pure functions are deterministic, have no side effects,
/// and are safe to use in pipelines.
/// 
/// # Safety
/// 
/// This trait should only be implemented by the `#[pure]` macro. Manual implementation
/// is not supported and may lead to undefined behavior.
#[doc(hidden)]
pub trait IsPure {
    // This trait has no methods - it's just a marker
}
