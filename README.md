# Pipex 🚀

[![Crates.io](https://img.shields.io/crates/v/pipex.svg)](https://crates.io/crates/pipex)
[![Documentation](https://docs.rs/pipex/badge.svg)](https://docs.rs/pipex)
[![License](https://img.shields.io/crates/l/pipex.svg)](https://github.com/edransy/pipex)

A powerful functional pipeline macro for Rust that combines synchronous, asynchronous, and parallel operations with extensible error handling strategies.

## ✨ Features

- **🔄 Sync Operations**: Chain regular synchronous transformations
- **⚡ Async Operations**: Handle asynchronous work with automatic await
- **🚀 Parallel Processing**: Leverage multiple CPU cores with Rayon (optional)
- **🛡️ Error Handling**: Extensible error handling strategies via proc macros
- **🔀 Mixed Workloads**: Seamlessly combine different operation types
- **📦 Modular**: Optional features for async and parallel processing

## 🚀 Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
pipex = "0.2.0"

# For async features
tokio = { version = "1", features = ["full"] }
```

### Basic Synchronous Example

```rust
use pipex::pipex;

fn main() {
    let result = pipex!(
        vec![1, 2, 3, 4, 5]
        => |x| x * 2
        => |x| x + 1
    );
    
    // Extract successful values
    let values: Vec<i32> = result.into_iter()
        .filter_map(|r| r.ok())
        .collect();
    println!("{:?}", values); // [3, 5, 7, 9, 11]
}
```

### Error Handling Strategies

```rust
use pipex::{pipex, error_strategy, IgnoreHandler, CollectHandler};

#[error_strategy(IgnoreHandler)]
async fn process_even(x: i32) -> Result<i32, String> {
    if x % 2 == 0 {
        Ok(x * 2)
    } else {
        Err("Odd number".to_string())
    }
}

#[error_strategy(CollectHandler)]
async fn always_succeed(x: i32) -> Result<i32, String> {
    Ok(x + 1)
}

#[tokio::main]
async fn main() {
    // This will ignore errors from odd numbers
    let result1 = pipex!(
        vec![1, 2, 3, 4, 5]
        => async |x| { process_even(x).await }
    );
    // Only even numbers are processed: [4, 8]
    
    // This will collect all results including errors
    let result2 = pipex!(
        vec![1, 2, 3, 4, 5]
        => async |x| { always_succeed(x).await }
    );
    // All numbers processed: [Ok(2), Ok(3), Ok(4), Ok(5), Ok(6)]
}
```

### Parallel Processing

```rust
use pipex::pipex;

fn heavy_computation(n: i32) -> i32 {
    // Simulate CPU-intensive work
    (1..=n).sum::<i32>() % 1000
}

#[tokio::main]
async fn main() {
    let result = pipex!(
        vec![100, 200, 300, 400, 500]
        => ||| |n| heavy_computation(n)  // Parallel processing
        => |result| format!("Computed: {}", result)
    );
    
    println!("Results: {:?}", result);
}
```

## 📖 Pipeline Syntax

| Syntax | Description | Example | Requires Feature |
|--------|-------------|---------|------------------|
| `\|x\| expr` | Synchronous transformation | `\|x\| x * 2` | None |
| `async \|x\| { ... }` | Asynchronous operation | `async \|x\| { fetch(x).await }` | `async` |
| `\|\|\| \|x\| expr` | Parallel processing | `\|\|\| \|x\| cpu_work(x)` | `parallel` |

## 🛡️ Error Handling Strategies

Pipex provides several built-in error handling strategies:

| Strategy | Description | Behavior |
|----------|-------------|----------|
| `IgnoreHandler` | Ignore errors | Only successful results are kept |
| `CollectHandler` | Collect all | Both success and error results are kept |
| `FailFastHandler` | Fail fast | Only error results are kept |
| `LogAndIgnoreHandler` | Log and ignore | Errors are logged to stderr, then ignored |

### Custom Error Handlers

You can implement your own error handling strategies:

```rust
use pipex::{ErrorHandler, error_strategy};

struct RetryHandler;

impl<T, E> ErrorHandler<T, E> for RetryHandler 
where 
    E: std::fmt::Debug 
{
    fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
        // Custom retry logic here
        results
    }
}

#[error_strategy(RetryHandler)]
async fn risky_operation(x: i32) -> Result<i32, String> {
    // Your async operation
    Ok(x)
}
```

## 📚 Complete Examples

### Data Processing Pipeline

```rust
use pipex::{pipex, error_strategy, LogAndIgnoreHandler};
use tokio;

#[error_strategy(LogAndIgnoreHandler)]
async fn fetch_user_data(id: i32) -> Result<String, String> {
    // Simulate network request that might fail
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    if id % 10 == 0 {
        Err(format!("Failed to fetch user {}", id))
    } else {
        Ok(format!("User {} data", id))
    }
}

fn process_data(data: String) -> usize {
    // Simulate CPU-intensive processing
    data.chars().filter(|c| c.is_alphanumeric()).count()
}

#[tokio::main]
async fn main() {
    let user_ids = (1..=20).collect::<Vec<_>>();
    
    let result = pipex!(
        user_ids
        => async |id| { fetch_user_data(id).await }  // Async fetch with error handling
        => ||| |data| process_data(data)             // Parallel processing
        => |count| format!("Processed {} chars", count)  // Final transformation
    );
    
    println!("Processed {} users successfully", result.len());
}
```

### Mixed Synchronous and Asynchronous Pipeline

```rust
use pipex::{pipex, error_strategy, CollectHandler};
use tokio;

#[error_strategy(CollectHandler)]
async fn async_double(x: i32) -> Result<i32, String> {
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    Ok(x * 2)
}

#[tokio::main]
async fn main() {
    let numbers = vec![1, 2, 3, 4, 5];
    
    let result = pipex!(
        numbers
        => |x| x + 1                           // Sync: add 1
        => async |x| { async_double(x).await } // Async: double
        => ||| |x| x * x                       // Parallel: square
        => |x| x - 1                           // Sync: subtract 1
    );
    
    println!("Final results: {:?}", result);
}
```

## 🎯 Features Configuration

Pipex uses feature flags to keep dependencies minimal:

```toml
[dependencies]
# Minimal installation (sync operations only)
pipex = { version = "0.2.0", default-features = false }

# With async support
pipex = { version = "0.2.0", features = ["async"] }

# With parallel support  
pipex = { version = "0.2.0", features = ["parallel"] }

# Full installation (recommended)
pipex = "0.2.0"  # Includes async and parallel by default
```

## 📋 Requirements

- Rust 1.75.0 or later
- For async features: tokio runtime
- For parallel features: compatible with rayon

## 🔧 Contributing

Contributions are welcome! Please see our [contributing guidelines](CONTRIBUTING.md).

## 📄 License

This project is licensed under the MIT License - see the [LICENSE-MIT](LICENSE-MIT) file for details. 