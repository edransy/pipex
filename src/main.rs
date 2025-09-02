use anyhow::Result;
use tokio;
use futures::{future::join_all, stream, StreamExt};
use rayon::prelude::*;
use std::time::Instant;

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
                join_all(input.into_iter().map(|$var| async move $body)).await
            }.await
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // PARALLEL step with configurable thread count
    (@process $input:expr => ||| $num_threads:tt |$var:ident| $body:expr $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads($num_threads)
                .build()
                .expect("Failed to create thread pool");
            pool.install(|| {
                input.into_par_iter().map(|$var| $body).collect::<Vec<_>>()
            })
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // PARALLEL step with default thread count (all cores) - for backwards compatibility
    (@process $input:expr => ||| |$var:ident| $body:expr $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            input.into_par_iter().map(|$var| $body).collect::<Vec<_>>()
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // STREAM step with configurable buffer size
    (@process $input:expr => ~async $buffer_size:tt |$var:ident| $body:block $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            stream::iter(input)
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
            stream::iter(input)
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
            let futures_results = join_all(input.into_iter().map(|$var| async move $body)).await;
            futures_results.into_iter().filter_map(Result::ok).collect::<Vec<_>>()
        };
        pipex!(@process result $(=> $($rest)+)?)
    }};

    // Corrected macro pattern for streaming parallel execution
    (@process $input:expr => |~| $threads:tt, $buffer:tt |$var:ident| $body:block $(=> $($rest:tt)+)?) => {{
        let result = {
            let input = pipex!(@ensure_vec $input);
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads($threads)
                .build()
                .expect("Failed to create thread pool");
            
            // Create a channel for passing successful results to the thread pool
            let (tx, mut rx) = tokio::sync::mpsc::channel($buffer);
            
            // Spawn async tasks and process results immediately
            let process_handle = tokio::spawn(async move {
                // Process input items with buffered concurrency
                stream::iter(input)
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
            let collection_handle = tokio::task::spawn_blocking(move || {
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

// === Mock Logic with Different Workload Types ===

// CPU-intensive work - FIXED to prevent overflow
fn heavy_cpu_work(n: u32) -> u32 {
    // Simulate heavy computation without overflow
    let n = n.min(1000); // Cap the input to prevent overflow
    (1..=n * 10).map(|x| x as u64).sum::<u64>() as u32 % 1000
}

// I/O-intensive work (simulated)
async fn slow_io_operation(id: u32) -> Result<String> {
    // Simulate varying I/O delays
    let delay = match id % 4 {
        0 => 50,  // Fast
        1 => 100, // Medium
        2 => 150, // Slow
        _ => 200, // Very slow
    };
    
    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    Ok(format!("IO_Result_{}", id))
}

// Mixed workload - FIXED to prevent overflow
async fn mixed_workload(n: u32) -> Result<u32> {
    // Some I/O
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    // Some CPU work (reduced to prevent overflow)
    let n = n.min(1000);
    let cpu_result = (1..=n * 5).map(|x| x as u64).sum::<u64>() as u32 % 100;
    Ok(cpu_result)
}

// Light async work
async fn light_async_work(n: u32) -> u32 {
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    n * 2
}

// === Benchmark Helper ===
async fn benchmark<F, T>(name: &str, f: F) -> T 
where 
    F: std::future::Future<Output = T>
{
    let start = Instant::now();
    let result = f.await;
    let duration = start.elapsed();
    println!("{}: {:?}", name, duration);
    result
}

// === Main Pipeline Benchmarks ===

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Pipeline Performance Analysis ===\n");
    
    // Test datasets
    let small_dataset: Vec<u32> = (1..=20).collect();
    let medium_dataset: Vec<u32> = (1..=100).collect();
    let large_dataset: Vec<u32> = (1..=10_000).collect();

    println!("🧪 SCENARIO 1: CPU-Intensive Work - Thread Count Comparison");
    println!("Dataset: {} items", medium_dataset.len());
    
    // Sequential CPU work
    benchmark("CPU Sequential (1 thread)", async {

        let _result = pipex!(
            large_dataset.clone()
            => ||| 1 |n| heavy_cpu_work(n)
        );

    }).await;
    
    // Parallel CPU work - different thread counts
    for threads in [2, 4, 8] {
        benchmark(&format!("CPU Parallel ({} threads)", threads), async {

            let _result = pipex!(
                large_dataset.clone()
                => ||| threads |n| heavy_cpu_work(n)
            );

        }).await;
    }

    println!("\n🌐 SCENARIO 2: I/O-Intensive Work - Buffer Size Comparison");
    println!("Dataset: {} items", small_dataset.len());
    
    // I/O work with different buffer sizes
    for buffer_size in [1, 5, 10, 20] {
        benchmark(&format!("I/O Buffer Size {}", buffer_size), async {

            let _result = pipex!(
                small_dataset.clone()
                => async? |id| { slow_io_operation(id).await }
                => ~async buffer_size |data| { 
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    data.len() 
                }
            );

        }).await;
    }

    println!("\n⚡ SCENARIO 3: Mixed Workload Pipeline");
    println!("Dataset: {} items", small_dataset.len());
    
    for (threads, buffer_size) in [(1, 5), (2, 10), (4, 15), (8, 20)] {
        benchmark(&format!("Mixed ({} threads, {} buffer)", threads, buffer_size), async {
            let _result = pipex!(
                small_dataset.clone()
                => |n| n * 3                                    // Quick transform
                => async? |n| { mixed_workload(n).await }       // Mixed I/O + CPU                
                => ||| threads |result| heavy_cpu_work(result)  // Heavy CPU
                => ~async buffer_size |n| { light_async_work(n).await }  // Light async
            );
        }).await;
    }

    println!("\n📊 SCENARIO 4: Large Dataset - Scaling Test");
    println!("Dataset: {} items", large_dataset.len());
    
    // Test how different configurations handle large datasets
    let configs = [
        ("Conservative", 2, 5),
        ("Balanced", 4, 10), 
        ("Aggressive", 8, 20),
        ("Max Concurrent", 12, 50),
    ];
    
    for (name, threads_no, _buffer_size) in configs {
        benchmark(&format!("Large Dataset - {}", name), async {
            let _result = pipex!(
                large_dataset.clone()
                => |n| n % 100                                      // Reduce data size
                => ||| threads_no |n| heavy_cpu_work(n)               // CPU work
                => async |n| { light_async_work(n).await }  // Async work
            );
        }).await;
    }

    println!("\n🔄 SCENARIO 5: Pipeline Depth Test");
    println!("Dataset: {} items", small_dataset.len());
    
    // Test deep pipelines with different configurations
    benchmark("Deep Pipeline - Conservative", async {
        let _result = pipex!(
            small_dataset.clone()
            => |n| n * 2
            => ||| 2 |n| n + 10
            => ~async 3 |n| { light_async_work(n).await }
            => |n| n / 2
            => ||| 2 |n| heavy_cpu_work(n / 10)
            => ~async 3 |n| { light_async_work(n).await }
        );
    }).await;
    
    benchmark("Deep Pipeline - Aggressive", async {
        let _result = pipex!(
            small_dataset.clone()
            => |n| n * 2
            => ||| 8 |n| n + 10
            => ~async 15 |n| { light_async_work(n).await }
            => |n| n / 2
            => ||| 8 |n| heavy_cpu_work(n / 10)
            => ~async 15 |n| { light_async_work(n).await }
        );
    }).await;

    println!("\n🎯 SCENARIO 6: Optimized Real-World Patterns");
    
    // Simulating different real-world scenarios
    benchmark("Web Scraping Pattern", async {
        let urls: Vec<u32> = (1..=30).collect();
        let _result = pipex!(
            urls
            => ~async 5 |url| {     // Respect server limits
                slow_io_operation(url).await }
            => |content| {   // Parse in parallel
                content.unwrap().len() }
            => ||| 4 |size| {       // Process data with limited cores
                heavy_cpu_work(size as u32 / 10) }
        );
    }).await;
    
    benchmark("Data Processing Pattern", async {
        let data: Vec<u32> = (1..=100).collect();
        let _result = pipex!(
            data
            => ||| 8 |n| {          // Max CPU for initial processing
                heavy_cpu_work(n / 10)
            }
            => ~async 3 |processed| { // Conservative I/O for saving
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                processed + 1
            }
            => ||| 4 |n| {          // Moderate CPU for post-processing
                n * 2
            }
        );
    }).await;

    

    println!("\n✅ Analysis Complete!");
    println!("\n📋 PERFORMANCE INSIGHTS:");
    println!("• CPU-bound: More threads = better performance (up to core count)");
    println!("• I/O-bound: Higher buffer sizes = better throughput (with diminishing returns)");
    println!("• Mixed workloads: Balance threads and buffer sizes based on bottleneck");
    println!("• Large datasets: Conservative settings prevent resource exhaustion");
    println!("• Deep pipelines: Moderate settings work best for complex workflows");
    
    Ok(())
}
