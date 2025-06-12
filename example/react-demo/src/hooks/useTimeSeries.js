import { useState, useEffect, useCallback } from 'react';
import { useAnimationPlayerContext } from './useAnimationPlayerContext.js';

export const useTimeSeries = () => {
  const animationPlayer = useAnimationPlayerContext();
  
  const [historyStats, setHistoryStats] = useState({
    totalKeys: 0,
    totalCaptures: 0,
    averageValuesPerKey: 0,
    memoryUsageEstimate: 0,
    captureRate: 0,
    isCapturing: false
  });

  const [historyConfig, setHistoryConfig] = useState({
    maxLength: 100,
    captureInterval: 1
  });

  const [derivativeConfig, setDerivativeConfig] = useState({
    enabled: false,
    derivativeWidth: 1000 // seconds
  });

  const [derivativeHistory, setDerivativeHistory] = useState({});

  // Update stats periodically
  useEffect(() => {
    if (!animationPlayer.isLoaded) return;

    const updateStats = () => {
      try {
        const stats = animationPlayer.getHistoryStats();
        setHistoryStats(stats);
      } catch (error) {
        console.error('Failed to get history stats:', error);
      }
    };

    // Update stats immediately and then every second
    updateStats();
    const interval = setInterval(updateStats, 1000);

    return () => clearInterval(interval);
  }, [animationPlayer.isLoaded, animationPlayer.getHistoryStats]);

  const clearHistory = useCallback(() => {
    if (!animationPlayer.isLoaded) return;
    try {
      animationPlayer.clearHistory();
      setDerivativeHistory({});
      console.log('🗑️ History and derivatives cleared');
    } catch (error) {
      console.error('Failed to clear history:', error);
    }
  }, [animationPlayer.isLoaded, animationPlayer.clearHistory]);

  const toggleDerivatives = useCallback((enabled) => {
    setDerivativeConfig(prev => ({
      ...prev,
      enabled
    }));
    
    if (!enabled) {
      setDerivativeHistory({});
    }
    
    console.log(enabled ? '📈 Derivatives enabled' : '📈 Derivatives disabled');
  }, []);

  const updateDerivativeConfig = useCallback((config) => {
    setDerivativeConfig(prev => ({
      ...prev,
      ...config
    }));
  }, []);

  const getDerivativeHistory = useCallback(() => {
    return derivativeHistory;
  }, [derivativeHistory]);

  const updateHistoryConfig = useCallback((newConfig) => {
    if (!animationPlayer.isLoaded) return;
    try {
      setHistoryConfig(prev => ({ ...prev, ...newConfig }));
      animationPlayer.setHistoryOptions(newConfig);
      console.log(`⚙️ History config updated:`, newConfig);
    } catch (error) {
      console.error('Failed to update history config:', error);
    }
  }, [animationPlayer.isLoaded, animationPlayer.setHistoryOptions]);

  const getValueHistory = useCallback((keyName) => {
    if (!animationPlayer.isLoaded) return [];
    try {
      return animationPlayer.getValueHistory(keyName);
    } catch (error) {
      console.error('Failed to get value history:', error);
      return [];
    }
  }, [animationPlayer.isLoaded, animationPlayer.getValueHistory]);

  const getAllValueHistory = useCallback(() => {
    if (!animationPlayer.isLoaded) return {};
    try {
      return animationPlayer.getAllValueHistory();
    } catch (error) {
      console.error('Failed to get all value history:', error);
      return {};
    }
  }, [animationPlayer.isLoaded, animationPlayer.getAllValueHistory]);

  const exportHistory = useCallback(() => {
    if (!animationPlayer.isLoaded) return null;
    try {
      const history = animationPlayer.getAllValueHistory();
      const metadata = animationPlayer.getHistoryMetadata();
      
      const exportData = {
        timeSeries: history,
        metadata: metadata,
        exportedAt: new Date().toISOString()
      };
      
      console.log('📥 Exported History:', exportData);
      return exportData;
    } catch (error) {
      console.error('Failed to export history:', error);
      return null;
    }
  }, [animationPlayer.isLoaded, animationPlayer.getAllValueHistory, animationPlayer.getHistoryMetadata]);

  const downloadHistoryCSV = useCallback(() => {
    if (!animationPlayer.isLoaded) return;
    try {
      const history = animationPlayer.getAllValueHistory();
      if (Object.keys(history).length === 0) {
        alert('No history data to download');
        return;
      }
      
      // Create CSV content
      const keys = Object.keys(history);
      const maxLength = Math.max(...Object.values(history).map(arr => arr.length));
      
      let csv = 'Index,' + keys.join(',') + '\n';
      
      for (let i = 0; i < maxLength; i++) {
        const row = [i];
        keys.forEach(key => {
          row.push(history[key][i] || '');
        });
        csv += row.join(',') + '\n';
      }
      
      // Download CSV
      const blob = new Blob([csv], { type: 'text/csv' });
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `animation_timeseries_${new Date().getTime()}.csv`;
      a.click();
      window.URL.revokeObjectURL(url);
    } catch (error) {
      console.error('Failed to download history CSV:', error);
    }
  }, [animationPlayer.isLoaded, animationPlayer.getAllValueHistory]);

  const formatBytes = (bytes) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const getAllDerivativeHistory = () => {
      if (!animationPlayer.isLoaded) return {};
      try {
        return animationPlayer.getAllDerivativeHistory();
      } catch (error) {
        console.error('Failed to get all derivative history:', error);
        return {};
      }
    }

  return {
    // State
    historyStats,
    historyConfig,
    derivativeConfig,
    
    // Methods
    clearHistory,
    updateHistoryConfig,
    getValueHistory,
    getAllValueHistory,
    getAllDerivativeHistory,
    exportHistory,
    downloadHistoryCSV,
    
    // Derivative methods
    getDerivativeHistory,
    toggleDerivatives,
    updateDerivativeConfig,
    
    // Utilities
    formatBytes
  };
};
