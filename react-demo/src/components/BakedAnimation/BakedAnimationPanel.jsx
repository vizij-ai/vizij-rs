import React, { useState, useEffect } from 'react';
import { Line } from 'react-chartjs-2';
import { useBaking } from '../../hooks/useBaking.js';
import { useTimeSeries } from '../../hooks/useTimeSeries.js';
import ControlPanel from '../UI/ControlPanel.jsx';

const BakedAnimationPanel = () => {
  const {
    bakingState,
    bakingConfig,
    generateBaked,
    updateConfig,
    clearBaked,
    getBakedChartData,
    getBakingStats,
    downloadBaked,
    formatBytes
  } = useBaking();

  const [frameRate, setFrameRate] = useState(60);
  const [showBaked, setShowBaked] = useState(false);
  const [selectedTrack, setSelectedTrack] = useState('');
  const [availableTracks, setAvailableTracks] = useState([]);

  // Update available tracks when baked data changes
  useEffect(() => {
    if (bakingState.lastBakedData && bakingState.lastBakedData.tracks) {
      const tracks = Object.keys(bakingState.lastBakedData.tracks);
      setAvailableTracks(tracks);
      if (tracks.length > 0 && !selectedTrack) {
        setSelectedTrack(tracks[0]);
      }
    }
  }, [bakingState.lastBakedData, selectedTrack]);

  const handleGenerateBaked = async () => {
    try {
      await generateBaked(frameRate);
      setShowBaked(true);
    } catch (error) {
      console.error('Failed to generate baked animation:', error);
      alert(`Failed to generate baked animation: ${error.message}`);
    }
  };

  const handleFrameRateChange = (e) => {
    const value = parseInt(e.target.value);
    if (value >= 1 && value <= 300) {
      setFrameRate(value);
      updateConfig({ frameRate: value });
    }
  };

  const handleClearBaked = () => {
    clearBaked();
    setShowBaked(false);
    setSelectedTrack('');
  };

  const handleDownload = () => {
    try {
      downloadBaked();
    } catch (error) {
      alert(`Failed to download: ${error.message}`);
    }
  };

  const renderBakedChart = () => {
    if (!selectedTrack || !bakingState.lastBakedData) {
      return (
        <div className="no-chart-data">
          Select a track to view the baked animation data.
        </div>
      );
    }

    const bakedChartData = getBakedChartData();
    const bakedTrackData = bakedChartData[selectedTrack];

    if (!bakedTrackData) {
      return (
        <div className="no-chart-data">
          No baked data available for track: {selectedTrack}
        </div>
      );
    }

    // Only show baked discrete samples
    const datasets = [{
      label: `Baked Animation (${bakedTrackData.frameRate} FPS)`,
      data: bakedTrackData.times.map((time, index) => ({
        x: time,
        y: bakedTrackData.values[index]
      })),
      borderColor: '#FF6384',
      backgroundColor: '#FF6384',
      borderWidth: 3,
      pointRadius: 5,
      pointHoverRadius: 8,
      tension: 0,
      fill: false,
      showLine: true,
      pointStyle: 'circle'
    }];

    const chartData = { datasets };

    const chartOptions = {
      responsive: true,
      maintainAspectRatio: false,
      interaction: {
        mode: 'index',
        intersect: false
      },
      plugins: {
        title: {
          display: true,
          text: `Baked Animation - ${selectedTrack} (${bakedTrackData.frameRate} FPS)`,
          font: { size: 16, weight: 'bold' }
        },
        legend: {
          display: true,
          position: 'top'
        },
        tooltip: {
          callbacks: {
            title: function(context) {
              return `Time: ${context[0].parsed.x.toFixed(3)}s`;
            },
            label: function(context) {
              return `Value: ${context.parsed.y.toFixed(3)}`;
            }
          }
        }
      },
      scales: {
        x: {
          type: 'linear',
          display: true,
          title: {
            display: true,
            text: 'Time (seconds)',
            font: { weight: 'bold' }
          },
          grid: {
            color: '#e0e0e0'
          }
        },
        y: {
          display: true,
          title: {
            display: true,
            text: 'Value',
            font: { weight: 'bold' }
          },
          grid: {
            color: '#e0e0e0'
          }
        }
      },
      animation: {
        duration: 300
      }
    };

    return (
      <div className="baked-chart">
        <div className="chart-wrapper" style={{ height: '450px' }}>
          <Line data={chartData} options={chartOptions} />
        </div>
      </div>
    );
  };

  const renderBakingStats = () => {
    const stats = getBakingStats();
    
    if (!stats.hasData) {
      return (
        <div className="stats-placeholder">
          Generate baked animation to see statistics
        </div>
      );
    }

    return (
      <div className="stats">
        <div className="stat-card">
          <div className="stat-title">Frame Rate</div>
          <div className="stat-value">{stats.frameRate} FPS</div>
        </div>
        <div className="stat-card">
          <div className="stat-title">Sample Count</div>
          <div className="stat-value">{stats.sampleCount.toLocaleString()}</div>
        </div>
        <div className="stat-card">
          <div className="stat-title">Track Count</div>
          <div className="stat-value">{stats.trackCount}</div>
        </div>
        <div className="stat-card">
          <div className="stat-title">Duration</div>
          <div className="stat-value">{stats.duration.toFixed(2)}s</div>
        </div>
        <div className="stat-card">
          <div className="stat-title">Generation Time</div>
          <div className="stat-value">{stats.generationTime.toFixed(2)}ms</div>
        </div>
        <div className="stat-card">
          <div className="stat-title">Memory Est.</div>
          <div className="stat-value">{formatBytes(stats.memoryEstimate)}</div>
        </div>
      </div>
    );
  };

  return (
    <div className="baked-animation-section">
      {/* Controls */}
      <ControlPanel title="🥧 Baked Animation Generator">
        <div className="baking-controls">
          <div className="control-group">
            <label htmlFor="frameRate">Frame Rate (FPS):</label>
            <input
              id="frameRate"
              type="number"
              value={frameRate}
              onChange={handleFrameRateChange}
              min="1"
              max="300"
              step="1"
            />
            <span className="input-hint">1-300 FPS</span>
          </div>

          <div className="action-buttons">
            <button 
              className="btn-primary"
              onClick={handleGenerateBaked}
              disabled={bakingState.isGenerating}
            >
              {bakingState.isGenerating ? '⏳ Generating...' : '🚀 Generate Baked Animation'}
            </button>
            
            {bakingState.lastBakedData && (
              <>
                <button 
                  className="btn-secondary"
                  onClick={handleDownload}
                >
                  💾 Download JSON
                </button>
                <button 
                  className="btn-danger"
                  onClick={handleClearBaked}
                >
                  🗑️ Clear
                </button>
              </>
            )}
          </div>

          {bakingState.error && (
            <div className="error-message">
              ❌ Error: {bakingState.error}
            </div>
          )}
        </div>
      </ControlPanel>

      {/* Statistics */}
      {(bakingState.lastBakedData || bakingState.isGenerating) && (
        <div className="container">
          <h3>📊 Baking Statistics</h3>
          {renderBakingStats()}
        </div>
      )}

      {/* Track Selection & Baked Chart */}
      {showBaked && bakingState.lastBakedData && (
        <div className="container">
          <h3>📈 Baked Animation Data</h3>
          
          <div className="track-selector">
            <label htmlFor="trackSelect">Select Track:</label>
            <select 
              id="trackSelect"
              value={selectedTrack} 
              onChange={(e) => setSelectedTrack(e.target.value)}
            >
              <option value="">-- Select a track --</option>
              {availableTracks.map(track => (
                <option key={track} value={track}>{track}</option>
              ))}
            </select>
          </div>

          {renderBakedChart()}

          <div className="chart-legend">
            <div className="legend-item">
              <span className="legend-line baked"></span>
              <span>Baked Animation Samples at {frameRate} FPS</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default BakedAnimationPanel;
