//! # Pipex
//! 
//! A powerful functional pipeline macro for Rust that combines synchronous, asynchronous, 
//! parallel, and streaming operations in a single, intuitive syntax.
//!
//! ## Features
//!
//! - **Sync Operations**: Chain regular synchronous transformations
//! - **Async Operations**: Handle asynchronous work with automatic await
//! - **Parallel Processing**: Leverage multiple CPU cores with configurable thread pools
//! - **Streaming**: Process large datasets with configurable buffer sizes
//! - **Error Handling**: Built-in Result handling with `async?` syntax
//! - **Mixed Workloads**: Seamlessly combine different operation types
//!
//! ## Quick Start
//!
//! ```rust
//! use pipex::pipex;
//!
//! // Simple synchronous pipeline
//! let result = pipex!(
//!     vec![1, 2, 3, 4, 5]
//!     => |x| x * 2
//!     => |x| x + 1
//! );
//! assert_eq!(result, vec![3, 5, 7, 9, 11]);
//! ```
//!
//! ## Pipeline Syntax
//!
//! - `|x| expr` - Synchronous transformation
//! - `async |x| { ... }` - Asynchronous operation
//! - `||| threads |x| expr` - Parallel processing with custom thread count
//! - `~async buffer |x| { ... }` - Streaming with custom buffer size
//! - `async? |x| { ... }` - Async with automatic Result unwrapping
//!

// Re-export dependencies so users don't need to add them explicitly
pub use futures;
pub use rayon;
pub use tokio;

/// The main pipeline macro that enables functional-style data processing 
/// with sync, async, parallel, and streaming operations.
#[macro_export]
macro_rules! pipex {
    // Entry point - auto-detect if input is iterator or collection
    ($input:expr $(=> $($rest:tt)+)?) => {{
        pipex!(@process $input $(=> $($rest)+)?)
    }};

    // SYNC step - keep as iterator, no auto-collect
    (@process $input:expr => |$var:ident| $body:expr $(=> $($rest:tt)+)?) => {{
        let iter_result = $input.into_iter().map(|$var| $body);
        pipex!(@process iter_result $(=> $($rest)+)?)
    }};

    // ASYNC step - force collection here since we need owned values
    (@process $input:expr => async |$var:ident| $body:block $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            async {
                $crate::futures::future::join_all(input.into_iter().map(|$var| async move $body)).await
            }.await
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // PARALLEL step with configurable thread count
    (@process $input:expr => ||| $num_threads:tt |$var:ident| $body:expr $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            let pool = $crate::rayon::ThreadPoolBuilder::new()
                .num_threads($num_threads)
                .build()
                .expect("Failed to create thread pool");
            pool.install(|| {
                use $crate::rayon::prelude::*;
                input.into_par_iter().map(|$var| $body).collect::<Vec<_>>()
            })
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // PARALLEL step with default thread count (all cores) - for backwards compatibility
    (@process $input:expr => ||| |$var:ident| $body:expr $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            use $crate::rayon::prelude::*;
            input.into_par_iter().map(|$var| $body).collect::<Vec<_>>()
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // STREAM step with configurable buffer size
    (@process $input:expr => ~async $buffer_size:tt |$var:ident| $body:block $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            use $crate::futures::StreamExt;
            $crate::futures::stream::iter(input)
                .map(|$var| async move $body)
                .buffer_unordered($buffer_size)
                .collect::<Vec<_>>()
                .await
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // STREAM step with default buffer size (10) - for backwards compatibility
    (@process $input:expr => ~async |$var:ident| $body:block $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            use $crate::futures::StreamExt;
            $crate::futures::stream::iter(input)
                .map(|$var| async move $body)
                .buffer_unordered(10)  // Default buffer size
                .collect::<Vec<_>>()
                .await
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // EXPLICIT COLLECT - when you want to force collection
    (@process $input:expr => collect $(=> $($rest:tt)+)?) => {{
        let result = pipex!(@ensure_vec $input);
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // Terminal case - auto-collect at the end
    (@process $input:expr) => {{
        pipex!(@ensure_vec $input)
    }};

    // Helper to ensure we have a Vec when needed
    (@ensure_vec $input:expr) => {{
        $input.into_iter().collect::<Vec<_>>()
    }};

    // Add this pattern to handle Results in async steps
    (@process $input:expr => async? |$var:ident| $body:block $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            let futures_results = $crate::futures::future::join_all(input.into_iter().map(|$var| async move $body)).await;
            futures_results.into_iter().filter_map(Result::ok).collect::<Vec<_>>()
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // Corrected macro pattern for streaming parallel execution
    (@process $input:expr => |~| $threads:tt, $buffer:tt |$var:ident| $body:block $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            let pool = $crate::rayon::ThreadPoolBuilder::new()
                .num_threads($threads)
                .build()
                .expect("Failed to create thread pool");
            
            // Create a channel for passing successful results to the thread pool
            let (tx, mut rx) = $crate::tokio::sync::mpsc::channel($buffer);
            
            // Spawn async tasks and process results immediately
            let process_handle = $crate::tokio::spawn(async move {
                use $crate::futures::StreamExt;
                // Process input items with buffered concurrency
                $crate::futures::stream::iter(input)
                    .map(|$var| async move { $body })
                    .buffer_unordered($buffer)
                    .for_each(|res| {
                        let tx_clone = tx.clone();
                        async move {
                            let _ = tx_clone.send(res).await;
                        }
                    })
                    .await;
                
                // Close channel when all async work is done
                drop(tx);
            });

            // Spawn a task to collect results in parallel
            let collection_handle = $crate::tokio::task::spawn_blocking(move || {
                pool.install(|| {
                    let mut results = Vec::new();
                    while let Some(val) = rx.blocking_recv() {
                        results.push(val);
                    }
                    results
                })
            });

            // Wait for both async processing and collection to complete
            let _ = process_handle.await;
            collection_handle.await.unwrap()
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_pipeline() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => |x| x * 2
            => |x| x + 1
        );
        assert_eq!(result, vec![3, 5, 7, 9, 11]);
    }

    #[tokio::test]
    async fn test_async_pipeline() {
        let result = pipex!(
            vec![1, 2, 3]
            => async |x| {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                x * 2
            }
        );
        assert_eq!(result, vec![2, 4, 6]);
    }

    #[test]
    fn test_parallel_pipeline() {
        let result = pipex!(
            vec![1, 2, 3, 4]
            => ||| 2 |x| x * x
        );
        assert_eq!(result, vec![1, 4, 9, 16]);
    }

    #[tokio::test]
    async fn test_streaming_pipeline() {
        let result = pipex!(
            vec![1, 2, 3]
            => ~async 2 |x| {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                x + 10
            }
        );
        assert_eq!(result, vec![11, 12, 13]);
    }

    #[tokio::test]
    async fn test_mixed_pipeline() {
        let result = pipex!(
            vec![1, 2, 3, 4]
            => |x| x * 2                    // Sync
            => ||| 2 |x| x + 1              // Parallel
            => ~async 2 |x| {               // Async streaming
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                x * 3
            }
        );
        assert_eq!(result, vec![9, 15, 21, 27]);
    }
} 