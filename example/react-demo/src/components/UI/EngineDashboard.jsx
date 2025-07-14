import React, { useState, useEffect } from 'react';
import ControlPanel from './ControlPanel.jsx';
import { useAnimationEngine } from '../../hooks/useAnimationEngine.js';

const EngineDashboard = () => {
  const {
    getEngineConfig,
    setEngineConfig,
    updateInterval,
    setUpdateInterval,
  } = useAnimationEngine();

  const [config, setConfig] = useState(null);

  useEffect(() => {
    if (getEngineConfig) {
      const cfg = getEngineConfig();
      setConfig(cfg);
    }
  }, [getEngineConfig]);

  if (!config) {
    return <ControlPanel title="⚙️ Engine Dashboard">Loading...</ControlPanel>;
  }

  const handleChange = (field, value) => {
    setConfig(prev => ({ ...prev, [field]: value }));
  };

  const applyConfig = () => {
    setEngineConfig(config);
  };

  return (
    <ControlPanel title="⚙️ Engine Dashboard" className="engine-dashboard">
      <div className="config-controls">
        <div className="control-group">
          <label>Target FPS</label>
          <input
            type="number"
            value={config.target_fps}
            onChange={e => handleChange('target_fps', parseFloat(e.target.value))}
          />
        </div>
        <div className="control-group">
          <label>Max Players</label>
          <input
            type="number"
            value={config.max_players}
            onChange={e => handleChange('max_players', parseInt(e.target.value))}
          />
        </div>
        <div className="control-group">
          <label>Update Interval (ms)</label>
          <input
            type="number"
            value={updateInterval}
            min="10"
            step="10"
            onChange={e => setUpdateInterval(parseInt(e.target.value))}
          />
        </div>
      </div>
      <button className="btn-primary" onClick={applyConfig}>
        Apply Config
      </button>
    </ControlPanel>
  );
};

export default EngineDashboard;
