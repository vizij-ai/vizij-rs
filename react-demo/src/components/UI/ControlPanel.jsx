import React from 'react';

const ControlPanel = ({ title, children, className = '' }) => {
  return (
    <div className={`control-panel ${className}`}>
      <h3>{title}</h3>
      {children}
    </div>
  );
};

export default ControlPanel;
