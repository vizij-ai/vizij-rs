import React, { useState, useEffect } from 'react';
import { useAnimationInstance } from '../../hooks/useAnimationInstance.js';

const InstanceConfig = ({ playerId, instance }) => {
  const { config, updateConfig } = useAnimationInstance(playerId, instance);
  const [local, setLocal] = useState({ enabled: true, weight: 1, timeScale: 1, instanceStartTime: 0 });

  useEffect(() => {
    console.log(config)
    if (config && config.settings) {
      setLocal({
        enabled: config.settings.enabled,
        weight: config.settings.weight,
        timeScale: config.settings.timeScale,
        instanceStartTime: config.settings.instanceStartTime,
      });
    }
  }, [config]);

  const change = (field, value) => {
    setLocal(prev => ({ ...prev, [field]: value }));
    updateConfig({ [field]: value });
  };

  if (!config) return null;

  return (
    <div className="instance-config">
      <h4>Instance ID: {instance}</h4>
      <div className="control-group">
        <label>
          <input type="checkbox" checked={local.enabled} onChange={e => change('enabled', e.target.checked)} />
          Enabled
        </label>
      </div>
      <div className="control-group">
        <label>Weight:</label>
        <input type="range" min="0" max="1" step="0.01" value={local.weight} onChange={e => change('weight', parseFloat(e.target.value))} />
        <span>{local.weight.toFixed(2)}</span>
      </div>
      <div className="control-group">
        <label>Time Scale:</label>
        <input type="range" min="-5" max="5" step="0.1" value={local.timeScale} onChange={e => change('timeScale', parseFloat(e.target.value))} />
        <span>{local.timeScale.toFixed(2)}</span>
      </div>
      <div className="control-group">
        <label>Start Offset (seconds):</label>
        <input type="number" step="0.1" value={local.instanceStartTime} onChange={e => change('instanceStartTime', parseFloat(e.target.value))} />
      </div>
    </div>
  );
};

export default InstanceConfig;
