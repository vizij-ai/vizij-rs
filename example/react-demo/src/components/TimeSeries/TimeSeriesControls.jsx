import React, { useState, useEffect } from 'react';
import { useTimeSeries } from '../../hooks/useTimeSeries.js';
import ControlPanel from '../UI/ControlPanel.jsx';
import TimeSeriesChart from './TimeSeriesChart.jsx';

const TimeSeriesControls = () => {
  const {
    historyStats,
    historyConfig,
    derivativeConfig,
    clearHistory,
    updateHistoryConfig,
    getAllValueHistory,
    getAllDerivativeHistory,
    getDerivativeHistory,
    toggleDerivatives,
    updateDerivativeConfig,
    exportHistory,
    downloadHistoryCSV,
    formatBytes
  } = useTimeSeries();

  const [showChart, setShowChart] = useState(false);
  const [chartType, setChartType] = useState('line');
  const [timeRange, setTimeRange] = useState('all');
  const [selectedKeys, setSelectedKeys] = useState(new Set());
  const [historyData, setHistoryData] = useState({});
  const [derivativeData, setDerivativeData] = useState({});
  const [showHistory, setShowHistory] = useState(false);
  const [maxLength, setMaxLength] = useState(historyConfig.maxLength);
  const [captureInterval, setCaptureInterval] = useState(historyConfig.captureInterval);

  // Update history data when showing chart
  useEffect(() => {
    if (showChart) {
      const data = getAllValueHistory();
      const derivData = getAllDerivativeHistory(); // don't include in dep array until properly memoized
      setHistoryData(data);
      setDerivativeData(derivData);
      
      // Auto-select all keys if none selected
      if (selectedKeys.size === 0) {
        setSelectedKeys(new Set(Object.keys(data)));
      }
    }
  }, [showChart, getAllValueHistory, selectedKeys.size]);

  const handleShowChart = () => {
    const data = getAllValueHistory();
    if (Object.keys(data).length === 0) {
      alert('No history data to plot. Start the animation to begin capturing values.');
      return;
    }
    setHistoryData(data);
    setShowChart(true);
  };

  const handleCloseChart = () => {
    setShowChart(false);
  };

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

  const handleExportHistory = () => {
    const exportData = exportHistory();
    if (exportData) {
      navigator.clipboard.writeText(JSON.stringify(exportData, null, 2)).then(() => {
        alert('History data copied to clipboard as JSON!');
      }).catch(() => {
        console.log('Export data:', exportData);
        alert('Export data logged to console');
      });
    }
  };

  const handleUpdateConfig = () => {
    updateHistoryConfig({
      maxLength: parseInt(maxLength),
      captureInterval: parseInt(captureInterval)
    });
  };

  const handleShowHistory = () => {
    setShowHistory(!showHistory);
  };

  const renderHistoryDisplay = () => {
    const data = getAllValueHistory();
    
    if (Object.keys(data).length === 0) {
      return <p>No history data captured yet. Start the animation to begin capturing values.</p>;
    }

    return (
      <div className="history-content">
        {Object.entries(data).map(([key, values]) => (
          <div key={key} className="key-history">
            <div className="key-name">{key} ({values.length} values)</div>
            <div className="values">
              [{values.slice(-10).map(v => v.toFixed(3)).join(', ')}
              {values.length > 10 ? '...' : ''}]
            </div>
          </div>
        ))}
      </div>
    );
  };

  const renderKeySelector = () => {
    const keys = Object.keys(historyData);
    
    if (keys.length === 0) return null;

    return (
      <div className="key-selector">
        <span>Select keys to plot:</span>
        {keys.map(key => (
          <div key={key} className="key-checkbox">
            <input
              type="checkbox"
              id={`key_${key}`}
              checked={selectedKeys.has(key)}
              onChange={() => toggleKey(key)}
            />
            <label htmlFor={`key_${key}`}>{key}</label>
          </div>
        ))}
      </div>
    );
  };

  return (
    <div className="timeseries-section">
      {/* Statistics */}
      <div className="container">
        <h3>📊 Statistics</h3>
        <div className="stats">
          <div className="stat-card">
            <div className="stat-title">Total Keys</div>
            <div className="stat-value">{historyStats.totalKeys}</div>
          </div>
          <div className="stat-card">
            <div className="stat-title">Total Captures</div>
            <div className="stat-value">{historyStats.totalCaptures}</div>
          </div>
          <div className="stat-card">
            <div className="stat-title">Avg Values/Key</div>
            <div className="stat-value">{historyStats.averageValuesPerKey.toFixed(1)}</div>
          </div>
          <div className="stat-card">
            <div className="stat-title">Memory Usage</div>
            <div className="stat-value">{formatBytes(historyStats.memoryUsageEstimate)}</div>
          </div>
          <div className="stat-card">
            <div className="stat-title">Capture Rate</div>
            <div className="stat-value">{historyStats.captureRate.toFixed(1)} /s</div>
          </div>
          <div className="stat-card">
            <div className="stat-title">Status</div>
            <div className="stat-value">{historyStats.isCapturing ? 'Capturing' : 'Stopped'}</div>
          </div>
        </div>
      </div>

      {/* Derivative Configuration */}
      <ControlPanel title="📈 Derivative Configuration">
        <div className="config-controls">
          <div className="control-group">
            <label>
              <input
                type="checkbox"
                checked={derivativeConfig.enabled}
                onChange={(e) => toggleDerivatives(e.target.checked)}
              />
              Enable Derivative Capture
            </label>
          </div>
          {derivativeConfig.enabled && (
            <div className="control-group">
              <label>Derivative Width (ms):</label>
              <input
                type="number"
                value={derivativeConfig.derivativeWidth}
                onChange={(e) => updateDerivativeConfig({ derivativeWidth: parseFloat(e.target.value) })}
                min="0.1"
                max="1000"
                step="0.1"
              />
              <span className="input-hint">Smaller values = more sensitive</span>
            </div>
          )}
        </div>
      </ControlPanel>

      {/* History Configuration */}
      <ControlPanel title="History Configuration">
        <div className="config-controls">
          <div className="control-group">
            <label>Max History Length:</label>
            <input
              type="number"
              value={maxLength}
              onChange={(e) => setMaxLength(e.target.value)}
              min="10"
              max="1000"
            />
          </div>
          <div className="control-group">
            <label>Capture Interval:</label>
            <input
              type="number"
              value={captureInterval}
              onChange={(e) => setCaptureInterval(e.target.value)}
              min="1"
              max="10"
            />
          </div>
          <button className="btn-secondary" onClick={handleUpdateConfig}>
            Update Config
          </button>
        </div>
      </ControlPanel>

      {/* Time Series History Controls */}
      <div className="container">
        <h3>📈 Time Series History</h3>
        <div className="controls">
          <button className="btn-info" onClick={handleShowHistory}>
            📋 {showHistory ? 'Hide' : 'Show'} History
          </button>
          <button className="btn-success" onClick={handleExportHistory}>
            📥 Export JSON
          </button>
          <button className="btn-secondary" onClick={downloadHistoryCSV}>
            💾 Download CSV
          </button>
          <button className="btn-primary" onClick={handleShowChart}>
            📊 Show Plot
          </button>
          <button className="btn-danger" onClick={clearHistory}>
            🗑️ Clear History
          </button>
        </div>

        {showHistory && (
          <div className="history-display">
            {renderHistoryDisplay()}
          </div>
        )}
      </div>

      {/* Chart Display */}
      {showChart && (
        <div className="container">
          <div className="chart-controls">
            {renderKeySelector()}
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
          
          <TimeSeriesChart
            historyData={historyData}
            derivativeData={derivativeData}
            selectedKeys={selectedKeys}
            chartType={chartType}
            timeRange={timeRange}
            showDerivatives={true}
            onClose={handleCloseChart}
          />
        </div>
      )}
    </div>
  );
};

export default TimeSeriesControls;
