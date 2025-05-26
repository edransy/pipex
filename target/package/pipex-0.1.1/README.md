# Pipex 🚀

[![Crates.io](https://img.shields.io/crates/v/pipex.svg)](https://crates.io/crates/pipex)
[![Documentation](https://docs.rs/pipex/badge.svg)](https://docs.rs/pipex)
[![License](https://img.shields.io/crates/l/pipex.svg)](https://github.com/yourusername/pipex)

A powerful functional pipeline macro for Rust that combines synchronous, asynchronous, parallel, and streaming operations in a single, intuitive syntax.

## ✨ Features

- **🔄 Sync Operations**: Chain regular synchronous transformations
- **⚡ Async Operations**: Handle asynchronous work with automatic await
- **🚀 Parallel Processing**: Leverage multiple CPU cores with configurable thread pools  
- **🌊 Streaming**: Process large datasets with configurable buffer sizes
- **🛡️ Error Handling**: Built-in Result handling with `async?` syntax
- **🔀 Mixed Workloads**: Seamlessly combine different operation types
- **📈 Performance**: Optimized for both throughput and resource efficiency

## 🚀 Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
pipex = "0.1.0"
tokio = { version = "1", features = ["full"] }  # If using async features
```

### Basic Example

```rust
use pipex::pipex;

fn main() {
    let result = pipex!(
        vec![1, 2, 3, 4, 5]
        => |x| x * 2
        => |x| x + 1
    );
    println!("{:?}", result); // [3, 5, 7, 9, 11]
}
```

### Async Example

```rust
use pipex::pipex;
use tokio;

#[tokio::main]
async fn main() {
    let result = pipex!(
        vec!["https://api1.com", "https://api2.com", "https://api3.com"]
        => async |url| {
            // Simulate HTTP request
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            format!("Data from {}", url)
        }
        => |response| response.len()
    );
    println!("Responses: {:?}", result);
}
```

## 📖 Pipeline Syntax

| Syntax | Description | Example |
|--------|-------------|---------|
| `\|x\| expr` | Synchronous transformation | `\|x\| x * 2` |
| `async \|x\| { ... }` | Asynchronous operation | `async \|url\| { fetch(url).await }` |
| `\|\|\| threads \|x\| expr` | Parallel processing | `\|\|\| 4 \|x\| cpu_work(x)` |
| `~async buffer \|x\| { ... }` | Streaming with buffer | `~async 10 \|x\| { process(x).await }` |
| `async? \|x\| { ... }` | Async with error filtering | `async? \|x\| { try_work(x).await }` |
| `collect` | Explicit collection | Force evaluation at this point |

## 📚 Examples

### 1. CPU-Intensive Work with Parallel Processing

```rust
use pipex::pipex;

fn heavy_computation(n: i32) -> i32 {
    (1..=n).sum::<i32>() % 1000
}

fn main() {
    let result = pipex!(
        vec![100, 200, 300, 400, 500]
        => ||| 4 |n| heavy_computation(n)  // Use 4 threads
        => |result| format!("Computed: {}", result)
    );
    println!("{:?}", result);
}
```

### 2. I/O-Intensive Work with Streaming

```rust
use pipex::pipex;
use tokio;

async fn fetch_data(id: i32) -> String {
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    format!("Data {}", id)
}

#[tokio::main]
async fn main() {
    let result = pipex!(
        (1..=20).collect::<Vec<_>>()
        => ~async 5 |id| {  // Process max 5 items concurrently
            fetch_data(id).await
        }
        => |data| data.len()
    );
    println!("Processed {} items", result.len());
}
```

### 3. Error Handling

```rust
use pipex::pipex;
use tokio;

async fn risky_operation(n: i32) -> Result<i32, &'static str> {
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    if n % 3 == 0 {
        Err("Divisible by 3")
    } else {
        Ok(n * 2)
    }
}

#[tokio::main]
async fn main() {
    let result = pipex!(
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9]
        => async? |n| { risky_operation(n).await }  // Filters out errors automatically
        => |success| success + 10
    );
    println!("Successful results: {:?}", result); // Only non-error values
}
```

### 4. Mixed Pipeline (Real-world scenario)

```rust
use pipex::pipex;
use tokio;

async fn fetch_user_data(id: i32) -> String {
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    format!("User {} data", id)
}

fn process_data(data: String) -> usize {
    // Simulate CPU-intensive processing
    data.chars().filter(|c| c.is_alphanumeric()).count()
}

#[tokio::main]
async fn main() {
    let user_ids = vec![1, 2, 3, 4, 5, 6, 7, 8];
    
    let result = pipex!(
        user_ids
        => |id| id * 100                           // Generate user codes
        => ~async 3 |code| {                       // Fetch max 3 users concurrently
            fetch_user_data(code).await
        }
        => ||| 4 |data| process_data(data)         // Process in parallel (4 threads)
        => |count| if count > 10 { count * 2 } else { count }  // Business logic
        => async |processed| {                     // Final async step
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            format!("Final: {}", processed)
        }
    );
    
    println!("Processed {} users: {:?}", result.len(), &result[0..3]);
}
```

### 5. Data Science Pipeline

```rust
use pipex::pipex;
use tokio;

#[tokio::main]
async fn main() {
    let raw_data = vec![1.5, 2.7, 3.1, 4.8, 5.2, 6.9, 7.3, 8.1];
    
    let processed = pipex!(
        raw_data
        => |x| x * 10.0                          // Scale up
        => ||| |x| x.round() as i32              // Parallel rounding
        => |x| if x % 2 == 0 { x } else { x + 1 }  // Make even
        => async |x| {                           // Async validation
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            if x > 50 { x } else { x * 2 }
        }
        => |x| format!("Value: {}", x)
    );
    
    println!("Processed data: {:?}", processed);
}
```

## 🎯 Performance Guidelines

### CPU-Intensive Work
```rust
// Use parallel processing with thread count ≤ CPU cores
pipex!(
    data => ||| 4 |item| cpu_heavy_work(item)
)
```

### I/O-Intensive Work  
```rust
// Use streaming with moderate buffer sizes
pipex!(
    data => ~async 10 |item| { io_work(item).await }
)
```

### Mixed Workloads
```rust
// Balance parallelism and concurrency
pipex!(
    data 
    => ||| 4 |x| cpu_work(x)           // CPU-bound: limited threads
    => ~async 20 |x| { io_work(x).await }  // I/O-bound: higher concurrency
)
```

## 🔧 Advanced Features

### Explicit Collection
Sometimes you need to force evaluation at a specific point:

```rust
let result = pipex!(
    large_dataset
    => |x| x * 2
    => collect              // Force collection here
    => |data| aggregate(data)  // Work with the collected Vec
);
```

### Configurable Threading
```rust
// Use specific thread counts for different workloads
let result = pipex!(
    data
    => ||| 2 |x| light_cpu_work(x)     // 2 threads for light work
    => ||| 8 |x| heavy_cpu_work(x)     // 8 threads for heavy work
);
```

### Custom Buffer Sizes
```rust
// Tune concurrency for different I/O patterns
let result = pipex!(
    urls
    => ~async 5 |url| { slow_api_call(url).await }    // Respect rate limits
    => ~async 50 |data| { fast_processing(data).await } // High concurrency
);
```

## 📊 When to Use Each Operation Type

| Operation | Best For | Thread/Buffer Count |
|-----------|----------|-------------------|
| `\|x\| expr` | Light transformations, filtering | N/A |
| `async \|x\| { ... }` | I/O operations, small datasets | Auto-managed |
| `\|\|\| n \|x\| expr` | CPU-intensive work | 1-CPU core count |
| `~async n \|x\| { ... }` | I/O-heavy, large datasets | 10-100 depending on I/O |
| `async? \|x\| { ... }` | Unreliable operations | Auto-managed |

## 🚀 Performance Tips

1. **CPU Work**: Use `|||` with thread count ≤ CPU cores
2. **I/O Work**: Use `~async` with buffer size 10-50
3. **Error-Prone**: Use `async?` to auto-filter failures
4. **Memory**: Use `collect` sparingly for large datasets
5. **Mixed**: Start conservative, then tune based on bottlenecks

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📄 License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option. 