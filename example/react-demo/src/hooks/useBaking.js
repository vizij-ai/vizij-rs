import { useState, useCallback } from 'react';
import { useAnimationEngine } from './useAnimationEngine.js';

export const useBaking = (playerId, animationId) => {
  const animationPlayer = useAnimationEngine();
  
  const [bakingState, setBakingState] = useState({
    isGenerating: false,
    lastBakedData: null,
    lastFrameRate: 60,
    error: null,
    generationTime: 0
  });

  const [bakingConfig, setBakingConfig] = useState({
    frame_rate: 60,
    include_disabled_tracks: false, // Default to false
    apply_track_weights: true, // Default to true
    time_range: null, // Default to null
    interpolation_method: 'cubic',
    include_derivatives: false,
    derivative_width: Math.round(1_000_000_000.0/60),
  });

  /**
   * Generate baked animation data at specified frame rate
   */
  const generateBaked = useCallback(async (frameRate = 60, config = {}) => {
    if (!animationPlayer.isLoaded || !animationId) {
      throw new Error('Animation player not initialized or no animation ID provided');
    }

    setBakingState(prev => ({
      ...prev,
      isGenerating: true,
      error: null
    }));

    const startTime = performance.now();

    try {
      // Merge provided config with default baking config
      const finalConfig = {
        ...bakingConfig,
        ...config,
        frame_rate: frameRate,
        include_disabled_tracks: config.includeDisabledTracks || bakingConfig.include_disabled_tracks,
        apply_track_weights: config.applyTrackWeights !== undefined ? config.applyTrackWeights : bakingConfig.apply_track_weights,
        time_range: config.timeRange || bakingConfig.time_range,
        interpolation_method: config.interpolationMethod || bakingConfig.interpolation_method,
        include_derivatives: config.includeDerivatives || bakingConfig.include_derivatives,
        derivative_width: config.derivativeWidth || bakingConfig.derivative_width,
      };
      
      const configJson = JSON.stringify(finalConfig);
      const bakedData = animationPlayer.bakeAnimation(animationId, configJson);
      
      const generationTime = performance.now() - startTime;

      console.log('✅ Baking completed:', {
        frameRate,
        sampleCount: bakedData.frame_count,
        generationTime: `${generationTime.toFixed(2)}ms`,
        tracks: Object.keys(bakedData.tracks || {}).length,
        method: bakedData.metadata?.method || 'native'
      });

      setBakingState(prev => ({
        ...prev,
        isGenerating: false,
        lastBakedData: bakedData,
        lastFrameRate: frameRate,
        generationTime,
        error: null
      }));

      return bakedData;
    } catch (error) {
      console.error('❌ Baking failed:', error);
      setBakingState(prev => ({
        ...prev,
        isGenerating: false,
        error: error.message
      }));
      throw error;
    }
  }, [animationPlayer, bakingConfig, animationId]);

  /**
   * Convert baked data to format suitable for chart visualization
   */
  const getBakedChartData = useCallback((bakedData = null) => {
    const data = bakedData || bakingState.lastBakedData;
    if (!data || !data.tracks) {
      return {};
    }

    const chartData = {};
    
    Object.entries(data.tracks).forEach(([trackName, samples]) => {
      // Extract time and value arrays
      const times = samples.map(sample => sample[0]/1_000_000_000);
      const values = samples.map(sample => {
        const value = sample[1];
        // Handle different value types (Float, Int, etc.)
        if (typeof value === 'object' && value !== null) {
          if ('Float' in value) return value.Float;
          if ('Int' in value) return value.Int;
          if ('Bool' in value) return value.Bool ? 1 : 0;
          // For complex types like transforms, extract first numeric value
          const firstValue = Object.values(value)[0];
          return typeof firstValue === 'number' ? firstValue : 0;
        }
        return typeof value === 'number' ? value : 0;
      });

      chartData[trackName] = {
        times,
        values,
        frameRate: data.frame_rate,
        sampleCount: values.length
      };
    });
    return chartData;
  }, [bakingState.lastBakedData]);


  /**
   * Update baking configuration
   */
  const updateConfig = useCallback((newConfig) => {
    setBakingConfig(prev => ({
      ...prev,
      ...newConfig
    }));
  }, []);

  /**
   * Clear baked data
   */
  const clearBaked = useCallback(() => {
    setBakingState(prev => ({
      ...prev,
      lastBakedData: null,
      error: null,
      generationTime: 0
    }));
  }, []);

  /**
   * Get baking statistics
   */
  const getBakingStats = useCallback(() => {
    const { lastBakedData, lastFrameRate, generationTime } = bakingState;
    
    if (!lastBakedData) {
      return {
        hasData: false,
        frameRate: 0,
        sampleCount: 0,
        trackCount: 0,
        duration: 0,
        generationTime: 0,
        memoryEstimate: 0
      };
    }

    const trackCount = Object.keys(lastBakedData.tracks || {}).length;
    const sampleCount = lastBakedData.frame_count || 0;
    const duration = lastBakedData.duration || 0;
    
    // Estimate memory usage (rough calculation)
    const bytesPerSample = 16; // Time + value pair
    const memoryEstimate = sampleCount * trackCount * bytesPerSample;

    return {
      hasData: true,
      frameRate: lastFrameRate,
      sampleCount,
      trackCount,
      duration,
      generationTime,
      memoryEstimate,
      method: lastBakedData.metadata?.method || 'native'
    };
  }, [bakingState]);

  /**
   * Export baked data as JSON
   */
  const exportBaked = useCallback(() => {
    if (!bakingState.lastBakedData) {
      throw new Error('No baked data to export');
    }

    const exportData = {
      ...bakingState.lastBakedData,
      metadata: {
        ...bakingState.lastBakedData.metadata,
        generatedAt: new Date().toISOString(),
        frameRate: bakingState.lastFrameRate,
        generationTime: bakingState.generationTime,
        exportedBy: 'Animation Player React Demo'
      }
    };

    return exportData;
  }, [bakingState]);

  /**
   * Download baked data as JSON file
   */
  const downloadBaked = useCallback(() => {
    try {
      const exportData = exportBaked();
      const blob = new Blob([JSON.stringify(exportData, null, 2)], { 
        type: 'application/json' 
      });
      
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `baked_animation_${bakingState.lastFrameRate}fps_${Date.now()}.json`;
      a.click();
      window.URL.revokeObjectURL(url);
      
      console.log('📥 Baked data downloaded');
    } catch (error) {
      console.error('Failed to download baked data:', error);
      throw error;
    }
  }, [exportBaked, bakingState.lastFrameRate]);

  /**
   * Format bytes for display
   */
  const formatBytes = useCallback((bytes) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }, []);

  return {
    // State
    bakingState,
    bakingConfig,
    
    // Methods
    generateBaked,
    updateConfig,
    clearBaked,
    
    // Data access
    getBakedChartData,
    getBakingStats,
    
    // Export
    exportBaked,
    downloadBaked,
    
    // Utilities
    formatBytes
  };
};
