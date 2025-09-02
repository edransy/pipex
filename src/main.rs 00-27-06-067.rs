use pipex::{
    pipex, error_strategy, ErrorHandler, PipexResult,
    IgnoreHandler, CollectHandler, LogAndIgnoreHandler
};
use std::fmt;

// Custom error handler that retries failed operations once
struct RetryOnceHandler;

impl<T, E> ErrorHandler<T, E> for RetryOnceHandler 
where 
    E: fmt::Debug 
{
    fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
        println!("🔄 RetryOnceHandler: Processing {} results", results.len());
        
        let mut final_results = Vec::new();
        let mut retry_count = 0;
        
        for result in results {
            match result {
                Ok(value) => {
                    println!("✅ Success: keeping successful result");
                    final_results.push(Ok(value));
                }
                Err(err) => {
                    retry_count += 1;
                    println!("❌ Error encountered: {:?} (retry #{} - simulating retry)", err, retry_count);
                    // In a real implementation, you might actually retry the operation
                    // For demo purposes, we'll just log and keep the error
                    final_results.push(Err(err));
                }
            }
        }
        
        println!("🏁 RetryOnceHandler: Processed {} errors, returning {} results", 
                retry_count, final_results.len());
        final_results
    }
}

// Custom error handler that collects only the first N successful results
struct TakeFirstNHandler;

impl<T, E> ErrorHandler<T, E> for TakeFirstNHandler {
    fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
        println!("🎯 TakeFirstNHandler: Taking first 3 successful results from {} total", results.len());
        
        results.into_iter()
            .filter(|r| r.is_ok())
            .take(3)
            .collect()
    }
}

// Test functions with different error strategies
#[error_strategy(RetryOnceHandler)]
async fn risky_operation(x: i32) -> Result<i32, String> {
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    
    if x % 3 == 0 {
        Err(format!("Failed on number {}", x))
    } else {
        Ok(x * 2)
    }
}

#[error_strategy(TakeFirstNHandler)]
async fn selective_operation(x: i32) -> Result<String, String> {
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    
    if x > 10 {
        Err(format!("Number {} is too large", x))
    } else {
        Ok(format!("Processed: {}", x))
    }
}

#[error_strategy(IgnoreHandler)]
async fn filter_evens(x: i32) -> Result<i32, String> {
    if x % 2 == 0 {
        Ok(x)
    } else {
        Err(format!("Odd number: {}", x))
    }
}

#[error_strategy(LogAndIgnoreHandler)]
async fn noisy_operation(x: i32) -> Result<i32, String> {
    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    
    if x == 7 {
        Err("Lucky number 7 causes an error!".to_string())
    } else {
        Ok(x * x)
    }
}

fn cpu_intensive_work(x: i32) -> i32 {
    // Simulate some CPU work
    (1..=x).sum::<i32>() % 100
}

#[tokio::main]
async fn main() {
    println!("🚀 Testing Pipex 0.2.0 with Custom Error Handlers!\n");

    // Test 1: Custom RetryOnceHandler
    println!("=== Test 1: Custom RetryOnceHandler ===");
    let result1 = pipex!(
        vec![1, 2, 3, 4, 5, 6]
        => async |x| { risky_operation(x).await }
        => |x| x + 10
    );
    println!("Result 1: {:?}\n", result1);

    // Test 2: Custom TakeFirstNHandler
    println!("=== Test 2: Custom TakeFirstNHandler ===");
    let result2 = pipex!(
        vec![1, 2, 3, 4, 5, 15, 20, 25]
        => async |x| { selective_operation(x).await }
    );
    println!("Result 2: {:?}\n", result2);

    // Test 3: Built-in IgnoreHandler
    println!("=== Test 3: Built-in IgnoreHandler (filter evens) ===");
    let result3 = pipex!(
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        => async |x| { filter_evens(x).await }
        => |x| x * 3
    );
    println!("Result 3: {:?}\n", result3);

    // Test 4: Built-in LogAndIgnoreHandler
    println!("=== Test 4: Built-in LogAndIgnoreHandler ===");
    let result4 = pipex!(
        vec![1, 2, 3, 4, 5, 6, 7, 8]
        => async |x| { noisy_operation(x).await }
        => |x| x + 1
    );
    println!("Result 4: {:?}\n", result4);

    // Test 5: Mixed pipeline with parallel processing
    println!("=== Test 5: Mixed Pipeline (Async + Parallel + Sync) ===");
    let result5 = pipex!(
        vec![1, 2, 3, 4, 5, 6, 7, 8]
        => async |x| { filter_evens(x).await }     // Async: filter evens
        => ||| |x| cpu_intensive_work(x)           // Parallel: CPU work
        => |x| format!("Final: {}", x)             // Sync: format
    );
    println!("Result 5: {:?}\n", result5);

    // Test 6: Pure synchronous pipeline
    println!("=== Test 6: Pure Synchronous Pipeline ===");
    let result6 = pipex!(
        vec![1, 2, 3, 4, 5]
        => |x| x * 2
        => |x| x + 1
        => |x| x * x
    );
    println!("Result 6: {:?}\n", result6);

    // Test 7: Extract successful values
    println!("=== Test 7: Extract Successful Values ===");
    use pipex::ExtractSuccessful;
    
    let mixed_results = vec![Ok(1), Err("error"), Ok(3), Err("another error"), Ok(5)];
    let successful: Vec<i32> = mixed_results.extract_successful();
    println!("Extracted successful values: {:?}\n", successful);

    println!("🎉 All tests completed! Pipex 0.2.0 is working great!");
} 