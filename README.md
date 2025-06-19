# Solana Pipex Example

A minimal Solana program demonstrating the revolutionary `pipex` pipeline processing library with compile-time pure function verification.

## Features Demonstrated

🚀 **Pipeline Processing**: Clean separation of impure I/O and pure business logic  
🔒 **Compile-time Security**: Pure functions verified at compile time with zero runtime overhead  
⚡ **Perfect for Solana**: Optimized for Solana's account model and compute budget  
🎯 **Real-world Pattern**: Demonstrates actual DeFi swap logic with AMM calculations  

## What This Example Shows

This program implements a simple DEX (Decentralized Exchange) that demonstrates:

1. **Pipeline Pattern**: `load → validate → calculate → save`
2. **Pure Functions**: Business logic isolated and compile-time verified
3. **Error Handling**: Graceful error propagation through the pipeline
4. **Solana Integration**: Real Anchor program with proper account management

```rust
let result: [Result<SwapContext, SwapError>; 1] = pipex!(
    pipeline_input
    => |ctx| load_swap_state(ctx)        // IMPURE: Load current state
    => |ctx| validate_swap_pure(ctx)     // PURE: Validate transaction
    => |ctx| calculate_swap_pure(ctx)    // PURE: Calculate new state
    => |ctx| save_swap_state(ctx)        // IMPURE: Save updates
);
```

## Prerequisites

1. Install Rust: https://rustup.rs/
2. Install Solana CLI: https://docs.solana.com/cli/install-solana-cli-tools
3. Install Anchor: https://www.anchor-lang.com/docs/installation
4. Install Node.js and Yarn

## Build Instructions

### 1. Clone and Setup

```bash
# Make sure you're in the pipex repository root
# This example uses the pipex library as a local dependency

# Create the example directory structure
mkdir solana-pipex-example
cd solana-pipex-example

# Copy the files from this example into the directory
# (You'll have created these files following the instructions above)
```

### 2. Install Dependencies

```bash
# Install Node.js dependencies
yarn install

# Build the pipex library first (from the pipex repo root)
cd ../
cargo build
cd solana-pipex-example/
```

### 3. Build the Program

```bash
# Build the Solana program
anchor build

# This will:
# - Compile the Rust program with pipex integration
# - Generate TypeScript types
# - Create the program binary
```

### 4. Run Tests

```bash
# Start a local Solana validator (in a separate terminal)
solana-test-validator

# Run the tests
anchor test --skip-local-validator

# Expected output:
# ✓ Initializes a liquidity pool
# ✓ Executes a successful token swap using pipex pipeline
# ✓ Fails swap due to insufficient balance
# ✓ Demonstrates the power of pure functions and pipeline
```

## Test Results Explanation

### Test 1: Pool Initialization
Creates a liquidity pool with 1M tokens of each type, demonstrating basic account creation.

### Test 2: Successful Swap
Shows the complete pipeline in action:
- Validates user has sufficient balance
- Calculates AMM swap amounts with fees
- Updates both pool and user state atomically

### Test 3: Error Handling
Demonstrates how pure validation functions catch errors before any state changes.

### Test 4: Pipeline Demo
Educational test showing the architectural benefits.

## Revolutionary Aspects

This example showcases why `pipex` is revolutionary for Solana development:

### 1. Compile-Time Security
```rust
#[pure]
fn validate_swap_pure(ctx: SwapContext) -> Result<SwapContext, SwapError> {
    // ✅ Cannot access external state
    // ✅ Cannot call impure functions  
    // ✅ Cannot use unsafe code
    // ✅ All verified at compile time!
}
```

### 2. Zero Runtime Overhead
- No runtime checks
- No allocations in pure functions
- Perfect for Solana's compute budget

### 3. Clear Architecture
- Impure functions clearly marked
- Business logic isolated in pure functions
- State changes explicit and controlled

### 4. Perfect Solana Fit
The pipeline pattern matches Solana's account model exactly:
1. Load accounts (impure)
2. Validate business rules (pure)
3. Calculate new state (pure)
4. Save updates (impure)

## Real-World Applications

This pattern is perfect for:
- **DeFi Protocols**: AMMs, lending, derivatives
- **Gaming**: State transitions, rewards, battles
- **NFT Marketplaces**: Pricing, royalties, transfers
- **DAOs**: Voting, proposals, treasury management

## File Structure

```
solana-pipex-example/
├── Anchor.toml              # Anchor configuration
├── Cargo.toml               # Workspace configuration
├── package.json             # Node.js dependencies
├── tsconfig.json            # TypeScript configuration
├── programs/
│   └── solana-pipex-example/
│       ├── Cargo.toml       # Program dependencies
│       └── src/
│           └── lib.rs       # Main program with pipex integration
└── tests/
    └── solana-pipex-example.ts  # TypeScript tests
```

## Next Steps

1. **Extend the Example**: Add more complex DeFi features
2. **Performance Testing**: Measure compute unit usage
3. **Security Audit**: Review the pure function guarantees
4. **Integration**: Use in your own Solana programs

## Learn More

- [Pipex Documentation](../README.md)
- [Anchor Framework](https://www.anchor-lang.com/)
- [Solana Program Library](https://spl.solana.com/)

---

*This example demonstrates the future of secure, efficient Solana program development with `pipex`!* 🚀 