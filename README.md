# Animation Player

This project provides a Rust implementation of an animation player designed to run
both embedded and in a web browser using WebAssembly.

## Directory Structure

- `/animation-player` - Rust crate containing the WebAssembly source code
  - `/src` - Rust source files
  - `/pkg` - Generated WebAssembly package (after building)
  - `/tests` - Tests, including WebAssembly-specific tests
- `/react-demo` - A comprehensive React application demonstrating integration with the WebAssembly package, showcasing advanced features like animation baking, file upload, and a dark/light theme toggle.

## Getting Started

- Install [Rust](https://www.rust-lang.org/tools/install)

Like any other Rust crate, you can build and test it with:

```bash
cd animation-player
cargo test
```

## WebAssembly integration

### Prerequisites

- Install [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)

```bash
# Install wasm-pack with the installer script
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
```

### Building the WebAssembly Package

Navigate to the animation-player directory and build the package:

```bash
# Build the Rust code into a WebAssembly package
cd animation-player
wasm-pack build --target web
```

This compiles the Rust code into a WebAssembly module and generates JavaScript bindings. The output is placed in the `pkg` directory, which contains:

- `animation_player_bg.wasm` - The WebAssembly binary
- `animation_player_bg.wasm.d.ts` - TypeScript declarations for the WASM module
- `animation_player.js` - JavaScript bindings to interact with the WASM module
- `animation_player.d.ts` - TypeScript declarations for the JavaScript bindings
- `package.json` - npm package configuration

### Testing for WebAssembly

To run tests for the WebAssembly build:

```bash
# Run WebAssembly tests in headless browsers
cd animation-player
wasm-pack test --node
```

Consider testing it for browsers as well:

```bash
cd animation-player
wasm-pack test --headless --firefox --chrome
```

## Using in a Web Application

To use this package in your own npm project,
see the example in [`react-demo`](react-demo/README.md).

In short, you can install the package built under `animation-player/pkg`,
either as a local dependency or by publishing it to npm.

```bash
npm install /path/to/animation-player/pkg
```

Then, you can import the package in your JavaScript/TypeScript code:

```typescript
import init, { greet } from 'animation-player';
```

It is important to call that `init()` function before using any of the exported functions.
