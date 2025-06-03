import React, { useState, useEffect } from 'react';
import AnimationPlayerProvider, { useAnimationPlayer } from './components/AnimationPlayer/AnimationPlayerProvider.jsx';
import AnimationControls from './components/AnimationPlayer/AnimationControls.jsx';
import AnimationDisplay from './components/AnimationPlayer/AnimationDisplay.jsx';
import TimeSeriesControls from './components/TimeSeries/TimeSeriesControls.jsx';
import BakedAnimationPanel from './components/BakedAnimation/BakedAnimationPanel.jsx';
import FileUpload from './components/UI/FileUpload.jsx';
import './App.css';

function App() {
  const [theme, setTheme] = useState('dark'); // Set default theme to dark

  useEffect(() => {
    document.body.setAttribute('data-theme', theme);
  }, [theme]);

  const toggleTheme = () => {
    setTheme((prevTheme) => (prevTheme === 'light' ? 'dark' : 'light'));
  };

  return (
    <AnimationPlayerProvider>
      <div className="app">
        <div className="demo-container">
          <div className="header">
            <h1>🎬 Animation Player - React Demo</h1>
            <p>Enhanced React integration with TypeScript support and time series visualization</p>
            <button onClick={toggleTheme} className="btn-secondary">
              Toggle {theme === 'light' ? 'Night' : 'Day'} Mode
            </button>
          </div>
          
          <LoadingWrapper />
        </div>
      </div>
    </AnimationPlayerProvider>
  );
}

const LoadingWrapper = () => {
  const { isLoading, error, isInitialized } = useAnimationPlayer();

  if (isLoading) {
    return (
      <div className="loading-container">
        <div className="loading-spinner"></div>
        <p>Loading WASM module and initializing animation player...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="error-container">
        <h3>Failed to initialize animation player</h3>
        <p style={{ color: 'red' }}>{error}</p>
        <p>Please check the console for more details.</p>
      </div>
    );
  }

  if (!isInitialized) {
    return (
      <div className="loading-container">
        <p>Initializing...</p>
      </div>
    );
  }

  return <MainDemo />;
};

const MainDemo = () => {
  return (
    <div className="demo">
      {/* File Upload Section */}
      <div className="file-upload-section">
        <FileUpload />
      </div>
      
      {/* Animation Display and Controls */}
      <AnimationDisplay />
      
      {/* Main Controls */}
      <AnimationControls />
      
      {/* Time Series Section */}
      <TimeSeriesControls />
      
      {/* Baked Animation Section */}
      <BakedAnimationPanel />
    </div>
  );
};

export default App;
