import React, { useState } from 'react';
import { useAnimationEngine } from '../../contexts/AnimationEngineContext.jsx';
import ControlPanel from '../UI/ControlPanel.jsx';

const AnimationLibraryPanel = () => {
  const { animationIds, playerIds, createPlayer, addInstance } = useAnimationEngine();
  const [selected, setSelected] = useState({});
  const [status, setStatus] = useState('');

  const handleAssign = async (animationId) => {
    let target = selected[animationId];
    if (!target) return;
    if (target === 'new') {
      target = createPlayer();
    }
    try {
      await addInstance(target, animationId);
      setStatus(`Assigned ${animationId} to player ${target}`);
      setTimeout(() => setStatus(''), 3000);
    } catch (e) {
      setStatus(`Error: ${e.message}`);
      setTimeout(() => setStatus(''), 5000);
    }
  };

  if (animationIds.length === 0) {
    return <ControlPanel title="🎞 Loaded Animations">No animations loaded.</ControlPanel>;
  }

  return (
    <ControlPanel title="🎞 Loaded Animations">
      {animationIds.map(id => (
        <div key={id} className="control-group">
          <label>{id}</label>
          <div style={{ display: 'flex', gap: '5px' }}>
            <select value={selected[id] || ''} onChange={e => setSelected({ ...selected, [id]: e.target.value })}>
              <option value="">Select Player</option>
              <option value="new">Create New Player</option>
              {playerIds.map(pid => (
                <option key={pid} value={pid}>{pid}</option>
              ))}
            </select>
            <button className="btn-primary" onClick={() => handleAssign(id)} disabled={!selected[id]}>Assign</button>
          </div>
        </div>
      ))}
      {status && <div className="upload-status">{status}</div>}
    </ControlPanel>
  );
};

export default AnimationLibraryPanel;
