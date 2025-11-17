# MotionCore

Arora's MotionCore engine

## Building and Running

```bash
# Build the project
cargo build

# Run with a specific configuration file
cargo run --bin motioncore_standalone test-data/testprogram1.aprog.json

# Or just use the default run
cargo run -- test-data/testprogram1.aprog.json

# Provide the address for the daemon to listen on (default is 127.0.0.1:8080)
cargo run -- test-data/testprogram1.aprog.json address:port
```

## Benchmarks

The project includes a comprehensive benchmarking suite to measure performance across different components.

### Running Benchmarks with Criterion

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench blackboard_benchmarks

# Or use the provided script
./run_benchmarks.ps1 blackboard_benchmarks
```

### Standalone Benchmark Binary

For quick performance testing, use the standalone benchmark binary:

```bash
# Build and run the benchmark binary
./run_standalone_benchmark.ps1 -BenchmarkType blackboard -Size 500 -Iterations 1000 -Release

# Or run directly
cargo run --bin dev_bb_benchmark -- blackboard --size=500 --iterations=1000
```

See the [benchmarks README](./benches/README.md) for more details on the benchmarking suite.
