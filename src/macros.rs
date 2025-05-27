//! Pipeline macro implementation

/// Main pipeline macro
/// 
/// The `pipex!` macro provides a functional pipeline syntax for chaining
/// operations across synchronous, asynchronous, and parallel processing.
/// 
/// # Syntax
/// 
/// - `|x| expr` - Synchronous transformation
/// - `async |x| { ... }` - Asynchronous operation  
/// - `||| |x| expr` - Parallel processing (requires "parallel" feature)
/// 
/// # Examples
/// 
/// Basic synchronous pipeline:
/// ```rust
/// use pipex::pipex;
/// 
/// let result = pipex!(
///     vec![1, 2, 3]
///     => |x| x * 2
///     => |x| x + 1
/// );
/// ```
/// 
/// Mixed async/sync pipeline:
/// ```rust,no_run
/// use pipex::pipex;
/// 
/// async fn double(x: i32) -> Result<i32, String> {
///     Ok(x * 2)
/// }
/// 
/// #[tokio::main]
/// async fn main() {
///     let result = pipex!(
///         vec![1, 2, 3]
///         => async |x| { double(x).await }
///         => |x| x + 1
///     );
/// }
/// ```
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
                #[cfg(feature = "async")]
                {
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
                #[cfg(not(feature = "async"))]
                {
                    compile_error!("Async pipeline operations require the 'async' feature to be enabled");
                }
            }
        };
        pipex!(@process result.await $(=> $($rest)+)?)
    }};

    // PARALLEL step - process items in parallel with error handling
    (@process $input:expr => ||| |$var:ident| $body:expr $(=> $($rest:tt)+)?) => {{
        let result = {
            #[cfg(feature = "parallel")]
            {
                use $crate::rayon::prelude::*;
                $input.into_par_iter().map(|item| {
                    match item {
                        Ok($var) => {
                            // Wrap the result in Ok() to ensure it's a Result type
                            Ok($body)
                        },
                        Err(e) => {
                            // Preserve error with smart unnesting
                            let mut error_string = format!("{:?}", e);
                            while error_string.starts_with("\"") && error_string.ends_with("\"") {
                                error_string = error_string[1..error_string.len()-1].to_string();
                            }
                            Err(error_string)
                        }
                    }
                }).collect::<Vec<Result<_, String>>>()
            }
            #[cfg(not(feature = "parallel"))]
            {
                compile_error!("Parallel pipeline operations require the 'parallel' feature to be enabled");
            }
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // Terminal case
    (@process $input:expr) => {{
        $input.into_iter().collect::<Vec<_>>()
    }};
} 