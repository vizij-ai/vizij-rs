# Example Vite + React + TypeScript App with WebAssembly

This example demonstrates how to use a Rust WebAssembly module in a Vite project with React and TypeScript.

## How it Works

This project imports and uses the WebAssembly package from the parent `animation-player` project.

### Installing the WebAssembly Package

The WebAssembly package is imported as a local dependency in the `package.json`:

```json
"dependencies": {
  "animation-player": "file:../../animation-player/pkg",
  "react": "^19.1.0",
  "react-dom": "^19.1.0"
}
```

> **Note**: The package path is relative to this project directory. Make sure you have built the WebAssembly package first by running `wasm-pack build --target web` in the animation-player directory.

### Loading and Initializing WebAssembly

The WebAssembly module must be initialized before use. This is done in the React component:

```typescript
import { useEffect, useState } from 'react'
import init, { greet } from 'animation-player'

function App() {
  const [wasmReady, setWasmReady] = useState(false)
  const [wasmError, setWasmError] = useState('')

  useEffect(() => {
    // Initialize the WebAssembly module
    init().then(() => {
      setWasmReady(true)
      setWasmError('')
    }).catch((error) => {
      setWasmReady(false)
      setWasmError(`Failed to load WebAssembly module: ${error.message}`)
    })
  }, [])

  // Rest of component...
}
```

### Using WebAssembly Functions

After initialization, you can call the exported Rust functions:

```typescript
<button onClick={() => {
  setCount((count) => count + 1)
  setText(greet())
}}>
  count is {count}. {wasmReady ? text : 'Loading...'}
  {wasmError && <div className="error">{wasmError}</div>}
</button>
```

## Running the Example

To run this example:

1. First build the WebAssembly package:
```bash
cd ../animation-player
wasm-pack build --target web
```

2. Then install dependencies and start the development server:
```bash
npm install
npm run dev
```

## Testing with Playwright

This project uses Playwright for end-to-end testing.

### Test Configuration

Playwright is configured in `playwright.config.ts`. Key settings include:

- **Base URL**: Set to the local development server URL
- **Browsers**: Tests run in Chromium and Firefox
- **Web Server**: Automatically starts the Vite dev server before running tests

```typescript
/* Base URL to use in actions like `await page.goto('/')`. */
baseURL: 'http://localhost:5174',

/* Run your local dev server before starting the tests */
webServer: {
  command: 'npm run dev',
  url: 'http://localhost:5174',
  reuseExistingServer: !process.env.CI,
},
```

### Running Tests

```bash
# Install Playwright browsers
npx playwright install

# Run tests
npm run test
```

### Test Structure

Tests are located in the `tests` directory. They test various aspects of the application:

- Check if the page title is correct
- Verify page loading
- Test the counter button functionality
- Test WebAssembly integration

## Vite Configuration

Since we are using a local installation of the WebAssembly package, we need to configure Vite to allow access to the package directory.

```typescript
export default defineConfig({
  plugins: [react()],
  server: {
    fs: {
      allow: [".", "../../animation-player/pkg"]
    },
  }
})
```

Consider installing the WASM package with different options to make your web application more portable, and avoid this configuration tweak.

```bash
npm install --install-links=false /path/to/animation-player/pkg
```