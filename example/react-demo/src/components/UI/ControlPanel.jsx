import React from 'react';
import PlayerForm from './PlayerForm';

const ControlPanel = ({ title, addPlayer, children, className = '' }) => {
  return (
    <div className={`control-panel ${className}`}>
      <h3>{title}</h3>
      {addPlayer && <PlayerForm addPlayer={addPlayer} />}
      {children}
    </div>
  );
};

export default ControlPanel;
