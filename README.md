# Pipex 🚀

[![Crates.io](https://img.shields.io/crates/v/pipex.svg)](https://crates.io/crates/pipex)
[![Documentation](https://docs.rs/pipex/badge.svg)](https://docs.rs/pipex)
[![License](https://img.shields.io/crates/l/pipex.svg)](https://github.com/edransy/pipex)

A powerful functional pipeline macro for Rust that combines synchronous, asynchronous, and parallel operations with extensible error handling strategies.

## ✨ Features

- **🔄 Sync Operations**: Chain regular synchronous transformations
- **⚡ Async Operations**: Handle asynchronous work
- **🚀 Parallel Processing**: Leverage multiple CPU cores with Rayon
- **🛡️ Error Handling**: Extensible error handling strategies via proc macros
- **🔀 Mixed Workloads**: Seamlessly combine different operation types
- **📦 Modular**: Optional features for async and parallel processing

## 🚀 Quick Start

Add this to your `Cargo.toml`:

```toml
[features]
default = ["async", "parallel"]
async = []
parallel = []

[dependencies]
pipex = { version = "0.1.13", features = ["full"] }
tokio = { version = "1", features = ["full", "macros", "rt-multi-thread"] }
```


### Error Handling Strategies

```rust
use pipex::*;

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
        => ||| |n| { heavy_computation(n) } // Parallel processing
        => |result| Ok::<_, String>(format!("Computed: {}", result))
    );
    
    println!("Results: {:?}", result);
}
```

## 📖 Pipeline Syntax

| Syntax | Description | Example | Requires Feature |
|--------|-------------|---------|------------------|
| `\|x\| expr` | Synchronous transformation | `\|x\| Ok::<_, String>(x * 2)` | None |
| `async \|x\| { ... }` | Asynchronous operation | `async \|x\| { fetch(x).await }` | `async` |
| `\|\|\| \|x\| { ... }` | Parallel processing | `\|\|\| \|x\| { cpu_work(x) }` | `parallel` |

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
use pipex::*;

// Example: A handler that collects only successful results and reverses their order.
pub struct ReverseSuccessHandler;

impl<T, E> ErrorHandler<T, E> for ReverseSuccessHandler {
    fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
        let mut successes: Vec<Result<T, E>> = results
            .into_iter()
            .filter(|r| r.is_ok())
            .collect();
        successes.reverse();
        successes
    }
}

#[error_strategy(ReverseSuccessHandler)]
async fn process_items_with_reverse(x: i32) -> Result<i32, String> {
    if x == 3 { // Example condition for failure
        Err("processing failed for item 3".to_string())
    } else {
        Ok(x * 2) // Example processing for success
    }
}
```

### Registering Custom Strategies

For `pipex` to recognize and use your custom error handlers with specific data types (e.g. `Result<i32, String>`), you may need to register them. This is typically done once, for instance, at the beginning of your `main` function, using the `register_strategies!` macro.

This macro ensures that the procedural macros used by `pipex` can correctly associate your handlers with the functions they annotate.

```rust
use pipex::*;

// Assuming LastErrorHandler and ReverseSuccessHandler are defined structs
// that implement the ErrorHandler<T, E> trait. For example:
pub struct LastErrorHandler;
impl<T, E> ErrorHandler<T, E> for LastErrorHandler { /* ... */ }

pub struct ReverseSuccessHandler; // As defined in the example above
impl<T, E> ErrorHandler<T, E> for ReverseSuccessHandler { /* ... */ }


#[tokio::main]
fn main() {
    // Register your custom handlers for specific type signatures
    register_strategies!(
        LastErrorHandler, ReverseSuccessHandler for <i32, String>
    );

    // Now, functions like:
    #[error_strategy(LastErrorHandler)]
    async fn my_func_last_error(x: i32) -> Result<i32, String> { ... }

    #[error_strategy(ReverseSuccessHandler)]
    async fn my_func_reverse(x: i32) -> Result<i32, String> { ... }

    // ...can be used in pipex! pipelines that operate on Result<i32, String>.
}
```

## 📚 Complete Examples

### Data Processing Pipeline

```rust
use pipex::*;

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
        => ||| |data| { process_data(data) }         // Parallel processing
        => |count| Ok::<_, String>(format!("Processed {} chars", count)) // Final transformation
    );
    
    println!("Processed {} users successfully", result.len());
}
```




## 📋 Requirements

- Rust 1.75.0 or later
- For async features: tokio runtime
- For parallel features: compatible with rayon

## 🔧 Contributing

Contributions are welcome! Please see our [contributing guidelines](CONTRIBUTING.md).

## 📄 License

This project is licensed under the MIT License - see the [LICENSE-MIT](LICENSE-MIT) file for details. 