import React, { useState, useEffect } from 'react';
import { AnimationEngineProvider } from './contexts/AnimationEngineProvider.jsx';
import { useAnimationEngine } from './hooks/useAnimationEngine.js';
import PlayerPanel from './components/AnimationPlayer/PlayerPanel.jsx';
import AnimationLibraryPanel from './components/AnimationPlayer/AnimationLibraryPanel.jsx';
import DataViewport from './components/DataViewport/DataViewport.jsx';
import BakedAnimationPanel from './components/BakedAnimation/BakedAnimationPanel.jsx';
import FileUpload from './components/UI/FileUpload.jsx';
import PlayerForm from './components/UI/PlayerForm.jsx';
import EngineDashboard from './components/UI/EngineDashboard.jsx';
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
    <AnimationEngineProvider>
      <div className="app">
        <div className="demo-container">
          <div className="header">
            <h1>🎬 Animation Player - React Demo</h1>
            <p>Enhanced React integration with TypeScript support and time series visualization</p>
            <button onClick={toggleTheme} className="btn-secondary">
              Toggle {theme === 'light' ? 'Night' : 'Day'} Mode
            </button>
          </div>
          
          <MainContent />
        </div>
      </div>
    </AnimationEngineProvider>
  );
}

const MainContent = () => {
  const { isLoading, error, isLoaded, playerIds, createPlayer } = useAnimationEngine();

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

  if (!isLoaded) {
    return (
      <div className="loading-container">
        <p>Initializing...</p>
      </div>
    );
  }

  return (
    <div className="demo">
      <EngineDashboard />
      {/* File Upload Section */}
      <div className="file-upload-section">
        <FileUpload />
      </div>
      
      {/* Animation Library */}
      <AnimationLibraryPanel />

      {/* Player Creation */}
      <div className="add-player-section">
        <PlayerForm addPlayer={createPlayer} />
      </div>

      {/* Player Panels */}
      <div className="player-panels-grid">
        {playerIds.map((id) => (
          <PlayerPanel key={id} playerId={id} />
        ))}
      </div>
      
      {/* Data Viewport */}
      <DataViewport />
      
      {/* Baked Animation Section */}
      <BakedAnimationPanel />
    </div>
  );
};

export default App;
