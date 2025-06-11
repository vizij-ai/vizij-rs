import React, { useState } from 'react';
import { useAnimationPlayerContext } from '../../hooks/useAnimationPlayerContext.js';
import ControlPanel from '../UI/ControlPanel.jsx';

const AnimationControls = () => {
  const { 
    play, 
    pause, 
    stop, 
    seek, 
    currentTime, 
    isPlaying, 
    updatePlayerConfig, 
    config,
    pollingRate,
    updatePollingRate,
    getPlayerIds,
    getPerformanceStats
  } = useAnimationPlayerContext();

  const [seekValue, setSeekValue] = useState(0);

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

  const handleUpdateRateChange = (updateRate) => {
    updatePollingRate(parseInt(updateRate));
  };



  return (
    <div className="controls-grid">
      {/* Playback Controls */}
      <ControlPanel title="Playback Controls">
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
          <button className="btn-secondary" onClick={() => handleSeek(2)}>
            ⏯ Middle
          </button>
          <button className="btn-secondary" onClick={() => handleSeek(4)}>
            ⏭ End
          </button>
        </div>

        <div className="slider-container">
          <label htmlFor="seek-slider">Seek Position:</label>
          <input
            type="range"
            id="seek-slider"
            className="slider"
            min="0"
            max="4"
            step="0.1"
            value={currentTime}
            onChange={(e) => handleSeek(e.target.value)}
          />
          <span>{currentTime?.toFixed(1) || '0.0'}s</span>
        </div>
      </ControlPanel>

      {/* Player Configuration */}
      <ControlPanel title="Player Configuration">
        <div className="slider-container">
          <label htmlFor="speed-slider">Speed (-5.0 to 5.0):</label>
          <input
            type="range"
            id="speed-slider"
            className="slider"
            min="-5"
            max="5"
            step="0.1"
            value={config.speed}
            onChange={(e) => handleSpeedChange(e.target.value)}
          />
          <span>{config.speed}x</span>
        </div>

        <div className="control-group">
          <label htmlFor="mode-select">Playback Mode:</label>
          <select
            id="mode-select"
            value={config.mode}
            onChange={(e) => handleModeChange(e.target.value)}
          >
            <option value="once">Once</option>
            <option value="loop">Loop</option>
            <option value="ping_pong">Ping Pong</option>
          </select>
        </div>

        <div className="control-group">
          <label htmlFor="start-time-input">Start Time (seconds):</label>
          <input
            type="number"
            id="start-time-input"
            min="0"
            step="0.1"
            value={config.startTime}
            onChange={(e) => handleStartTimeChange(e.target.value)}
          />
        </div>

        <div className="control-group">
          <label htmlFor="end-time-input">End Time (seconds):</label>
          <input
            type="number"
            id="end-time-input"
            min="0"
            step="0.1"
            value={config.endTime || ''}
            onChange={(e) => handleEndTimeChange(e.target.value)}
            placeholder="No limit"
          />
          <button onClick={() => handleEndTimeChange('')}>Clear</button>
        </div>
      </ControlPanel>

      {/* Update Mode Controls */}
      <ControlPanel title="Update Mode">
        <div className="slider-container">
          <label htmlFor="fps-slider">Update Rate:</label>
          <input
            type="range"
            id="fps-slider"
            className="slider"
            min="1"
            max="120"
            step="10"
            value={pollingRate}
            onChange={(e) => handleUpdateRateChange(e.target.value)}
          />
          <span>{pollingRate} FPS</span>
        </div>

        <p style={{ fontSize: '12px', color: '#666', margin: '10px 0 0 0' }}>
          Updates are automatically managed by Play/Pause/Stop controls
        </p>
      </ControlPanel>

      {/* Animation Management */}
      <ControlPanel title="Animation Management">
        <div className="button-group">
          <button className="btn-primary" onClick={() => {
            const info = getPlayerIds();
            console.log('Player Info:', info);
          }}>
            Get Info
          </button>
          <button className="btn-secondary" onClick={() => {
            const stats = getPerformanceStats();
            console.log('Performance Stats:', stats);
          }}>
            Performance
          </button>
        </div>
      </ControlPanel>
    </div>
  );
};

export default AnimationControls;
