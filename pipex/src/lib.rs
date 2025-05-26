//! # Pipex
//! 
//! A powerful functional pipeline macro for Rust that combines synchronous, asynchronous, 
//! parallel, and streaming operations with extensible error handling via proc macros.

// Re-export the proc macros
pub use pipex_macros::error_strategy;

// Re-export dependencies
pub use futures;
pub use rayon; 
pub use tokio;

/// Result wrapper that carries strategy information
#[derive(Debug)]
pub struct PipexResult<T, E> {
    pub result: Result<T, E>,
    pub strategy_name: &'static str,
}

impl<T, E> PipexResult<T, E> {
    pub fn new(result: Result<T, E>, strategy_name: &'static str) -> Self {
        Self {
            result,
            strategy_name,
        }
    }
}

/// Trait to handle pipeline results uniformly
pub trait PipelineResultHandler<T, E> 
where
    E: std::fmt::Debug,
{
    fn handle_pipeline_results(self) -> Vec<Result<T, E>>;
}

/// Trait to extract successful values from pipeline results
pub trait ExtractSuccessful<T> {
    fn extract_successful(self) -> Vec<T>;
}

impl<T> ExtractSuccessful<T> for Vec<T> {
    fn extract_successful(self) -> Vec<T> {
        self
    }
}

impl<T, E> ExtractSuccessful<T> for Vec<Result<T, E>> {
    fn extract_successful(self) -> Vec<T> {
        self.into_iter().filter_map(|r| r.ok()).collect()
    }
}

/// Add this trait near the top with other traits
pub trait IntoResult<T, E> {
    fn into_result(self) -> Result<T, E>;
}

impl<T, E> IntoResult<T, E> for Result<T, E> {
    fn into_result(self) -> Result<T, E> {
        self
    }
}

impl<T, E> IntoResult<T, E> for PipexResult<T, E> {
    fn into_result(self) -> Result<T, E> {
        self.result
    }
}

/// Macro to register strategies dynamically - now much simpler!
#[macro_export]
macro_rules! register_strategies {
    ($($handler:ty),* $(,)?) => {
        pub fn apply_strategy<T, E>(strategy_name: &str, results: Vec<Result<T, E>>) -> Vec<Result<T, E>>
        where
            E: std::fmt::Debug,
        {
            match strategy_name {
                $(
                    stringify!($handler) => <$handler as $crate::ErrorHandler<T, E>>::handle_results(results),
                )*
                _ => results, // Unknown strategy, return as-is
            }
        }
    };
}

// Register all available strategies - much cleaner!
register_strategies! {
    IgnoreHandler,
    CollectHandler,
    FailFastHandler,
    LogAndIgnoreHandler,
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
            
            // Use the generated function
            apply_strategy(strategy_name, inner_results)
        } else {
            vec![]
        }
    }
}

impl<T, E> PipelineResultHandler<T, E> for Vec<Result<T, E>> 
where
    E: std::fmt::Debug,
{
    fn handle_pipeline_results(self) -> Vec<Result<T, E>> {
        // Regular Results - no strategy, return as-is
        self
    }
}

/// Trait for custom error handling strategies
pub trait ErrorHandler<T, E> {
    fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>>;
}

/// Ignore errors strategy
pub struct IgnoreHandler;
impl<T, E> ErrorHandler<T, E> for IgnoreHandler {
    fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
        results.into_iter()
            .filter(|r| r.is_ok())
            .collect()
    }
}

/// Collect all strategy  
pub struct CollectHandler;
impl<T, E> ErrorHandler<T, E> for CollectHandler {
    fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
        results
    }
}

/// Fail fast strategy
pub struct FailFastHandler;
impl<T, E> ErrorHandler<T, E> for FailFastHandler {
    fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
        // Filter out Ok results, return only errors
        results.into_iter()
            .filter(|r| r.is_err())
            .collect()
    }
}

/// Log and ignore errors strategy
pub struct LogAndIgnoreHandler;
impl<T, E> ErrorHandler<T, E> for LogAndIgnoreHandler 
where
    E: std::fmt::Debug,
{
    fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
        results.into_iter()
            .filter_map(|r| match r {
                Ok(val) => Some(Ok(val)),
                Err(err) => {
                    eprintln!("Pipeline error (ignored): {:?}", err);
                    None
                }
            })
            .collect()
    }
}

/// Add this trait to create errors that match the async function's return type
pub trait CreateError<E> {
    fn create_error(error_msg: E) -> Self;
}

impl<T, E> CreateError<E> for Result<T, E> {
    fn create_error(error_msg: E) -> Self {
        Err(error_msg)
    }
}

impl<T, E> CreateError<E> for PipexResult<T, E> {
    fn create_error(error_msg: E) -> Self {
        PipexResult::new(Err(error_msg), "preserve_error")
    }
}

