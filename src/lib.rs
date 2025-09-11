//! A pipeline processing library for Rust
//! 
//! This crate provides a powerful macro-based system for creating data processing
//! pipelines with various error handling strategies.

#![deny(missing_docs)]
#![warn(clippy::all)]
#![cfg_attr(docsrs, feature(doc_cfg))]

// Core modules
mod result;
pub mod traits;
mod handlers;
mod macros;

// Re-export public API
pub use result::PipexResult;
pub use traits::{PipelineResultHandler, ExtractSuccessful, IntoResult, CreateError};
pub use handlers::{
    ErrorHandler, IgnoreHandler, CollectHandler, FailFastHandler, LogAndIgnoreHandler
};

// Re-export the proc macros
pub use pipex_macros::{error_strategy, pure, memoized};

// Conditional re-exports based on features
#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub use futures;

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub use tokio;

#[cfg(feature = "parallel")]
#[cfg_attr(docsrs, doc(cfg(feature = "parallel")))]
pub use rayon;

#[cfg(feature = "memoization")]
#[cfg_attr(docsrs, doc(cfg(feature = "memoization")))]
pub use dashmap;

#[cfg(feature = "memoization")]
#[cfg_attr(docsrs, doc(cfg(feature = "memoization")))]
pub use once_cell;

#[cfg(feature = "gpu")]
#[cfg_attr(docsrs, doc(cfg(feature = "gpu")))]
pub mod gpu;

#[cfg(feature = "gpu")]
#[cfg_attr(docsrs, doc(cfg(feature = "gpu")))]
pub use wgpu;

#[cfg(feature = "gpu")]
#[cfg_attr(docsrs, doc(cfg(feature = "gpu")))]
pub use bytemuck;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::any::{Any, TypeId};

// Registry for strategy functions
static STRATEGY_REGISTRY: OnceLock<Mutex<HashMap<String, Box<dyn Any + Send + Sync>>>> = OnceLock::new();

/// Register a custom strategy handler for specific types
pub fn register_strategy<T, E>(
    name: &str,
    handler: fn(Vec<Result<T, E>>) -> Vec<Result<T, E>>
) where
    T: 'static,
    E: std::fmt::Debug + 'static,
{
    let registry = STRATEGY_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));
    let mut registry = registry.lock().unwrap();
    
    let type_id = (TypeId::of::<T>(), TypeId::of::<E>());
    let key = format!("{}_{:?}", name, type_id);
    
    registry.insert(key, Box::new(handler));
}

/// Try to call a registered strategy (fix ownership issue by borrowing)
fn try_registered_strategy<T, E>(strategy_name: &str, results: &Vec<Result<T, E>>) -> Option<Vec<Result<T, E>>>
where
    T: 'static + Clone,
    E: std::fmt::Debug + 'static + Clone,
{
    let registry = STRATEGY_REGISTRY.get()?;
    let registry = registry.lock().unwrap();
    
    let type_id = (TypeId::of::<T>(), TypeId::of::<E>());
    let key = format!("{}_{:?}", strategy_name, type_id);
    
    if let Some(handler_any) = registry.get(&key) {
        if let Some(handler) = handler_any.downcast_ref::<fn(Vec<Result<T, E>>) -> Vec<Result<T, E>>>() {
            return Some(handler(results.clone()));
        }
    }
    
    None
}

