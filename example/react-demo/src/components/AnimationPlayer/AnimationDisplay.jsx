import React from 'react';
import { useAnimationPlayerContext } from '../../hooks/useAnimationPlayerContext.js';
import MetricsGrid from '../UI/MetricsGrid.jsx';

const AnimationDisplay = () => {
  const { 
    currentValues, 
    playerState, 
    currentTime, 
    isPlaying, 
    metrics,
    logs,
    clearLogs
  } = useAnimationPlayerContext();

  const formatValue = (value) => {
    if (typeof value === 'object' && value !== null) {
      // Handle wrapped values from WASM
      if ('Float' in value) return value.Float.toFixed(3);
      if ('Int' in value) return value.Int;
      if ('Bool' in value) return value.Bool ? 'true' : 'false';
      return JSON.stringify(value);
    }
    if (typeof value === 'number') return value.toFixed(3);
    return String(value);
  };

  const renderCurrentValues = () => {
    if (!currentValues || (currentValues instanceof Map ? currentValues.size === 0 : Object.keys(currentValues).length === 0)) {
      return <div className="no-values">No values yet...</div>;
    }

    const entries = currentValues instanceof Map 
      ? Array.from(currentValues.entries()) 
      : Object.entries(currentValues);

    return (
      <div className="values-grid">
        {entries.map(([playerId, playerValues]) => {
          const playerEntries = playerValues instanceof Map
            ? Array.from(playerValues.entries())
            : Object.entries(playerValues);

          return (
            <div key={playerId} className="player-values">
              <h4>Player: {playerId}</h4>
              <div className="value-list">
                {playerEntries.map(([trackName, trackValue]) => {
                  return (
                    <div key={`${playerId}-${trackName}`} className="value-item">
                      <span className="value-key">{trackName}:</span>
                      <span className="value-value">{formatValue(trackValue)}</span>
                    </div>
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>
    );
  };

  return (
    <div className="animation-display">
      {/* Current Values Display */}
      <div className="container">
        <h3>Current Animation Values</h3>
        {renderCurrentValues()}
      </div>

      {/* Status Panel */}
      <div className="status-panel">
        <h3>Animation Status</h3>
        <div className="status-info">
          <div className="status-item">
            <strong>State:</strong> 
            <span className={`status-badge ${playerState}`}>
              {playerState}
            </span>
          </div>
          <div className="status-item">
            <strong>Time:</strong> {currentTime?.toFixed(2) || '0.00'}s
          </div>
          <div className="status-item">
            <strong>Playing:</strong> 
            <span className={`status-badge ${isPlaying ? 'playing' : 'stopped'}`}>
              {isPlaying ? 'Yes' : 'No'}
            </span>
          </div>
        </div>
        
        <MetricsGrid metrics={metrics} />
      </div>

      {/* Event Log */}
      {/* <div className="container">
        <div className="log-header">
          <h3>Event Log & Console Output</h3>
          <button className="btn-secondary" onClick={clearLogs}>
            Clear Logs
          </button>
        </div>
        <div className="log-container">
          {logs.length === 0 ? (
            <div className="no-logs">No logs yet...</div>
          ) : (
            <div className="log-entries">
              {logs.map((log, index) => (
                <div key={index} className={`log-entry log-${log.type}`}>
                  <span className="log-timestamp">[{log.timestamp}]</span>
                  <span className="log-message">{log.message}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div> */}
    </div>
  );
};

export default AnimationDisplay;
