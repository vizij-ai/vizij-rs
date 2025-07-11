import React, { useState, useEffect, useCallback } from 'react';
import { useEngineDataViewport } from '../../hooks/useEngineDataViewport.js';
import TimeSeriesChart from '../TimeSeries/TimeSeriesChart.jsx';
import ControlPanel from '../UI/ControlPanel.jsx';

const DataViewport = () => {
  const [durationMs, setDurationMs] = useState(10000); // Default to 10 seconds
  const [chartType, setChartType] = useState('line');
  const [timeRange, setTimeRange] = useState('all');
  const [selectedKeys, setSelectedKeys] = useState(new Set());

  const historyData = useEngineDataViewport({ durationMs, enabled: true });

  const allAvailableKeys = useCallback(() => {
    const keys = new Set();
    for (const playerId in historyData) {
      for (const trackName in historyData[playerId]) {
        // keys.add(`${playerId}.${trackName}`);
        keys.add(`${trackName}`);
      }
    }
    return Array.from(keys).sort();
  }, [historyData]);

  useEffect(() => {
    // Auto-select all keys if none selected when historyData changes
    if (selectedKeys.size === 0 && Object.keys(historyData).length > 0) {
      setSelectedKeys(new Set(allAvailableKeys()));
    }
  }, [historyData, selectedKeys, allAvailableKeys]);

  const toggleKey = (key) => {
    setSelectedKeys(prev => {
      const newSet = new Set(prev);
      if (newSet.has(key)) {
        newSet.delete(key);
      } else {
        newSet.add(key);
      }
      return newSet;
    });
  };

  const handleDurationChange = (e) => {
    setDurationMs(parseInt(e.target.value));
  };

  const renderKeySelector = () => {
    const uniqueKeys = allAvailableKeys();
    
    if (uniqueKeys.length === 0) return null;

    return (
      <div className="key-selector">
        <span>Select keys to plot:</span>
        {uniqueKeys.map(key => (
          <div key={key} className="key-checkbox">
            <input
              type="checkbox"
              id={`key_${key}`}
              checked={selectedKeys.has(key)}
              onChange={() => toggleKey(key)}
            />
            <label htmlFor={`key_${key}`}>{key.slice(0,8)}</label>
          </div>
        ))}
      </div>
    );
  };

  return (
    <ControlPanel title="📈 Animation Data Viewport">
      <div className="config-controls">
        <div className="control-group">
          <label>Viewport Duration (ms):</label>
          <input
            type="number"
            value={durationMs}
            onChange={handleDurationChange}
            min="1000"
            max="60000"
          />
        </div>
        <div className="chart-options">
          <label>
            Chart Type:
            <select value={chartType} onChange={(e) => setChartType(e.target.value)}>
              <option value="line">Line Chart</option>
              <option value="scatter">Scatter Plot</option>
              <option value="bar">Bar Chart</option>
            </select>
          </label>
          <label>
            Time Range:
            <select value={timeRange} onChange={(e) => setTimeRange(e.target.value)}>
              <option value="all">All Data</option>
              <option value="last100">Last 100 Points</option>
              <option value="last50">Last 50 Points</option>
              <option value="last20">Last 20 Points</option>
            </select>
          </label>
        </div>
      </div>

      <div className="chart-controls">
        {renderKeySelector()}
      </div>
      
      <TimeSeriesChart
        historyData={historyData}
        selectedKeys={selectedKeys}
        chartType={chartType}
        timeRange={timeRange}
        onClose={() => {}} // No close button needed here
      />
    </ControlPanel>
  );
};

export default DataViewport;
