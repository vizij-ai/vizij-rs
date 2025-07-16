import React, { useState, useEffect } from 'react';
import { usePlayer } from '../../hooks/usePlayer.js';
import { useAnimationEngine } from '../../hooks/useAnimationEngine.js';
import ControlPanel from '../UI/ControlPanel.jsx';
import InstanceConfig from './InstanceConfig.jsx';

const PlayerPanel = ({ playerId }) => {
  const {
    play,
    pause,
    stop,
    seek,
    playerState,
    playerValues,
    updatePlayerConfig,
    addInstance,
    removePlayer,
  } = usePlayer(playerId);
  const { animationIds } = useAnimationEngine();

  const [seekValue, setSeekValue] = useState(0);
  const [isConfigOpen, setIsConfigOpen] = useState(false);
  const [isValuesOpen, setIsValuesOpen] = useState(true);
  const [isInstancesOpen, setIsInstancesOpen] = useState(false);
  const [selectedAnim, setSelectedAnim] = useState('');

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
    <ControlPanel title={`🎬 Player: ${playerState.name} (${playerId})`}>
      {/* Playback Controls */}
      <div className="button-group">
        <button className="player-btn" onClick={play} title="Play">
          <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"></path></svg>
        </button>
        <button className="player-btn" onClick={pause} title="Pause">
          <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"></path></svg>
        </button>
        <button className="player-btn" onClick={stop} title="Stop">
          <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M6 6h12v12H6z"></path></svg>
        </button>
      </div>

      <div className="slider-container">
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

      {/* Status Panel */}
      <div className="status-info">
        <div className="status-item">
          <strong>State:</strong> 
          <span className={`status-badge ${playerState.state}`}>
            {JSON.stringify(playerState)}
          </span>
        </div>
        <div className="status-item">
          <strong>Playing:</strong> 
          <span className={`status-badge ${playerState.state === 'Playing' ? 'playing' : 'stopped'}`}>
            {playerState.playback_state === 'Playing' ? 'Yes' : 'No'}
          </span>
        </div>
      </div>

      {/* Player Configuration */}
      <div className="collapsible-section">
        <button className="collapsible-header" onClick={() => setIsConfigOpen(!isConfigOpen)}>
          <h4>Configuration</h4>
          <span>{isConfigOpen ? '▲' : '▼'}</span>
        </button>
        {isConfigOpen && (
          <div className="collapsible-content">
            <div className="control-group">
              <label htmlFor={`speed-slider-${playerId}`}>Speed:</label>
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
              <label htmlFor={`mode-select-${playerId}`}>Mode:</label>
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
              <label htmlFor={`start-time-input-${playerId}`}>Start Time (s):</label>
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
              <label htmlFor={`end-time-input-${playerId}`}>End Time (s):</label>
              <input
                type="number"
                id={`end-time-input-${playerId}`}
                min="0"
                step="0.1"
                value={playerState.end_time/1000000000 || ''}
                onChange={(e) => handleEndTimeChange(e.target.value)}
                placeholder="No limit"
              />
            </div>
          </div>
        )}
      </div>

      {/* Animation Instances */}
      <div className="collapsible-section">
        <button className="collapsible-header" onClick={() => setIsInstancesOpen(!isInstancesOpen)}>
          <h4>Instances</h4>
          <span>{isInstancesOpen ? '▲' : '▼'}</span>
        </button>
        {isInstancesOpen && (
          <div className="collapsible-content">
            <div className="control-group" style={{ display: 'flex', gap: '5px' }}>
              <select value={selectedAnim} onChange={e => setSelectedAnim(e.target.value)}>
                <option value="">Select Animation</option>
                {animationIds.map(id => (
                  <option key={id} value={id}>{id}</option>
                ))}
              </select>
              <button className="btn-primary" onClick={() => { if(selectedAnim) { addInstance(selectedAnim); setSelectedAnim(''); } }} disabled={!selectedAnim}>Add</button>
            </div>
            {playerState.instance_ids && playerState.instance_ids.map(inst => (
              <>
                {/* <div>Hello{inst}</div> */}
                <InstanceConfig key={inst} playerId={playerId} instance={inst} />
              </>
            ))}
          </div>
        )}
      </div>

      {/* Current Values Display */}
      <div className="collapsible-section">
        <button className="collapsible-header" onClick={() => setIsValuesOpen(!isValuesOpen)}>
          <h4>Current Values</h4>
          <span>{isValuesOpen ? '▲' : '▼'}</span>
        </button>
        {isValuesOpen && (
          <div className="collapsible-content">
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
          </div>
        )}
      </div>
      <div className="separator"></div>
      <button className="btn-danger" onClick={removePlayer} style={{ width: '100%', marginTop: '10px' }}>
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" style={{ verticalAlign: 'middle', marginRight: '5px' }}>
          <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"></path>
        </svg>
        Remove Player
      </button>
    </ControlPanel>
  );
};

export default PlayerPanel;
