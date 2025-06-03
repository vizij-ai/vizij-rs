# Animation Player Core - WebAssembly Demo (Vite)

A high-performance animation engine built in Rust with WebAssembly bindings for web integration. This project provides real-time animation streaming, interpolation, and playback capabilities optimized for modern web browsers.

## Overview

The Animation Player Core is a Rust-based animation engine that compiles to WebAssembly for web deployment. It features:

- **Comprehensive Value Types**: Supports Float, Integer, Boolean, String, Vector2, Vector3, Vector4, Color, and Transform.
- **Real-time Interpolation**: Smooth animation transitions with extensible interpolation functions.
- **Animation Baking**: Pre-calculates animation values at specified frame rates for optimized playback.
- **Event System**: Dispatches detailed events for playback state changes, data modifications, and performance warnings.
- **Advanced Player Management**: Multiple animation players with independent playback states, speed control, and playback modes (once, loop, ping-pong).
- **Performance Monitoring & Configuration**: Configurable engine settings, performance thresholds, and real-time metrics for optimal performance.
- **Derivative Calculation**: Supports numerical derivative calculation for animation values, useful for motion analysis.
- **WebAssembly Integration**: Robust WASM bindings for seamless integration into web applications, including all core functionalities.

## Prerequisites

Before building and running the WASM demo, ensure you have the following installed:

### Required Tools

1. **Rust** (latest stable version)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **wasm-pack** (for building WASM modules)
   ```bash
   curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
   ```

3. **Node.js** (for the local HTTP server)
   ```bash
   # Using Node Version Manager (recommended)
   curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
   nvm install node
   ```

4. **Python** (alternative for HTTP server)
   ```bash
   # Python 3 is usually pre-installed on most systems
   python3 --version
   ```

### Verify Installation

```bash
rustc --version
wasm-pack --version
node --version  # or python3 --version
```

## Quick Start

Get the demo running in 3 steps:

```bash
# 1. Build the WASM module (Rust to WASM)
wasm-pack build --target web --out-dir pkg --features wasm

# 2. Install Node.js dependencies
npm install

# 3. Build the web demo (HTML, CSS, JS, WASM assets)
npm run build

# 4. Start the development server (optional, for local development)
npm start

# 5. Open the demo in your browser
# For development server: Vite will automatically open a browser or you can navigate to:
# http://localhost:5173 (default Vite port)

```

## Building the WASM Module

### Standard Build

```bash
# Build for web target (ES6 modules) with WASM features
wasm-pack build --target web --out-dir pkg --features wasm
```

### Development Build (with debug symbols)

```bash
# Build with debug information for development and WASM features
wasm-pack build --target web --out-dir pkg --dev --features wasm
```

### Production Build (optimized)

```bash
# Build with maximum optimizations and WASM features
wasm-pack build --target web --out-dir pkg --release --features wasm
```

### Build Output (WASM)

After building the WASM module, you'll find the following files in the `pkg/` directory:

- `animation_player.js` - JavaScript bindings
- `animation_player_bg.wasm` - WebAssembly module
- `animation_player.d.ts` - TypeScript definitions
- `package.json` - Package metadata

## Running the Demo

### Step 1: Build the WASM Module (Rust to WASM)

```bash
# Build for web target with WASM features
wasm-pack build --target web --out-dir pkg --features wasm
```

### Step 2: Install Node.js Dependencies

```bash
# Install Node.js dependencies (including Vite)
npm install
```

### Step 3: Build the Web Demo

```bash
# Build the HTML, CSS, and JavaScript assets for the demo
npm run build
```

### Step 4: Start Development Server (Optional, for local development)

```bash
# Start the Vite development server
npm start
```
