# Animation Player React Demo

## Overview

This project is a comprehensive React demo showcasing the integration of the Animation Player WebAssembly (WASM) module. 
It demonstrates advanced features such as animation playback, time series visualization, baked animation processing, 
and a file upload for custom animations.

## Project Structure

The project is organized as follows:
- **`src/`**: Source code for the React application.
  - **`components/`**: UI components for animation controls, display, time series charts, baked animation panel, and file upload.
  - **`contexts/`**: React context for managing the animation player state.
  - **`hooks/`**: Custom hooks for interacting with the animation player WASM module, baking animations, and time series data.
  - **`App.jsx`**: Main application component tying all features together.
  - **`App.css`**: Styling for the application with support for dark and light themes.
- **`public/`**: Static assets, including sample animation files like `test_animation.json`.
- **`package.json`**: Project configuration and dependencies, including the local animation-player WASM package.

## Installation

To set up the project, ensure you have Node.js installed. 
Then, run the following command in the `example/react-demo/` directory to install the dependencies:

```bash
npm install
```

This will install the required packages, including the local animation-player WASM module referenced from `../../animation-player/pkg`.

## Running the Demo

Start the development server with:

```bash
npm run dev
```

This uses Vite to serve the application locally. 
Open your browser and navigate to the provided URL (typically `http://localhost:5173`) to interact with the demo.

## Building for Production

To create a production-ready build, run:

```bash
npm run build
```

This compiles the application into the `dist/` directory, optimized for deployment.

## Testing in Preview

- **Preview Build**: After building, preview the production build locally with:
  
  ```bash
  npm run preview
  ```
  
  This serves the contents of the `dist/` directory for testing before deployment.

- **Clean Build Artifacts**: Remove the build output and dependencies directories with:
  
  ```bash
  npm run clean
  ```
  
  This deletes the `dist/` and `node_modules/` directories to ensure a fresh build environment. 
  Note that you will need to run `npm install` after this command to reinstall dependencies.

## Features

This demo highlights the following capabilities of the Animation Player:
- **Animation Playback**: Control animation with play, pause, stop, and speed adjustments.
- **Time Series Visualization**: View animation data over time with interactive charts using Chart.js.
- **Baked Animation**: Process animations for optimized playback or export.
- **File Upload**: Load custom animation files in JSON format to test different scenarios.
- **Theme Toggle**: Switch between dark and light modes for a comfortable viewing experience.

## Feature Roadmap

The following enhancements are planned for future updates to this demo:
- **Engine/Player/Instance Hierarchy Management**: Improve the structure and demonstration of the 
animation engine, player, and instance relationships for clearer control and visualization of complex animations.
- **Improved UI**: Enhance the user interface with better layouts, more intuitive controls, 
and responsive design to improve user experience across devices.
- **Full Feature Support**: Expand the demo to support all features of the animation-player WASM module, 
ensuring comprehensive testing and demonstration of its capabilities.

For further details on the Animation Player WASM module, refer to the main project documentation at `../../README.md`.
