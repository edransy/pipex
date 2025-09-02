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

    // GPU AUTO step - automatic Rust-to-WGSL transpilation 
    (@process $input:expr => gpu ||| |$var:ident| $body:expr $(=> $($rest:tt)+)?) => {{
        let result = {
            #[cfg(feature = "gpu")]
            {
                async {
                    // Collect successful inputs for GPU processing
                    let mut gpu_inputs = Vec::new();
                    let mut input_items = Vec::new();
                    let mut success_indices = Vec::new();
                    
                    for (idx, item_result) in $input.into_iter().enumerate() {
                        match item_result {
                            Ok(item) => {
                                gpu_inputs.push(item);
                                success_indices.push(idx);
                                input_items.push(Ok(()));  // Placeholder for successful items
                            },
                            Err(e) => {
                                input_items.push(Err(e)); // Preserve errors
                            }
                        }
                    }
                    
                    // Auto-generate WGSL kernel from Rust expression
                    let transpiled_kernel = $crate::gpu::transpile_rust_expression(stringify!($body), stringify!($var));
                    
                    // Execute GPU kernel if we have inputs
                    let gpu_results = if !gpu_inputs.is_empty() {
                        let gpu_inputs_clone = gpu_inputs.clone();
                        match $crate::gpu::execute_gpu_kernel(gpu_inputs, &transpiled_kernel).await {
                            Ok(results) => results,
                            Err(gpu_error) => {
                                // If GPU fails, fallback to CPU execution
                                eprintln!("⚠️ GPU execution failed, falling back to CPU: {}", gpu_error);
                                
                                // CPU fallback processing using cloned inputs
                                gpu_inputs_clone.into_iter().map(|$var| $body).collect::<Vec<_>>()
                            }
                        }
                    } else {
                        Vec::new()
                    };
                    
                    // Map results back to their original positions
                    let mut gpu_idx = 0;
                    input_items.into_iter().map(|item_result| {
                        match item_result {
                            Ok(_) => {
                                let result = Ok(gpu_results[gpu_idx].clone());
                                gpu_idx += 1;
                                result
                            },
                            Err(e) => Err(format!("{:?}", e)), // Convert error to String
                        }
                    }).collect::<Vec<_>>()
                }
            }
            #[cfg(not(feature = "gpu"))]
            {
                // Fallback to CPU parallel processing when GPU not available
                use $crate::rayon::prelude::*;
                $input.into_par_iter().map(|item_result| {
                    match item_result {
                        Ok($var) => Ok($body),
                        Err(e) => Err(format!("{:?}", e)),
                    }
                }).collect::<Vec<_>>()
            }
        };
        pipex!(@process result.await $(=> $($rest)+)?)
    }};

    // GPU step - execute WGSL compute kernel on GPU
    (@process $input:expr => gpu $kernel:literal |$var:ident: Vec<$t:ty>| $body:block $(=> $($rest:tt)+)?) => {{
        let result = {
            #[cfg(feature = "gpu")]
            {
                async {
                    // Collect successful inputs for GPU processing
                    let mut gpu_inputs = Vec::new();
                    let mut input_items = Vec::new();
                    let mut success_indices = Vec::new();
                    
                    for (idx, item_result) in $input.into_iter().enumerate() {
                        match item_result {
                            Ok(item) => {
                                gpu_inputs.push(item);
                                success_indices.push(idx);
                                input_items.push(Ok(()));  // Placeholder for successful items
                            },
                            Err(e) => {
                                input_items.push(Err(e)); // Preserve errors
                            }
                        }
                    }
                    
                    // Execute GPU kernel if we have inputs
                    let gpu_results = if !gpu_inputs.is_empty() {
                        match $crate::gpu::execute_gpu_kernel(gpu_inputs, $kernel).await {
                            Ok(results) => results,
                            Err(gpu_error) => {
                                // If GPU fails, return error for all successful input positions
                                let error_msg = format!("GPU execution failed: {}", gpu_error);
                                return input_items.into_iter().map(|item_result| {
                                    match item_result {
                                        Ok(_) => Err(error_msg.clone()),
                                        Err(e) => Err(format!("{:?}", e)), // Convert error to String
                                    }
                                }).collect::<Vec<_>>();
                            }
                        }
                    } else {
                        Vec::new()
                    };
                    
                    // Map GPU results back to their original positions
                    let mut gpu_idx = 0;
                    input_items.into_iter().map(|item_result| {
                        match item_result {
                            Ok(_) => {
                                let result = Ok(gpu_results[gpu_idx].clone());
                                gpu_idx += 1;
                                result
                            },
                            Err(e) => Err(format!("{:?}", e)), // Convert error to String
                        }
                    }).collect::<Vec<_>>()
                }
            }
            #[cfg(not(feature = "gpu"))]
            {
                compile_error!("GPU pipeline operations require the 'gpu' feature to be enabled");
            }
        };
        pipex!(@process result.await $(=> $($rest)+)?)
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
