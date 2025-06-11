import React from 'react';

const MetricBox = ({ label, value, unit = '' }) => {
  return (
    <div className="metric-box">
      <div className="metric-value">{value}{unit}</div>
      <div className="metric-label">{label}</div>
    </div>
  );
};

const MetricsGrid = ({ metrics, className = '' }) => {
  return (
    <div className={`metrics-grid ${className}`}>
      <MetricBox 
        label="FPS" 
        value={metrics.fps?.toFixed(1) || '0'} 
      />
      <MetricBox 
        label="Frame Time (ms)" 
        value={metrics.frame_time_ms?.toFixed(1) || '0'} 
      />
      <MetricBox 
        label="Memory (MB)" 
        value={metrics.memory_usage_mb?.toFixed(1) || '0'} 
      />
      <MetricBox 
        label="Total Updates" 
        value={metrics.total_updates || '0'} 
      />
    </div>
  );
};

export default MetricsGrid;
