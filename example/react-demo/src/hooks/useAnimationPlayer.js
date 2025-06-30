import { useRef, useEffect, useState, useCallback } from 'react';
import { useWasm } from './useWasm.js';

/**
 * Enhanced animation player hook that provides additional functionality on top of WASM:
 * - History tracking and time series data
 * - Performance monitoring and metrics
 * - Baking utilities
 * - Animation frame coordination
 * - Value change detection
 */
export function useAnimationPlayer(wasmConfig = null, options = {}) {
  const wasm = useWasm(wasmConfig);
  
  // Animation frame management
  const animationFrameRef = useRef(null);
  const lastUpdateTimeRef = useRef(0);
  const lastFrameTimeRef = useRef(0);
  
  // Performance tracking
  const [metrics, setMetrics] = useState({
    fps: 0,
    frameTimeMs: 0,
    memoryUsageMb: 0,
    activeAnimations: 0,
    totalUpdates: 0,
    lastUpdateTime: 0
  });
  
  const frameTimeHistory = useRef([]);
  const maxFrameHistory = options.maxFrameHistory || 60;
  
  // Value change tracking
  const lastValuesRef = useRef({});
  const lastDerivativesRef = useRef({});
  
  // Time series history tracking
  const [valueHistory, setValueHistory] = useState({});
  const [derivativeHistory, setDerivativeHistory] = useState({});
  const [historyMetadata, setHistoryMetadata] = useState({
    timestamps: [],
    captureCount: 0,
    startTime: null,
    lastCaptureTime: null
  });
  
  // History configuration
  const historyOptions = useRef({
    enabled: true,
    maxLength: 1000,
    captureInterval: 1,
    includeTimestamps: true,
    ...options.history
  });
  
  const captureCounter = useRef(0);

  // Calculate frame delta from last update
  const calculateFrameDelta = useCallback(() => {
    const now = performance.now();
    if (lastFrameTimeRef.current === 0) {
      lastFrameTimeRef.current = now;
      return 1.0 / 60.0; // Default to 60 FPS for first frame
    }
    const delta = (now - lastFrameTimeRef.current) / 1000.0;
    lastFrameTimeRef.current = now;
    return Math.min(delta, 1.0 / 30.0); // Cap at 30 FPS minimum
  }, []);

  // Update performance metrics
  const updateMetrics = useCallback(() => {
    const now = performance.now();

    if (lastUpdateTimeRef.current > 0) {
      const frameTime = now - lastUpdateTimeRef.current;
      frameTimeHistory.current.push(frameTime);
      
      if (frameTimeHistory.current.length > maxFrameHistory) {
        frameTimeHistory.current.shift();
      }
      
      // Calculate average frame time and FPS
      const avgFrameTime = frameTimeHistory.current.reduce((a, b) => a + b, 0) / frameTimeHistory.current.length;
      
      setMetrics(prev => ({
        ...prev,
        frameTimeMs: avgFrameTime,
        fps: avgFrameTime > 0 ? 1000 / avgFrameTime : 0,
        totalUpdates: prev.totalUpdates + 1,
        lastUpdateTime: now
      }));
    }
    
    lastUpdateTimeRef.current = now;
    
    // Get engine stats
    try {
      if (wasm.isLoaded) {
        const engineStats = wasm.getMetrics();
        setMetrics(prev => ({
          ...prev,
          memoryUsageMb: engineStats.total_memory_mb || 0,
          activeAnimations: engineStats.playing_players || 0
        }));
      }
    } catch (error) {
      // Ignore errors in metrics collection
    }
  }, [wasm.isLoaded, wasm.getMetrics, maxFrameHistory]);

  // Capture current values for time series history
  const captureValueHistory = useCallback((values, timestamp) => {
    if (!historyOptions.current.enabled) return;
    
    // Check capture interval
    captureCounter.current++;
    if (captureCounter.current % historyOptions.current.captureInterval !== 0) {
      return;
    }
    
    // Initialize start time if needed
    setHistoryMetadata(prev => {
      if (prev.startTime === null) {
        return { ...prev, startTime: timestamp };
      }
      return prev;
    });

    const isMap = values instanceof Map;
    const entries = isMap ? Array.from(values.entries()) : Object.entries(values);
    
    const newValues = { };
    
    for (const [playerId, playerValues] of entries) {
      const playerIsMap = playerValues instanceof Map;
      const playerEntries = playerIsMap ? Array.from(playerValues.entries()) : Object.entries(playerValues);
      
      Object.entries(playerEntries).forEach(([ind, trackEntries]) => {
        const [key, wrappedValue] = trackEntries;
        
        let numericValue;
        // Handle different value types (Float, Int, Bool, etc.)
        if (typeof wrappedValue === 'object' && wrappedValue !== null) {
          if ('Float' in wrappedValue) {
            numericValue = wrappedValue.Float;
          } else if ('Int' in wrappedValue) {
            numericValue = wrappedValue.Int;
          } else if ('Bool' in wrappedValue) {
            numericValue = wrappedValue.Bool ? 1 : 0;
          } else {
            // Unknown format, try to extract first value
            const firstValue = Object.values(wrappedValue)[0];
            numericValue = typeof firstValue === 'number' ? firstValue : 0;
          }
        } else {
          // Direct numeric value
          numericValue = typeof wrappedValue === 'number' ? wrappedValue : 0;
        }
        
        // Add value to history
        newValues[key] = numericValue;
        
        });
    }
    setValueHistory(prev => {
        const newHistory = { ...prev };
        Object.keys(newValues).forEach((k) => {
            const v = newValues[k];
            if (k in newHistory) {
                newHistory[k] = [...newHistory[k], v];
            } else {
                newHistory[k] = [v];
            }

            // Maintain max length limit
            if (newHistory[k].length > historyOptions.current.maxLength) {
                console.log("Slicing")
                newHistory[k] = newHistory[k].slice(1);
            }
        });
        return newHistory;
    });
    
    // Update metadata
    if (historyOptions.current.includeTimestamps) {
      setHistoryMetadata(prev => {
        const newTimestamps = [...prev.timestamps, timestamp];
        
        // Maintain timestamp array length
        if (newTimestamps.length > historyOptions.current.maxLength) {
          newTimestamps.shift();
        }
        
        return {
          ...prev,
          timestamps: newTimestamps,
          captureCount: prev.captureCount + 1,
          lastCaptureTime: timestamp
        };
      });
    }
  }, [valueHistory]);

  // Capture current derivatives for time series history
  const captureDerivativeHistory = useCallback((derivatives, timestamp) => {
    if (!historyOptions.current.enabled || !derivatives) return;
    
    // Check capture interval
    if (captureCounter.current % historyOptions.current.captureInterval !== 0) {
      return;
    }
    
    const isMap = derivatives instanceof Map;
    const entries = isMap ? Array.from(derivatives.entries()) : Object.entries(derivatives);
    
    const newDerivatives = {};
    
    for (const [key, wrappedValue] of entries) {
      let numericValue;
      
      // Handle different value types (Float, Int, Bool, etc.)
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
      
      newDerivatives[key] = numericValue;
    }
    
    setDerivativeHistory(prev => {
      const newDerivativeHistory = { ...prev };
      Object.keys(newDerivatives).forEach((k) => {
        const v = newDerivatives[k];
        if (k in newDerivativeHistory) {
          newDerivativeHistory[k] = [...newDerivativeHistory[k], v];
        } else {
          newDerivativeHistory[k] = [v];
        }

        // Maintain max length limit
        if (newDerivativeHistory[k].length > historyOptions.current.maxLength) {
          newDerivativeHistory[k] = newDerivativeHistory[k].slice(1);
        }
      });
      return newDerivativeHistory;
    });
  }, []);

  // Enhanced update function with metrics and history tracking
  const enhancedUpdate = useCallback((frameDelta) => {
    if (!wasm.isLoaded) return {};

    try {
      const delta = frameDelta !== undefined ? frameDelta : calculateFrameDelta();
      const values = wasm.update(delta);
      const now = performance.now();
      
      updateMetrics();
      
      // Check for value changes
      const currentValuesStr = JSON.stringify(values);
      const lastValuesStr = JSON.stringify(lastValuesRef.current);
      const hasChanged = currentValuesStr !== lastValuesStr;
      
      if (hasChanged) {
        lastValuesRef.current = values;
      }
      
      // Capture history
      captureValueHistory(values, now);
      
      // Get derivatives if we have players
      const playerIds = wasm.getPlayerIds();
      if (playerIds.length > 0) {
        try {
          const derivativeWidth = delta * 1000.0; // Convert delta to milliseconds
          const derivatives = wasm.getDerivatives(playerIds[0], derivativeWidth);
          captureDerivativeHistory(derivatives, now);
        } catch (error) {
          // Silently ignore derivative errors
        }
      }
      
      return {
        values,
        hasChanged,
        timestamp: now,
        frameInfo: playerIds.length > 0 ? {
          currentTime: wasm.getPlayerTime(playerIds[0]),
          playerState: wasm.getPlayerState(playerIds[0])
        } : null
      };
      
    } catch (error) {
      console.error('Enhanced update failed:', error);
      return {};
    }
  }, [wasm.isLoaded, wasm.update, wasm.getPlayerIds, wasm.getPlayerTime, wasm.getPlayerState, wasm.getDerivatives, calculateFrameDelta, updateMetrics, captureValueHistory, captureDerivativeHistory]);

  // Animation frame coordination
  const startAnimationFrame = useCallback((callback) => {
    if (animationFrameRef.current) {
      cancelAnimationFrame(animationFrameRef.current);
    }

    const animate = () => {
      const updateResult = enhancedUpdate();
      if (callback && updateResult.values) {
        callback(updateResult);
      }
      animationFrameRef.current = requestAnimationFrame(animate);
    };

    animationFrameRef.current = requestAnimationFrame(animate);
    
    return () => {
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
        animationFrameRef.current = null;
      }
    };
  }, [enhancedUpdate]);

  const stopAnimationFrame = useCallback(() => {
    if (animationFrameRef.current) {
      cancelAnimationFrame(animationFrameRef.current);
      animationFrameRef.current = null;
    }
  }, []);

  // Baking utilities
  const bakeAnimationWithConfig = useCallback((animationId, frameRate = 60, config = {}) => {
    if (!wasm.isLoaded) throw new Error('WASM not loaded');

    const bakingConfig = {
      frame_rate: frameRate,
      include_disabled_tracks: config.includeDisabledTracks || false,
      apply_track_weights: config.applyTrackWeights !== false,
      time_range: config.timeRange || null,
      interpolation_method: config.interpolationMethod || 'linear',
      include_derivatives: config.includeDerivatives || true,
      derivative_width: config.derivativeWidth || 1000000000.0 / frameRate,
    };

    const bakedData = wasm.bakeAnimation(animationId, JSON.stringify(bakingConfig));
    
    // Convert duration from nanoseconds to seconds
    if (bakedData.duration) {
      bakedData.duration = bakedData.duration / 1000000000;
    }
    
    return bakedData;
  }, [wasm.isLoaded, wasm.bakeAnimation]);

  // History management functions
  const getValueHistory = useCallback((keyName) => {
    return valueHistory[keyName] ? [...valueHistory[keyName]] : [];
  }, [valueHistory]);

  const getAllValueHistory = useCallback(() => {
    const result = {};
    Object.entries(valueHistory).forEach(([key, values]) => {
      result[key] = [...values];
    });
    return result;
  }, [valueHistory]);

  const getDerivativeHistory = useCallback((keyName) => {
    return derivativeHistory[keyName] ? [...derivativeHistory[keyName]] : [];
  }, [derivativeHistory]);

  const getAllDerivativeHistory = useCallback(() => {
    const result = {};
    Object.entries(derivativeHistory).forEach(([key, values]) => {
      result[key] = [...values];
    });
    return result;
  }, [derivativeHistory]);

  const clearHistory = useCallback(() => {
    setValueHistory({});
    setDerivativeHistory({});
    setHistoryMetadata({
      timestamps: [],
      captureCount: 0,
      startTime: null,
      lastCaptureTime: null
    });
    captureCounter.current = 0;
  }, []);

  const setHistoryOptions = useCallback((newOptions) => {
    historyOptions.current = {
      ...historyOptions.current,
      ...newOptions
    };
  }, []);

  const getHistoryStats = useCallback(() => {
    const keys = Object.keys(valueHistory);
    const totalKeys = keys.length;
    const totalCaptures = historyMetadata.captureCount;
    
    const totalValues = keys.reduce((sum, key) => sum + valueHistory[key].length, 0);
    const averageValuesPerKey = totalKeys > 0 ? totalValues / totalKeys : 0;
    
    const estimatedBytesPerValue = 8;
    const timestampBytes = historyMetadata.timestamps.length * 8;
    const memoryUsageEstimate = (totalValues * estimatedBytesPerValue) + timestampBytes;
    
    let captureRate = 0;
    if (historyMetadata.startTime && historyMetadata.lastCaptureTime) {
      const duration = (historyMetadata.lastCaptureTime - historyMetadata.startTime) / 1000;
      captureRate = duration > 0 ? totalCaptures / duration : 0;
    }
    
    return {
      totalKeys,
      totalCaptures,
      totalValues,
      averageValuesPerKey,
      memoryUsageEstimate,
      captureRate,
      oldestCaptureTime: historyMetadata.startTime,
      newestCaptureTime: historyMetadata.lastCaptureTime,
      captureInterval: historyOptions.current.captureInterval,
      maxLength: historyOptions.current.maxLength
    };
  }, [valueHistory, historyMetadata]);

  // Cleanup
  useEffect(() => {
    return () => {
      stopAnimationFrame();
    };
  }, [stopAnimationFrame]);

  return {
    // WASM state
    ...wasm,
    
    // Enhanced functionality
    metrics,
    enhancedUpdate,
    startAnimationFrame,
    stopAnimationFrame,
    
    // Baking
    bakeAnimationWithConfig,
    
    // History
    valueHistory,
    derivativeHistory,
    historyMetadata,
    getValueHistory,
    getAllValueHistory,
    getDerivativeHistory,
    getAllDerivativeHistory,
    clearHistory,
    setHistoryOptions,
    getHistoryStats,
  };
}
