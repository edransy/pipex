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

/// Macro to register strategies dynamically
#[macro_export]
macro_rules! register_strategies {
    ($($name:literal => $handler:ty),* $(,)?) => {
        pub fn apply_strategy<T, E>(strategy_name: &str, results: Vec<Result<T, E>>) -> Vec<Result<T, E>>
        where
            E: std::fmt::Debug,
        {
            match strategy_name {
                $(
                    $name => <$handler as $crate::ErrorHandler<T, E>>::handle_results(results),
                )*
                _ => results, // Unknown strategy, return as-is
            }
        }
    };
}

// Register all available strategies
register_strategies! {
    "ignore" => IgnoreHandler,
    "collect" => CollectHandler,
    "failfast" => FailFastHandler,
    "logandignore" => LogAndIgnoreHandler,
}

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
        // Check if there are any errors first
        if let Some(err_pos) = results.iter().position(|r| r.is_err()) {
            // Return just the first error
            vec![results.into_iter().nth(err_pos).unwrap()]
        } else {
            // No errors, return all results
            results
        }
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

    // ASYNC step - handles both PipexResult and regular Result functions
    (@process $input:expr => async |$var:ident| { $fn_name:ident($($args:tt)*).await } $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            async {
                let results = $crate::futures::future::join_all(
                    input.into_iter().map(|$var| async move { $fn_name($($args)*).await })
                ).await;

                // Use the trait to handle results uniformly
                use $crate::PipelineResultHandler;
                results.handle_pipeline_results()
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
        
        // Should return all results including errors (no strategy applied)
        assert_eq!(result.len(), 4);
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

    // Test functions with strategy decorators
    #[error_strategy(CollectHandler)]
    async fn process_with_collect(x: i32) -> Result<i32, String> {
        if x == 3 {
            Err("failed on 3".to_string())
        } else {
            Ok(x * 2)
        }
    }

    #[error_strategy(IgnoreHandler)]
    async fn process_with_ignore(x: i32) -> Result<i32, String> {
        if x == 3 {
            Err("failed on 3".to_string())
        } else {
            Ok(x * 2)
        }
    }

    #[tokio::test]
    async fn test_strategy_collect() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_with_collect(x).await }
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
            => async |x| { process_with_ignore(x).await }
        );
        
        // Should ignore errors, only return successes
        assert!(result.iter().all(|r| r.is_ok()));
        assert_eq!(result.len(), 4); // 3 is filtered out
        println!("result: {:?}", result);

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
    async fn test_strategy_log_and_ignore() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_with_log_and_ignore(x).await }
        );
        
        // Should log errors and ignore them, only return successes
        assert!(result.iter().all(|r| r.is_ok()));
        assert_eq!(result.len(), 4); // 3 is filtered out but logged
        println!("result: {:?}", result);
        
        // Verify the actual values
        let values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(values, vec![2, 4, 8, 10]); // 1*2, 2*2, 4*2, 5*2
    }
}
