# Animation Player - React Demo

A comprehensive React application demonstrating integration with the Animation Player core library built with Rust/WASM. This demo showcases TypeScript support, time series visualization, and modern React patterns.

## Overview

This React demo provides an enhanced interface for the Animation Player, modeling after the `time_series_demo.html` but with improved structure, reusable components, and React-specific optimizations.

## Project Structure

```
react-demo/
├── src/
│   ├── components/
│   │   ├── AnimationPlayer/
│   │   │   ├── AnimationPlayerProvider.jsx   # Context provider with WASM integration
│   │   │   ├── AnimationControls.jsx         # Playback and configuration controls
│   │   │   └── AnimationDisplay.jsx          # Values display and event logging
│   │   ├── TimeSeries/
│   │   │   ├── TimeSeriesChart.jsx           # Chart.js integration for plotting
│   │   │   └── TimeSeriesControls.jsx        # History management and visualization
│   │   └── UI/
│   │       ├── ControlPanel.jsx              # Reusable control panel component
│   │       └── MetricsGrid.jsx               # Performance metrics display
│   ├── hooks/
│   │   └── useTimeSeries.js                  # Custom hook for time series operations
│   ├── utils/
│   │   └── wasmLoader.js                     # WASM module loading utilities
│   ├── App.jsx                               # Main application component
│   ├── App.css                               # Application styles
│   └── main.jsx                              # React entry point
├── public/
│   └── wasm/                                 # WASM files (copied from build)
├── package.json                              # Dependencies and scripts
├── vite.config.js                            # Vite configuration
└── index.html                                # HTML entry point
```

## Integration Architecture

### 1. Context Provider Pattern

The `AnimationPlayerProvider` component wraps the entire application and provides:
- WASM module loading and initialization
- Animation player instance management
- Shared state management (current values, metrics, logs)
- Event handling and state updates

### 2. Custom Hooks

- `useAnimationPlayer()`: Primary hook for accessing player functionality
- `useTimeSeries()`: Specialized hook for time series operations and history management

### 3. Component Structure

**Core Components:**
- **AnimationControls**: Playback controls, configuration, and update management
- **AnimationDisplay**: Real-time value display, status panel, and event logging
- **TimeSeriesControls**: History configuration, statistics, and chart visualization

**UI Components:**
- **ControlPanel**: Reusable wrapper for grouped controls
- **MetricsGrid**: Performance metrics display with responsive layout

### 4. WASM Integration

The integration uses several key patterns:

```javascript
// Async WASM loading with error handling
const wasmModule = await loadWasmModule();

// Player initialization with configuration
const player = wasmModule.AnimationPlayer.new(config);

// Event-driven updates
const updateLoop = () => {
  const currentValues = player.update();
  setCurrentValues(currentValues);
  // Continue update cycle...
};
```

## Key Differences from HTML Demo

### 1. State Management
- **HTML Demo**: Global variables and direct DOM manipulation
- **React Demo**: React Context API with centralized state management

### 2. Event Handling
- **HTML Demo**: Event listeners attached to DOM elements
- **React Demo**: React event handlers with proper cleanup

### 3. Data Flow
- **HTML Demo**: Imperative updates with manual DOM changes
- **React Demo**: Declarative components with reactive data flow

### 4. Component Architecture
- **HTML Demo**: Monolithic structure in single file
- **React Demo**: Modular components with clear separation of concerns

### 5. Time Series Visualization
- **HTML Demo**: Basic text display of history
- **React Demo**: Interactive Chart.js integration with multiple chart types

## Setup and Installation

1. **Install Dependencies**:
   ```bash
   cd react-demo
   npm install
   ```

2. **Copy WASM Files**:
   Ensure the WASM files are built and copied to `public/wasm/`:
   ```bash
   # From the root animation-player-core directory
   npm run build
   cp pkg/*.wasm pkg/*.js react-demo/public/wasm/
   ```

3. **Start Development Server**:
   ```bash
   npm run dev
   ```

4. **Open in Browser**:
   Navigate to `http://localhost:5173`

## Features

### Animation Control
- Play, pause, stop controls
- Seek position with slider
- Speed control (-5x to 5x)
- Playback modes (once, loop, ping-pong)
- Start/end time configuration

### Real-time Display
- Current animation values
- Player state and metrics
- Performance statistics
- Event logging with timestamps

### Time Series Visualization
- History capture configuration
- Interactive charts (line, bar, scatter)
- Data export (JSON, CSV)
- Statistics dashboard
- Memory usage tracking

### TypeScript Support
- Type definitions for WASM bindings
- Proper typing for all React components
- Enhanced development experience with IntelliSense

## Usage Example

```jsx
import React from 'react';
import AnimationPlayerProvider, { useAnimationPlayer } from './components/AnimationPlayer/AnimationPlayerProvider.jsx';

function MyComponent() {
  const { player, currentValues, play, pause } = useAnimationPlayer();
  
  return (
    <div>
      <button onClick={play}>Play</button>
      <button onClick={pause}>Pause</button>
      <div>Current Values: {JSON.stringify(currentValues)}</div>
    </div>
  );
}

function App() {
  return (
    <AnimationPlayerProvider>
      <MyComponent />
    </AnimationPlayerProvider>
  );
}
```

## Configuration

The player can be configured through the provider:

```jsx
<AnimationPlayerProvider
  config={{
    speed: 1.0,
    mode: 'loop',
    startTime: 0,
    endTime: 10,
    updateRate: 60
  }}
>
  {/* Your app components */}
</AnimationPlayerProvider>
```

## Development Notes

### Performance Considerations
- The demo uses `requestAnimationFrame` for smooth updates
- Chart.js rendering is optimized with data limits
- WASM calls are batched to minimize overhead

### Memory Management
- Proper cleanup of intervals and event listeners
- WASM memory is managed through the player lifecycle
- History buffers are configurable to prevent memory leaks

### Error Handling
- Comprehensive error boundaries
- WASM loading failure recovery
- Player initialization error handling

## Building for Production

```bash
npm run build
```

The built application will be in the `dist/` directory and can be served statically.

## Extending the Demo

To add new features:

1. **New Animation Types**: Extend the player configuration in `AnimationPlayerProvider`
2. **Custom Visualizations**: Create new components that consume the `useAnimationPlayer` hook
3. **Additional Chart Types**: Extend `TimeSeriesChart` with new Chart.js configurations
4. **Data Processing**: Add custom hooks for specific data transformations

## Dependencies

- **React 18**: Modern React with concurrent features
- **Vite**: Fast build tool and development server
- **Chart.js**: Interactive charts for time series visualization
- **WASM**: Rust-compiled animation player core

This React integration provides a solid foundation for building complex animation applications with the Animation Player core library.
