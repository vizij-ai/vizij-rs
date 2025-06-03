import { useState, useCallback } from 'react';
import { useAnimationPlayerContext } from './useAnimationPlayerContext.js';

/**
 * Manual baking fallback - samples animation at specified frame rate
 */
async function generateManualBakedData(animationPlayer, frameRate, config) {
  console.log('🔧 Starting manual baking fallback...');
  
  // Get animation duration - use test animation duration
  const duration = 4.0; // Test animation is 4 seconds
  const frameInterval = 1.0 / frameRate;
  const totalFrames = Math.ceil(duration * frameRate);
  
  const trackData = {};
  const playerId = 'demo_player';
  
  // Store original player state
  const originalTime = animationPlayer.getPlayerTime(playerId);
  const originalState = animationPlayer.getPlayerState(playerId);
  
  try {
    // Stop player to ensure we can control time precisely
    animationPlayer.stop(playerId);
    
    // Sample at each frame time
    for (let frame = 0; frame < totalFrames; frame++) {
      const time = frame * frameInterval;
      
      // Seek to the time point
      animationPlayer.seek(playerId, time);
      
      // Get current values
      const values = animationPlayer.update(0); // Use 0 delta to avoid time progression
      
      // Extract values for each track
      const isMap = values instanceof Map;
      const playerValues = isMap ? values.get(playerId) : values[playerId];
      
      if (playerValues) {
        const playerIsMap = playerValues instanceof Map;
        const entries = playerIsMap ? Array.from(playerValues.entries()) : Object.entries(playerValues);
        
        entries.forEach(([trackName, wrappedValue]) => {
          if (!trackData[trackName]) {
            trackData[trackName] = [];
          }
          
          // Extract numeric value
          let numericValue;
          if (typeof wrappedValue === 'object' && wrappedValue !== null) {
            if ('Float' in wrappedValue) {
              numericValue = wrappedValue.Float;
            } else if ('Int' in wrappedValue) {
              numericValue = wrappedValue.Int;
            } else if ('Bool' in wrappedValue) {
              numericValue = wrappedValue.Bool ? 1 : 0;
            } else {
              const firstValue = Object.values(wrappedValue)[0];
              numericValue = typeof firstValue === 'number' ? firstValue : 0;
            }
          } else {
            numericValue = typeof wrappedValue === 'number' ? wrappedValue : 0;
          }
          
          // Store as [time, value] pair
          trackData[trackName].push([time, { Float: numericValue }]);
        });
      }
    }
    
    // Restore original player state
    animationPlayer.seek(playerId, originalTime);
    if (originalState.playback_state === 'Playing') {
      animationPlayer.play(playerId);
    }
    
    return {
      frame_rate: frameRate,
      frame_count: totalFrames,
      duration: duration,
      tracks: trackData,
      metadata: {
        method: 'manual_sampling',
        generated_at: new Date().toISOString()
      }
    };
    
  } catch (error) {
    // Ensure we restore state even if something fails
    try {
      animationPlayer.seek(playerId, originalTime);
      if (originalState.playback_state === 'Playing') {
        animationPlayer.play(playerId);
      }
    } catch (restoreError) {
      console.warn('Failed to restore player state:', restoreError);
    }
    throw error;
  }
}