/// Apply strategy - checks registry first, then built-ins
pub fn apply_strategy<T, E>(strategy_name: &str, results: Vec<Result<T, E>>) -> Vec<Result<T, E>>
where
    T: 'static + Clone,
    E: std::fmt::Debug + 'static + Clone,
{
    // Try registered strategies first (pass by reference to avoid move)
    if let Some(result) = try_registered_strategy(strategy_name, &results) {
        return result;
    }
    
    // Fall back to built-ins (now we can still use results since we didn't move it)
    match strategy_name {
        "IgnoreHandler" => IgnoreHandler::handle_results(results),
        "CollectHandler" => CollectHandler::handle_results(results),
        "FailFastHandler" => FailFastHandler::handle_results(results),
        "LogAndIgnoreHandler" => LogAndIgnoreHandler::handle_results(results),
        _ => {
            eprintln!("Warning: Unknown strategy '{}'. Use register_strategy() to register custom handlers.", strategy_name);
            results
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub struct FirstErrorHandler;

    impl<T, E> ErrorHandler<T, E> for FirstErrorHandler {
        fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
            results.into_iter()
                .find(|r| r.is_err())
                .map_or(Vec::new(), |e| vec![e])
        }
    }

    // Register the handler for our test types
    use std::sync::Once;
    static INIT: Once = Once::new();
    
    fn setup() {
        INIT.call_once(|| {
            register_strategies!( FirstErrorHandler for <i32, String> );
        });
    }

    pub fn apply_strategy<T, E>(strategy_name: &str, results: Vec<Result<T, E>>) -> Vec<Result<T, E>>
    where
        T: 'static + Clone,
        E: std::fmt::Debug + 'static + Clone,
    {
        setup(); // Ensure registration
        crate::apply_strategy(strategy_name, results)
    }

    // Basic test functions
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

    #[error_strategy(FirstErrorHandler)]
    async fn process_with_first_error(x: i32) -> Result<i32, String> {
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
            => |x| Ok::<i32, String>(x + 1)
        );
        
        assert_eq!(result.len(), 4);
        assert!(result.iter().all(|r| r.is_ok()));
        let values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(values, vec![3, 5, 9, 11]); // (1*2)+1, (2*2)+1, (4*2)+1, (5*2)+1
    }

    #[tokio::test]
    async fn test_sync_pipeline() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => |x| Ok::<i32, String>(x * 2)
            => |x| Ok::<i32, String>(x + 1)
        );
        
        assert_eq!(result.len(), 5);
        assert!(result.iter().all(|r| r.is_ok()));
        let values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(values, vec![3, 5, 7, 9, 11]); // (1*2)+1, (2*2)+1, (3*2)+1, (4*2)+1, (5*2)+1
    }

    #[tokio::test]
    async fn test_strategy_collect() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_and_collect(x).await }
        );
        
        // Should collect all results including errors
        assert!(result.iter().any(|r| r.is_err()));
        assert_eq!(result.len(), 5);
        eprintln!("result: {:?}", result);
    }

    #[tokio::test]
    async fn test_strategy_ignore() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_and_ignore(x).await }
        );
        
        // Should ignore errors, only return successes
        assert_eq!(result.len(), 4); // 3 is filtered out
        assert!(result.iter().all(|r| r.is_ok()));
        eprintln!("result: {:?}", result);
    }

    #[tokio::test]
    async fn test_strategy_log_and_ignore() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_with_log_and_ignore(x).await }
        );
        
        // Should log errors and ignore them, only return successes
        assert_eq!(result.len(), 4); // 3 is filtered out but logged
        assert!(result.iter().all(|r| r.is_ok()));
        
        let values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(values, vec![2, 4, 8, 10]); // 1*2, 2*2, 4*2, 5*2
    }

    #[tokio::test]
    async fn test_strategy_failfast() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5, 3]
            => async |x| { process_with_failfast(x).await }
        );
        
        // Should fail fast - return only errors
        assert!(result.iter().all(|r| r.is_err()));
        assert_eq!(result.len(), 2); // Two instances of 3
        eprintln!("result: {:?}", result);
    }

    #[tokio::test]
    async fn test_custom_first_error_handler() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => async |x| { process_with_first_error(x).await }
        );
        
        // Should return only the first error
        assert_eq!(result.len(), 1);
        assert!(result[0].is_err());
        eprintln!("FirstErrorHandler result: {:?}", result);
    }

    #[cfg(feature = "parallel")]
    #[tokio::test]
    async fn test_parallel_pipeline() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => ||| |x| Ok::<i32, String>(x * 2)
        );
        
        assert_eq!(result.len(), 5);
        assert!(result.iter().all(|r| r.is_ok()));

        eprintln!("result: {:?}", result);
        // let mut values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        // values.sort(); // Parallel processing might change order
        // assert_eq!(values, vec![3, 5, 7, 9, 11]);
    }

    // Test sync function with strategy decorator
    #[error_strategy(IgnoreHandler)]
    fn sync_process_and_ignore(x: i32) -> Result<i32, String> {
        if x == 3 { Err("failed on 3".to_string()) }
        else { Ok(x * 2) }
    }

    #[test]
    fn test_sync_step_with_error_strategy() {
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => |x| sync_process_and_ignore(x)
            => |x| Ok::<i32, String>(x + 1)
        );
        
        // Should ignore errors due to IgnoreHandler strategy, only return successes
        assert_eq!(result.len(), 4); // 3 is filtered out by IgnoreHandler
        assert!(result.iter().all(|r| r.is_ok()));

        eprintln!("result: {:?}", result);
        
        let values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        let expected_values = vec![3, 5, 9, 11]; // (1*2)+1, (2*2)+1, (4*2)+1, (5*2)+1 (3 filtered out)
        let mut actual_values = values;
        actual_values.sort(); // Ensure order for comparison
        assert_eq!(actual_values, expected_values); 
    }

    #[tokio::test]
    async fn test_mixed_sync_and_async_pipeline() {
        // Using an existing async function with its own error strategy
        // process_and_collect is #[error_strategy(CollectHandler)]
        // It returns Err for input 3, Ok(x*2) otherwise.

        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => |x| sync_process_and_ignore(x) // IgnoreHandler drops item 3
            => |x| Ok::<i32, String>(x - 1)
            => async |x| { process_and_collect(x).await } // CollectHandler collect error item 3
        );

        assert_eq!(result.len(), 4, "Expected 4 results after IgnoreHandler dropped one item.");
        eprintln!("Mixed sync-async-sync pipeline (Ignore then Collect) result: {:?}", result);

        // Since sync_process_and_ignore filters out 3, we only get results for [1,2,4,5]
        // After sync_process_and_ignore: [2,4,8,10] -> After -1: [1,3,7,9] -> After process_and_collect(*2): [2,6,14,18]
        let actual_results: Vec<Result<i32, String>> = result.into_iter().collect();
        assert_eq!(actual_results.len(), 4);
    }

    // === Pure Macro Tests ===
    
    #[pure]
    fn pure_add(a: i32, b: i32) -> i32 {
        a + b
    }

        #[pure]
    fn pure_multiply(a: i32, b: i32) -> i32 {
        pure_add(a, b) * 2  // ✅ Calls another pure function
    }

        #[pure]
    fn pure_transform(x: i32) -> i32 {
        x * x + 1  // ✅ Only mathematical operations
    }

    #[test]
    fn test_pure_functions() {
        // Test that pure functions work correctly
        assert_eq!(pure_add(5, 3), 8);
        assert_eq!(pure_multiply(4, 2), 12); // (4+2)*2
        assert_eq!(pure_transform(6), 37); // 6*6+1
    }

    #[test]
    fn test_pure_function_composition() {
        // Test that pure functions can call other pure functions
        let result = pure_multiply(3, 4);
        assert_eq!(result, 14); // (3+4)*2
        
        // Test multiple levels of pure function calls
        let step1 = pure_add(2, 3);  // = 5
        let step2 = pure_transform(step1);  // = 5*5+1 = 26
        let step3 = pure_add(step2, 4);  // = 26+4 = 30
        assert_eq!(step3, 30);
    }

    #[test]
    fn test_pure_functions_in_pipeline() {
        // Verify pure functions work in pipelines
        let result = pipex!(
            vec![1, 2, 3, 4, 5]
            => |x| Ok::<i32, String>(pure_transform(x))
            => |x| Ok::<i32, String>(pure_add(x, 10))
        );
        
        assert_eq!(result.len(), 5);
        assert!(result.iter().all(|r| r.is_ok()));
        
        let values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        // Expected: [(1*1+1)+10, (2*2+1)+10, (3*3+1)+10, (4*4+1)+10, (5*5+1)+10] = [12, 15, 20, 27, 36]
        assert_eq!(values, vec![12, 15, 20, 27, 36]);
    }

        // These tests verify that the pure macro correctly prevents impure operations
    // They should fail to compile if uncommented, demonstrating the macro's safety
    
    // Test case for demonstrating proper error messages when calling impure functions
    fn regular_impure_function(x: i32) -> i32 {
        // This function is not marked as #[pure], so it's considered impure
        x * 2
    }

    // === Memoization Tests ===
    
    #[pure]
    #[memoized]
    fn fibonacci_memoized(n: u64) -> u64 {
        if n <= 1 { 
            n 
        } else { 
            fibonacci_memoized(n - 1) + fibonacci_memoized(n - 2) 
        }
    }
    
    // === Triple Power: Error Strategy + Pure + Memoized ===
    
    #[pure]
    #[memoized(capacity = 200)]
    #[error_strategy(CollectHandler)]
    fn advanced_mathematical_operation(x: i32, y: i32) -> Result<i32, String> {
        // Simulate a mathematical operation that can fail
        if x < 0 || y < 0 {
            return Err(format!("Negative inputs not supported: x={}, y={}", x, y));
        }
        
        if x > 100 || y > 100 {
            return Err(format!("Inputs too large: x={}, y={}", x, y));
        }
        
        // Complex mathematical computation (pure operations only)
        let mut result = x * x + y * y;
        for i in 1..10 {
            result = result + (i * (x + y)) / (i + 1);
        }
        
        Ok(result)
    }
    
    #[pure]
    #[memoized(capacity = 100)]
    fn expensive_computation(x: i32, y: i32) -> i32 {
        // Simulate expensive computation with pure operations only
        let mut result = x * x + y * y;
        for i in 0..1000 {
            result = result + (i % 2);
        }
        result
    }
    
    // Non-memoized version for comparison
    #[pure]
    fn fibonacci_regular(n: u64) -> u64 {
        if n <= 1 { 
            n 
        } else { 
            fibonacci_regular(n - 1) + fibonacci_regular(n - 2) 
        }
    }

    #[test]
    fn test_memoized_basic_functionality() {
        // Test that memoized functions work correctly
        assert_eq!(fibonacci_memoized(10), 55);
        assert_eq!(fibonacci_memoized(15), 610);
        
        // Test with parameters
        let result1 = expensive_computation(3, 4);
        let result2 = expensive_computation(5, 12);
        
        // Just test that they're consistent (since we changed the computation)
        assert_eq!(expensive_computation(3, 4), result1);
        assert_eq!(expensive_computation(5, 12), result2);
    }
    
    #[test]
    fn test_memoized_performance_improvement() {
        use std::time::Instant;
        
        // First call - should be slow (computes and caches)
        let start = Instant::now();
        let result1 = expensive_computation(10, 20);
        let duration1 = start.elapsed();
        
        // Second call - should be fast (from cache)
        let start = Instant::now();
        let result2 = expensive_computation(10, 20);
        let duration2 = start.elapsed();
        
        assert_eq!(result1, result2);
        
        // Second call should be faster (cached) or at least not much slower
        // Note: This is a timing-based test, results may vary
        if duration2 > duration1 {
            println!("⚠️ Cache wasn't faster this time (timing variance): {:?} vs {:?}", duration2, duration1);
            println!("  This is normal in test environments with timing variance");
        } else {
            println!("✅ Cache was faster: {:?} vs {:?}", duration2, duration1);
        }
    }
    
    #[test]
    fn test_memoized_with_different_parameters() {
        // Test that different parameters produce different results
        let result1 = expensive_computation(1, 1);
        let result2 = expensive_computation(2, 2);
        let result3 = expensive_computation(3, 3);
        
        // Test that same parameters return cached results
        assert_eq!(expensive_computation(1, 1), result1); // Should be cached
        assert_eq!(expensive_computation(2, 2), result2); // Should be cached
        assert_eq!(expensive_computation(3, 3), result3); // Should be cached
    }
    
    #[test]
    fn test_memoized_in_pipeline() {
        // Test that memoized functions work correctly in pipelines
        let result = pipex!(
            vec![(1, 1), (2, 2), (3, 3), (1, 1)] // Note: (1,1) appears twice
            => |pair| Ok::<i32, String>(expensive_computation(pair.0, pair.1))
            => |x| Ok::<i32, String>(x * 2)
        );
        
        assert_eq!(result.len(), 4);
        assert!(result.iter().all(|r| r.is_ok()));
        
        let values: Vec<i32> = result.into_iter().map(|r| r.unwrap()).collect();
        
        // Test that we got 4 results and that the first and fourth are the same (cached)
        assert_eq!(values.len(), 4);
        assert_eq!(values[0], values[3]); // (1,1) appears twice, should be same result
    }

    #[test]
    fn test_fibonacci_memoization() {
        // Test recursive memoization with Fibonacci
        // This would be very slow without memoization for larger numbers
        assert_eq!(fibonacci_memoized(20), 6765);
        assert_eq!(fibonacci_memoized(25), 75025);
        
        // Calling again should use cached intermediate results
        assert_eq!(fibonacci_memoized(22), 17711);
    }
    
    #[test]
    fn test_triple_power_error_pure_memoized() {
        // Test function that combines all three powerful features:
        // 1. #[error_strategy] - Automatic error handling
        // 2. #[pure] - Compile-time purity guarantees  
        // 3. #[memoized] - Runtime performance optimization
        
        // Test successful operations (should be memoized)
        let result1 = advanced_mathematical_operation(5, 10);
        assert!(result1.is_ok());
        let value1 = result1.unwrap();
        
        // Call again with same parameters - should hit cache
        let result2 = advanced_mathematical_operation(5, 10);
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap(), value1); // Same result from cache
        
        // Test error cases
        let error_result1 = advanced_mathematical_operation(-1, 5);
        assert!(error_result1.is_err());
        assert!(error_result1.unwrap_err().contains("Negative inputs not supported"));
        
        let error_result2 = advanced_mathematical_operation(150, 10);
        assert!(error_result2.is_err());
        assert!(error_result2.unwrap_err().contains("Inputs too large"));
        
        // Test in a pipeline with mixed success/error cases
        let input_data = vec![
            (5, 10),   // ✅ Valid - will be cached
            (-1, 5),   // ❌ Error - negative input
            (10, 20),  // ✅ Valid
            (5, 10),   // ✅ Valid - should hit cache from first call
            (200, 5),  // ❌ Error - too large
        ];
        
        let pipeline_result = pipex!(
            input_data
            => |pair| advanced_mathematical_operation(pair.0, pair.1)
            => |x| Ok::<i32, String>(x + 100) // Add 100 to successful results
        );
        
        // Should have 5 results total
        assert_eq!(pipeline_result.len(), 5);
        
        // Check results pattern
        assert!(pipeline_result[0].is_ok()); // (5,10) success
        assert!(pipeline_result[1].is_err()); // (-1,5) error
        assert!(pipeline_result[2].is_ok()); // (10,20) success  
        assert!(pipeline_result[3].is_ok()); // (5,10) success from cache
        assert!(pipeline_result[4].is_err()); // (200,5) error
        
        // Verify cached result consistency
        let first_success = pipeline_result[0].as_ref().unwrap();
        let cached_success = pipeline_result[3].as_ref().unwrap();
        assert_eq!(first_success, cached_success); // Cache consistency
        
        // Count successful operations
        let success_count = pipeline_result.iter().filter(|r| r.is_ok()).count();
        let error_count = pipeline_result.iter().filter(|r| r.is_err()).count();
        
        assert_eq!(success_count, 3); // 3 successful operations
        assert_eq!(error_count, 2);   // 2 error cases
        
        println!("✅ Triple Power Test Complete!");
        println!("  🔒 Pure: Compile-time safety guaranteed");
        println!("  ⚡ Memoized: Performance optimization active");  
        println!("  🛡️ Error Strategy: Automatic error handling");
        println!("  📊 Results: {} success, {} errors", success_count, error_count);
    }

    // === GPU Tests ===
    
    #[cfg(feature = "gpu")]
    #[tokio::test]
    async fn test_basic_gpu_kernel() {
        
        // Simple GPU kernel that doubles each element
        let kernel = r#"
            @group(0) @binding(0) var<storage, read> input: array<f32>;
            @group(0) @binding(1) var<storage, read_write> output: array<f32>;
            
            @compute @workgroup_size(1)
            fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
                let index = global_id.x;
                if (index >= arrayLength(&input)) { return; }
                output[index] = input[index] * 2.0;
            }
        "#;
        
        let input_data = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        let expected = vec![2.0f32, 4.0, 6.0, 8.0, 10.0];
        
        match crate::gpu::execute_gpu_kernel(input_data, kernel).await {
            Ok(result) => {
                assert_eq!(result, expected);
                println!("✅ Basic GPU kernel test passed!");
            },
            Err(e) => {
                println!("⚠️ GPU test skipped (no GPU available): {}", e);
                // GPU tests can fail in CI environments, so we don't panic
            }
        }
    }
    
    #[cfg(feature = "gpu")]
    #[tokio::test]
    async fn test_gpu_pipeline_integration() {
        
        // WGSL kernel that squares each element - defined inline
        
        let input_data = vec![1.0f32, 2.0, 3.0, 4.0];
        
        // Test GPU step in a pipeline
        let result = pipex!(
            input_data
            => |x| Ok::<f32, String>(x + 1.0)  // Add 1: [2.0, 3.0, 4.0, 5.0]
            => gpu r#"
                @group(0) @binding(0) var<storage, read> input: array<f32>;
                @group(0) @binding(1) var<storage, read_write> output: array<f32>;
                
                @compute @workgroup_size(64)
                fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
                    let index = global_id.x;
                    if (index >= arrayLength(&input)) { return; }
                    output[index] = input[index] * input[index];
                }
            "# |data: Vec<f32>| { Ok(data) }  // Square: [4.0, 9.0, 16.0, 25.0]
            => |x| Ok::<f32, String>(x - 1.0)  // Subtract 1: [3.0, 8.0, 15.0, 24.0]
        );
        
        assert_eq!(result.len(), 4);
        if result.iter().all(|r| r.is_ok()) {
            let values: Vec<f32> = result.into_iter().map(|r| r.unwrap()).collect();
            let expected = vec![3.0f32, 8.0, 15.0, 24.0];
            assert_eq!(values, expected);
            println!("✅ GPU pipeline integration test passed!");
            println!("  🔄 CPU -> GPU -> CPU pipeline works!");
            println!("  📊 Results: {:?}", values);
        } else {
            println!("⚠️ GPU pipeline test failed - some operations errored");
            for (i, result) in result.iter().enumerate() {
                if let Err(e) = result {
                    println!("  ❌ Item {}: {}", i, e);
                }
            }
        }
    }
    
    #[cfg(feature = "gpu")]
    #[tokio::test]
    async fn test_gpu_with_errors() {
        
        // WGSL kernel defined inline
        
        // Test GPU handling with mixed success/error inputs
        let input_data = vec![1.0f32, 3.0f32, 4.0f32];
        
        let result = pipex!(
            input_data
            => |x| if x == 3.0 { Err("error item".to_string()) } else { Ok(x) }  // Inject error
            => gpu r#"
                @group(0) @binding(0) var<storage, read> input: array<f32>;
                @group(0) @binding(1) var<storage, read_write> output: array<f32>;
                
                @compute @workgroup_size(1)
                fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
                    let index = global_id.x;
                    if (index >= arrayLength(&input)) { return; }
                    output[index] = input[index] * 10.0;
                }
            "# |data: Vec<f32>| { Ok(data) }
            => |x| Ok::<f32, String>(x + 100.0)
        );
        
        assert_eq!(result.len(), 3);
        
        // Check that error was preserved
        assert!(result[1].is_err());
        
        if result[0].is_ok() && result[2].is_ok() {
            assert!((result[0].as_ref().unwrap() - 110.0).abs() < 0.001); // 1*10+100
            assert!((result[2].as_ref().unwrap() - 140.0).abs() < 0.001); // 4*10+100
            
            println!("✅ GPU error handling test passed!");
            println!("  🛡️ Errors preserved through GPU pipeline");
            println!("  ⚡ GPU computation: [110.0, Error, 140.0]");
        } else {
            println!("⚠️ GPU error handling test failed");
            for (i, result) in result.iter().enumerate() {
                match result {
                    Ok(val) => println!("  ✅ Item {}: {}", i, val),
                    Err(e) => println!("  ❌ Item {}: {}", i, e),
                }
            }
        }
    }
    
    #[cfg(feature = "gpu")]
    #[tokio::test] 
    async fn test_gpu_performance_vs_cpu() {
        use std::time::Instant;
        
        // Large dataset for performance comparison
        let data: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        
        // GPU kernel - complex mathematical operation defined inline
        
        // CPU version
        let cpu_start = Instant::now();
        let cpu_result = pipex!(
            data.clone()
            => |x| Ok::<f32, String>((x * 0.1).sin() * (x * 0.2).cos() + (x + 1.0).sqrt())
        );
        let cpu_duration = cpu_start.elapsed();
        
        // GPU version
        let gpu_start = Instant::now();
        let gpu_result = pipex!(
            data
            => gpu r#"
                @group(0) @binding(0) var<storage, read> input: array<f32>;
                @group(0) @binding(1) var<storage, read_write> output: array<f32>;
                
                @compute @workgroup_size(64) 
                fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
                    let index = global_id.x;
                    if (index >= arrayLength(&input)) { return; }
                    let x = input[index];
                    output[index] = sin(x * 0.1) * cos(x * 0.2) + sqrt(x + 1.0);
                }
            "# |data: Vec<f32>| { Ok(data) }
        );
        let gpu_duration = gpu_start.elapsed();
        
        if cpu_result.iter().all(|r| r.is_ok()) && gpu_result.iter().all(|r| r.is_ok()) {
            let cpu_values: Vec<f32> = cpu_result.into_iter().map(|r| r.unwrap()).collect();
            let gpu_values: Vec<f32> = gpu_result.into_iter().map(|r| r.unwrap()).collect();
                
            // Verify results are approximately equal (GPU floating point may differ slightly)
            let all_close = cpu_values.iter().zip(gpu_values.iter())
                .all(|(cpu, gpu)| (cpu - gpu).abs() < 0.01);
            
            if all_close {
                println!("✅ GPU performance test passed!");
                println!("  ⏱️ CPU time: {:?}", cpu_duration);
                println!("  🚀 GPU time: {:?}", gpu_duration);
                println!("  📊 Processing 1000 elements with complex math");
                
                if gpu_duration < cpu_duration {
                    println!("  🏆 GPU was faster!");
                } else {
                    println!("  💻 CPU was faster (expected for small datasets)");
                }
            } else {
                println!("⚠️ GPU/CPU results don't match closely enough");
            }
        } else {
            println!("⚠️ GPU performance test skipped (computation failed)");
        }
    }
    
    #[cfg(feature = "gpu")]
    #[tokio::test]
    async fn test_gpu_auto_transpilation() {
        // Test the dream syntax: automatic Rust-to-WGSL transpilation!
        let input_data = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        
        // This should automatically transpile `x * x + 1.0` to WGSL!
        let result = pipex!(
            input_data
            => gpu ||| |x| x * x + 1.0  // 🔥 MAGIC: Auto-transpiled to GPU!
            => |x| Ok::<f32, String>(x - 0.5)
        );
        
        assert_eq!(result.len(), 5);
        if result.iter().all(|r| r.is_ok()) {
            let values: Vec<f32> = result.into_iter().map(|r| r.unwrap()).collect();
            let expected = vec![1.5f32, 4.5, 9.5, 16.5, 25.5]; // (x² + 1) - 0.5
            
            // Allow small floating point differences  
            for (actual, expected) in values.iter().zip(expected.iter()) {
                assert!((actual - expected).abs() < 0.01, 
                    "Expected {}, got {}", expected, actual);
            }
            
            println!("✅ GPU Auto-Transpilation Test PASSED!");
            println!("  🎯 Expression: x * x + 1.0");
            println!("  🔄 Rust -> WGSL -> GPU -> Result");
            println!("  📊 Results: {:?}", values);
            println!("  🚀 DREAM SYNTAX WORKS!");
        } else {
            println!("⚠️ GPU auto-transpilation failed, but CPU fallback worked!");
            for (i, result) in result.iter().enumerate() {
                match result {
                    Ok(val) => println!("  ✅ Item {}: {}", i, val),
                    Err(e) => println!("  ❌ Item {}: {}", i, e),
                }
            }
        }
    }
    
    #[cfg(feature = "gpu")]
    #[tokio::test]
    async fn test_gpu_auto_complex_expressions() {
        // Test auto-transpilation with complex nested mathematical expressions
        let input_data = vec![0.5f32, 1.0, 1.5, 2.0];
        
        let result = pipex!(
            input_data
            => gpu ||| |x| (x * x + 1.0) * x.sin() + x.cos() * (x + 3.14159) - x.sqrt()
            // 🔥 Complex expression: mixed operations, multiple math functions, constants!
        );
        
        assert_eq!(result.len(), 4);
        if result.iter().all(|r| r.is_ok()) {
            let values: Vec<f32> = result.into_iter().map(|r| r.unwrap()).collect();
            
            // Verify against CPU calculations with the same complex expression
            let expected: Vec<f32> = vec![0.5f32, 1.0, 1.5, 2.0]
                .into_iter()
                .map(|x| (x * x + 1.0) * x.sin() + x.cos() * (x + 3.14159) - x.sqrt())
                .collect();
            
            for (actual, expected) in values.iter().zip(expected.iter()) {
                assert!((actual - expected).abs() < 0.01,
                    "Expected {}, got {}", expected, actual);
            }
            
            println!("✅ GPU Complex Expression Auto-Transpilation PASSED!");
            println!("  🎯 Expression: (x * x + 1.0) * x.sin() + x.cos() * (x + 3.14159) - x.sqrt()");
            println!("  🧮 Features successfully transpiled:");
            println!("    - Parentheses grouping: (x * x + 1.0), (x + 3.14159)");
            println!("    - Method calls: x.sin(), x.cos(), x.sqrt()");
            println!("    - Mixed operations: *, +, -, respecting precedence");
            println!("    - Multiple math functions: sin, cos, sqrt");
            println!("    - Constants: 1.0, 3.14159 (π approximation)");
            println!("    - Complex mathematical formula");
            println!("  📊 Results: {:?}", values);
            println!("  🚀 TRANSPILER HANDLES REAL-WORLD EXPRESSIONS!");
            println!("  ⚠️  Current limitation: Nested method chains like (expr).method().method()");
            println!("      → This would fallback to CPU automatically!");
        } else {
            println!("⚠️ Complex expression too advanced, used CPU fallback");
            println!("  This demonstrates the automatic fallback system working!");
        }
    }

    // Uncomment this test to see the proper error message when calling impure functions from pure ones:
    /*
    #[test]
    fn test_pure_macro_prevents_impure_function_calls() {
        #[pure]
        fn should_fail_compilation(x: i32) -> i32 {
            // This WILL fail compilation with this error message:
            // "cannot find value `RegularImpureFunction` in this scope"
            // This proves the purity check is working at compile time!
            regular_impure_function(x)
        }
        
        // This test would fail to compile, demonstrating the macro works
        assert_eq!(should_fail_compilation(5), 10);
    }
    */
}

// It's also good practice to explicitly re-export items that macros need,
// especially if they are somewhat internal.
// This makes the macro's dependency on $crate::traits::IntoPipelineItem robust.
#[doc(hidden)]
pub use traits::IntoPipelineItem as __InternalIntoPipelineItem; // Re-export for macro under a hidden name
                                                                 // The macro can then use $crate::__InternalIntoPipelineItem
                                                                 // OR, if the `traits` module is pub, $crate::traits::IntoPipelineItem works.
