import React, { useState, useEffect } from 'react';
import { usePlayer } from '../../hooks/usePlayer.js';
import ControlPanel from '../UI/ControlPanel.jsx';

const PlayerPanel = ({ playerId }) => {
  const { 
    play, 
    pause, 
    stop, 
    seek, 
    playerState,
    playerValues,
    updatePlayerConfig,
  } = usePlayer(playerId);

  const [seekValue, setSeekValue] = useState(0);

  useEffect(() => {
    if (playerState) {
      setSeekValue(playerState.current_player_time || 0);
    }
  }, [playerState]);

  if (!playerState) {
    return <ControlPanel title={`Player: ${playerId}`}>Loading player controls...</ControlPanel>;
  }
  
  const handleSeek = (value) => {
    const time = parseFloat(value);
    setSeekValue(time);
    seek(time);
  };

  const handleSpeedChange = (speed) => {
    updatePlayerConfig({ speed: parseFloat(speed) });
  };

  const handleModeChange = (mode) => {
    updatePlayerConfig({ mode });
  };

  const handleStartTimeChange = (startTime) => {
    updatePlayerConfig({ startTime: parseFloat(startTime) });
  };

  const handleEndTimeChange = (endTime) => {
    const time = endTime === '' ? null : parseFloat(endTime);
    updatePlayerConfig({ endTime: time });
  };

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

  return (
    <ControlPanel title={`🎬 Player: ${playerId}`}>
      {/* Playback Controls */}
      <div className="button-group">
        <button className="btn-success" onClick={play}>
          ▶ Play
        </button>
        <button className="btn-warning" onClick={pause}>
          ⏸ Pause
        </button>
        <button className="btn-danger" onClick={stop}>
          ⏹ Stop
        </button>
      </div>
      
      <div className="button-group">
        <button className="btn-secondary" onClick={() => handleSeek(0)}>
          ⏮ Start
        </button>
        <button className="btn-secondary" onClick={() => handleSeek(playerState.duration / 2)}>
          ⏯ Middle
        </button>
        <button className="btn-secondary" onClick={() => handleSeek(playerState.duration)}>
          ⏭ End
        </button>
      </div>

      <div className="slider-container">
        <label htmlFor={`seek-slider-${playerId}`}>Seek Position:</label>
        <input
          type="range"
          id={`seek-slider-${playerId}`}
          className="slider"
          min="0"
          max={playerState.duration || 4}
          step="0.01"
          value={seekValue}
          onChange={(e) => handleSeek(e.target.value)}
        />
        <span>{playerState.current_player_time?.toFixed(2) || '0.00'}s / {playerState.duration?.toFixed(2) || '0.00'}s</span>
      </div>

      {/* Player Configuration */}
      <h4>Configuration</h4>
      <div className="control-group">
        <label htmlFor={`speed-slider-${playerId}`}>Speed (-5.0 to 5.0):</label>
        <input
          type="range"
          id={`speed-slider-${playerId}`}
          className="slider"
          min="-5"
          max="5"
          step="0.1"
          value={playerState.speed}
          onChange={(e) => handleSpeedChange(e.target.value)}
        />
        <span>{playerState.speed}x</span>
      </div>

      <div className="control-group">
        <label htmlFor={`mode-select-${playerId}`}>Playback Mode:</label>
        <select
          id={`mode-select-${playerId}`}
          value={playerState.mode.toLowerCase()}
          onChange={(e) => handleModeChange(e.target.value)}
        >
          <option value="once">Once</option>
          <option value="loop">Loop</option>
          <option value="ping_pong">Ping Pong</option>
        </select>
      </div>

      <div className="control-group">
        <label htmlFor={`start-time-input-${playerId}`}>Start Time (seconds):</label>
        <input
          type="number"
          id={`start-time-input-${playerId}`}
          min="0"
          step="0.1"
          value={playerState.start_time/1000000000}
          onChange={(e) => handleStartTimeChange(e.target.value)}
        />
      </div>

      <div className="control-group">
        <label htmlFor={`end-time-input-${playerId}`}>End Time (seconds):</label>
        <input
          type="number"
          id={`end-time-input-${playerId}`}
          min="0"
          step="0.1"
          value={playerState.end_time/1000000000 || ''}
          onChange={(e) => handleEndTimeChange(e.target.value)}
          placeholder="No limit"
        />
        <button onClick={() => handleEndTimeChange('')}>Clear</button>
      </div>

      {/* Status Panel */}
      <h4>Status</h4>
      <div className="status-info">
        <div className="status-item">
          <strong>State:</strong> 
          <span className={`status-badge ${playerState.state}`}>
            {playerState.state}
          </span>
        </div>
        <div className="status-item">
          <strong>Time:</strong> {playerState.current_player_time?.toFixed(2) || '0.00'}s
        </div>
        <div className="status-item">
          <strong>Playing:</strong> 
          <span className={`status-badge ${playerState.state === 'Playing' ? 'playing' : 'stopped'}`}>
            {playerState.state === 'Playing' ? 'Yes' : 'No'}
          </span>
        </div>
      </div>

      {/* Current Values Display */}
      <h4>Current Values</h4>
      <div className="player-values">
        {playerValues.size === 0 ? (
          <div className="no-values">No values yet...</div>
        ) : (
          <div className="value-list">
            {Object.entries(playerValues).sort(([nameA], [nameB]) => nameA.localeCompare(nameB)).map(([trackName, trackValue]) => (
              <div key={trackName} className="value-item">
                <span className="value-key">{trackName.slice(0,8)}:</span>
                <span className="value-value">{formatValue(trackValue)}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </ControlPanel>
  );
};

export default PlayerPanel;