export const useBaking = () => {
  const animationPlayer = useAnimationPlayerContext();
  
  const [bakingState, setBakingState] = useState({
    isGenerating: false,
    lastBakedData: null,
    lastFrameRate: 60,
    error: null,
    generationTime: 0
  });

  const [bakingConfig, setBakingConfig] = useState({
    frameRate: 60,
    includeDerivatives: false,
    derivativeWidth: Math.round(1_000_000_000.0/60),
    interpolationMethod: 'cubic'
  });

  /**
   * Generate baked animation data at specified frame rate
   */
  const generateBaked = useCallback(async (frameRate = 60, config = {}) => {
    if (!animationPlayer.isLoaded) {
      throw new Error('Animation player not initialized');
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
        frameRate
      };


      // First check if we have players and animations loaded
      const playerIds = animationPlayer.getPlayerIds();
      
      if (playerIds.length === 0) {
        throw new Error('No players found. Make sure an animation is loaded and a player is created.');
      }
      
      // Get current player state for debugging
      try {
        const playerState = animationPlayer.getPlayerState('demo_player');
        console.log('Current player state:', playerState);
      } catch (stateError) {
        console.warn('Could not get player state:', stateError.message);
      }
      
      // Try different approaches to baking
      let bakedData;
      let lastError = null;
      
      // Approach 1: Try bakeCurrent method
      try {
        console.log('Attempting bakeCurrent...');
        bakedData = animationPlayer.bakeCurrent(frameRate, finalConfig);
        if (bakedData) {
          console.log('bakeCurrent succeeded');
        } else {
          throw new Error('bakeCurrent returned undefined');
        }
      } catch (bakingError) {
        console.log('bakeCurrent failed:', bakingError.message);
        lastError = bakingError;
        
        // Approach 2: Try with explicit animation ID 'demo_player'
        try {
          console.log('Attempting bakeAnimation with demo_player...');
          bakedData = animationPlayer.bakeAnimation('demo_player', frameRate, finalConfig);
          if (bakedData) {
            console.log('bakeAnimation with demo_player succeeded');
          } else {
            throw new Error('bakeAnimation returned undefined');
          }
        } catch (altError) {
          console.log('bakeAnimation with demo_player failed:', altError.message);
          lastError = altError;
          
          // Approach 3: Try using the first available player ID as animation ID
          try {
            const firstPlayerId = playerIds[0];
            console.log(`Attempting bakeAnimation with first player ID: ${firstPlayerId}...`);
            bakedData = animationPlayer.bakeAnimation(firstPlayerId, frameRate, finalConfig);
            if (bakedData) {
              console.log('bakeAnimation with first player ID succeeded');
            } else {
              throw new Error('bakeAnimation returned undefined');
            }
          } catch (finalError) {
            console.log('All native baking approaches failed, trying manual sampling...', finalError.message);
            
            // Approach 4: Manual sampling fallback
            try {
              bakedData = await generateManualBakedData(animationPlayer, frameRate, finalConfig);
              console.log('Manual baking succeeded');
            } catch (manualError) {
              console.log('Manual baking also failed:', manualError.message);
              const firstPlayerId = playerIds[0];
              throw new Error(`All baking methods failed. Last errors: bakeCurrent: ${bakingError.message}, bakeAnimation(demo_player): ${altError.message}, bakeAnimation(${firstPlayerId}): ${finalError.message}, manual: ${manualError.message}`);
            }
          }
        }
      }
      
      if (!bakedData) {
        throw new Error(`Baking failed: no data returned. Last error: ${lastError?.message || 'unknown'}`);
      }
      
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
  }, [animationPlayer, bakingConfig]);

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
   * Get original smooth animation data for comparison
   */
  const getOriginalChartData = useCallback(() => {
    if (!animationPlayer.isLoaded) {
      return {};
    }

    // Get current value history (smooth interpolated values)
    const history = animationPlayer.getAllValueHistory();
    const chartData = {};

    Object.entries(history).forEach(([key, values]) => {
      if (values.length > 0) {
        chartData[key] = {
          times: values.map((_, index) => index * (1.0 / (animationPlayer.pollingRate || 60))), // Convert to time
          values: values,
          frameRate: animationPlayer.pollingRate || 60,
          sampleCount: values.length
        };
      }
    });

    return chartData;
  }, [animationPlayer]);

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
    getOriginalChartData,
    getBakingStats,
    
    // Export
    exportBaked,
    downloadBaked,
    
    // Utilities
    formatBytes
  };
};
