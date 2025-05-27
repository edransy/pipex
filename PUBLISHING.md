# Publishing Guide for Pipex

This crate consists of two components:
- `pipex-macros`: The procedural macro crate
- `pipex`: The main library crate

## Publishing Process

Due to the dependency relationship, these must be published in order:

### Option 1: Use the provided script

```bash
./publish.sh
```

This will automatically publish both crates in the correct order.

### Option 2: Manual publishing

1. **First, publish pipex-macros:**
   ```bash
   cd pipex-macros
   cargo publish
   cd ..
   ```

2. **Wait for it to be available on crates.io (usually 1-2 minutes)**

3. **Then publish the main crate:**
   ```bash
   cargo publish
   ```

## Dry Run Testing

To test the publishing process without actually publishing:

```bash
# Test pipex-macros
cd pipex-macros && cargo publish --dry-run
cd ..

# Test pipex (will fail until pipex-macros is on crates.io)
cargo publish --dry-run
```

## Version Updates

When updating versions:

1. Update the version in `Cargo.toml`
2. Update the version in `pipex-macros/Cargo.toml`
3. Update the `pipex-macros` dependency version in the main `Cargo.toml`
4. Ensure all versions match

## Package Contents

The main `pipex` package includes:
- All source code from `src/`
- The entire `pipex-macros/` subdirectory
- README.md and LICENSE-MIT

This means users only need to add `pipex = "0.2.0"` to their dependencies - the proc macros are included automatically. 