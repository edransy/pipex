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

    // SYNC step - process all items (successful and errors) uniformly like async
    (@process $input:expr => |$var:ident| $body:expr $(=> $($rest:tt)+)?) => {{
        let sync_results = $input
            .into_iter()
            .map(|item_result| {
                match item_result {
                    Ok($var) => {
                        use $crate::traits::IntoPipelineItem;
                        ($body).into_pipeline_item()
                    },
                    Err(e) => {
                        let mut error_string = format!("{:?}", e);
                        while error_string.starts_with("\"") && error_string.ends_with("\"") {
                            error_string = error_string[1..error_string.len()-1].to_string();
                        }
                        <_ as $crate::CreateError<String>>::create_error(error_string)
                    }
                }
            })
            .collect::<Vec<_>>();
        
        use $crate::PipelineResultHandler;
        let iter_result = sync_results.handle_pipeline_results();
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
                                    while error_string.starts_with("\"") && error_string.ends_with("\"") {
                                        error_string = error_string[1..error_string.len()-1].to_string();
                                    }
                                    <_ as $crate::CreateError<String>>::create_error(error_string)
                                }
                            }
                        })
                    ).await;
                    
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

    // PARALLEL step - process items in parallel with uniform error handling
    (@process $input:expr => ||| |$var:ident| $body:expr $(=> $($rest:tt)+)?) => {{
        let result = {
            #[cfg(feature = "parallel")]
            {
                use $crate::rayon::prelude::*;
                let parallel_results_intermediate = $input.into_par_iter().map(|item_result| {
                    match item_result {
                        Ok($var) => {
                            use $crate::traits::IntoPipelineItem;
                            ($body).into_pipeline_item()
                        },
                        Err(e) => {
                            let mut error_string = format!("{:?}", e);
                            while error_string.starts_with("\"") && error_string.ends_with("\"") {
                                error_string = error_string[1..error_string.len()-1].to_string();
                            }
                            <_ as $crate::CreateError<String>>::create_error(error_string)
                        }
                    }
                }).collect::<Vec<_>>();

                use $crate::PipelineResultHandler;
                parallel_results_intermediate.handle_pipeline_results()
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

/// Convenience macro to register multiple strategies at once
/// 
/// # Examples
/// 
/// ```rust,ignore
/// register_strategies! {
///     "MyHandler" => MyHandler::handle_results,
///     "AnotherHandler" => AnotherHandler::handle_results,
/// }
/// ```
#[macro_export]
macro_rules! register_strategies {
    ($($handler:ident),+ $(,)? for <$t:ty, $e:ty>) => {
        $(
            $crate::register_strategy::<$t, $e>(stringify!($handler), $handler::handle_results);
        )+
    };
}