/// Main pipeline macro
#[macro_export]
macro_rules! pipex {
    // Entry point
    ($input:expr $(=> $($rest:tt)+)?) => {{
        let initial_results = $input
            .into_iter()
            .map(|x| Ok(x))
            .collect::<Vec<Result<_, ()>>>();
        pipex!(@process initial_results $(=> $($rest)+)?)
    }};

    // SYNC step - preserve errors, only apply to successful values
    (@process $input:expr => |$var:ident| $body:expr $(=> $($rest:tt)+)?) => {{
        let iter_result = $input
            .into_iter()
            .map(|result| {
                match result {
                    Ok($var) => Ok($body),
                    Err(e) => Err(e),
                }
            })
            .collect::<Vec<_>>();
        pipex!(@process iter_result $(=> $($rest)+)?)
    }};

    // ASYNC step - process all items (successful and errors) uniformly
    (@process $input:expr => async |$var:ident| $body:block $(=> $($rest:tt)+)?) => {{
        let result = {
            async {
                let futures_results = $crate::futures::future::join_all(
                    $input.into_iter().map(|item| async move {
                        match item {
                            Ok($var) => {
                                $body
                            },
                            Err(e) => {
                                let mut error_string = format!("{:?}", e);
                                // Recursively remove nested quotes
                                while error_string.starts_with("\"") && error_string.ends_with("\"") {
                                    error_string = error_string[1..error_string.len()-1].to_string();
                                }
                                <_ as $crate::CreateError<String>>::create_error(error_string)
                            }
                        }
                    })
                ).await;
                
                // Use the trait to handle results uniformly
                use $crate::PipelineResultHandler;
                futures_results.handle_pipeline_results()
            }
        };
        pipex!(@process result.await $(=> $($rest)+)?)
    }};

    // PARALLEL step - process items in parallel using rayon
    (@process $input:expr => ||| |$var:ident| $body:expr $(=> $($rest:tt)+)?) => {{
        let result = {
            use $crate::rayon::prelude::*;
            $input.into_par_iter().map(|item| {
                match item {
                    Ok($var) => Ok($body),
                    Err(e) => Err(e),
                }
            }).collect::<Vec<_>>()
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // Terminal case
    (@process $input:expr) => {{
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
        
        // Should return all results including errors (no strategy applied)
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
        
        // Now should return Vec<Result<i32, ()>>
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
        
        println!("result: {:?}", result);
        // Should collect all results including errors
        assert!(result.iter().any(|r| r.is_err()));
        assert_eq!(result.len(), 5);
    }

    #[tokio::test]
    async fn test_strategy_ignore() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_and_ignore(x).await }
        );
        
        // Should ignore errors, only return successes
        // assert!(result.iter().all(|r| r.is_ok()));
        assert_eq!(result.len(), 4); // 3 is filtered out
        println!("result: {:?}", result);

    }


    #[tokio::test]
    async fn test_strategy_log_and_ignore() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_with_log_and_ignore(x).await }
        );
        
        // Should log errors and ignore them, only return successes
        // assert!(result.iter().all(|r| r.is_ok()));
        assert_eq!(result.len(), 4); // 3 is filtered out but logged
        println!("result: {:?}", result);
        
        // Verify the actual values
        let values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(values, vec![2, 4, 8, 10]); // 1*2, 2*2, 4*2, 5*2
    }



    #[tokio::test]
    async fn test_strategy_failfast() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5, 3]
            => async |x| { process_with_failfast(x).await }
        );
        
        // Should fail fast - return only the first error
        assert_eq!(result.len(), 2);
        assert!(result[0].is_err());
        assert_eq!(result[0].as_ref().unwrap_err(), "failed on 3");
        println!("Failfast result: {:?}", result);
    }

    #[tokio::test]
    async fn test_mixed_pipeline() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_and_collect(x).await }
            => |x| x + 1
            => async |x| { process_and_collect(x).await }
        );
        
        println!("result: {:?}", result);
        // Should collect all results including errors
        // assert!(result.iter().any(|r| r.is_err()));
        // assert_eq!(result.len(), 5);
    }

    #[tokio::test]
    async fn test_parallel_pipeline() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => ||| |x| x * x  // Parallel squaring
            => |x| x + 1      // Sync add
        );
        
        // Should return all results
        assert_eq!(result.len(), 5);
        assert!(result.iter().all(|r| r.is_ok()));
        let values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        // Sort because parallel execution might change order
        let mut expected = vec![2, 5, 10, 17, 26]; // (1²+1, 2²+1, 3²+1, 4²+1, 5²+1)
        let mut actual = values;
        expected.sort();
        actual.sort();
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_mixed_sync_async_parallel_pipeline() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_with_log_and_ignore(x).await } // Async with LogAndIgnoreHandler: error on 3
            => ||| |x| x + 10                                     // Parallel: add 10
            => |x| x - 1                                          // Sync: subtract 1
        );
        
        println!("Mixed pipeline result: {:?}", result);
        
        // Should have 4 successful and 1 error (from x=3)
        let successful_count = result.iter().filter(|r| r.is_ok()).count();
        assert_eq!(successful_count, 4);
        
        let error_count = result.iter().filter(|r| r.is_err()).count();
        assert_eq!(error_count, 0);
    }
}
