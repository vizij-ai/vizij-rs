import React, { createContext, useContext, useEffect, useState, useCallback, useRef } from 'react';
import { useAnimationPlayer } from '../hooks/useAnimationPlayer.js';

const AnimationPlayerContext = createContext(null);

export const useAnimationPlayerContext = () => {
  const context = useContext(AnimationPlayerContext);
  if (!context) {
    throw new Error('useAnimationPlayerContext must be used within an AnimationPlayerProvider');
  }
  return context;
};

export const AnimationPlayerProvider = ({ children, wasmConfig = null, options = {} }) => {
  const animationPlayer = useAnimationPlayer(wasmConfig, options);
  
  // Polling/streaming state
  const pollingIntervalRef = useRef(null);
  const [isPolling, setIsPolling] = useState(false);
  const [pollingRate, setPollingRate] = useState(options.defaultPollingRate || 30);
  
  // Animation state
  const [currentValues, setCurrentValues] = useState({});
  const [currentTime, setCurrentTime] = useState(0);
  const [playerState, setPlayerState] = useState('stopped');
  const [isPlaying, setIsPlaying] = useState(false);
  const [progress, setProgress] = useState(0);
  
  // Configuration state
  const [config, setConfig] = useState({
    speed: 1.0,
    mode: 'loop',
    startTime: 0,
    endTime: null,
    ...options.defaultConfig
  });
  
  // UI state
  const [logs, setLogs] = useState([]);
  const [activePlayerId, setActivePlayerId] = useState('demo_player');
  
  // Add log entry
  const addLog = useCallback((message, type = 'info') => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [...prev.slice(-49), { timestamp, message, type }]);
  }, []);

  // Clear logs
  const clearLogs = useCallback(() => {
    setLogs([]);
  }, []);

  // Initialize with test animation when WASM is loaded
  useEffect(() => {
    if (animationPlayer.isLoaded && !animationPlayer.isLoading && !animationPlayer.error) {
      const initializeTestAnimation = async () => {
        try {
          addLog('Initializing test animation...');
          
          // Get test animation data
          const testAnimationData = animationPlayer.getTestAnimationData();
          
          // Load animation
          const animationJson = typeof testAnimationData === 'string' 
            ? testAnimationData 
            : JSON.stringify(testAnimationData);
          animationPlayer.loadAnimation(animationJson);
          
          // Create player
          animationPlayer.createPlayer(activePlayerId);
          
          // Extract animation ID
          const animData = typeof testAnimationData === 'object' ? testAnimationData : JSON.parse(testAnimationData);
          const animationId = animData.id || 'test_animation';
          
          // Add instance
          animationPlayer.addInstance(activePlayerId, animationId);
          
          addLog('Test animation loaded successfully');
          
          // Get initial values
          const initialUpdate = animationPlayer.enhancedUpdate(0);
          if (initialUpdate.values) {
            setCurrentValues(initialUpdate.values);
          }
          
          // Set initial state
          updatePlayerStateFromWasm();
          
        } catch (error) {
          addLog(`Failed to initialize test animation: ${error.message}`, 'error');
        }
      };
      
      initializeTestAnimation();
    }
  }, [animationPlayer.isLoaded, animationPlayer.isLoading, animationPlayer.error, activePlayerId]);

  // Update player state from WASM
  const updatePlayerStateFromWasm = useCallback(() => {
    if (!animationPlayer.isLoaded) return;
    
    try {
      const playerIds = animationPlayer.getPlayerIds();
      if (playerIds.length === 0) return;
      
      const playerId = playerIds.includes(activePlayerId) ? activePlayerId : playerIds[0];
      
      const currentPlayerTime = animationPlayer.getPlayerTime(playerId);
      const currentPlayerState = animationPlayer.getPlayerState(playerId);
      const currentPlayerProgress = animationPlayer.getPlayerProgress(playerId);
      
      setCurrentTime(currentPlayerTime);
      setPlayerState(currentPlayerState.playback_state);
      setIsPlaying(currentPlayerState.playback_state === 'Playing');
      setProgress(currentPlayerProgress);
      
    } catch (error) {
      // Silently handle errors in state updates
    }
  }, [animationPlayer.isLoaded, animationPlayer.getPlayerIds, animationPlayer.getPlayerTime, 
      animationPlayer.getPlayerState, animationPlayer.getPlayerProgress, activePlayerId]);

  // Polling functions
  const startPolling = useCallback(() => {
    if (pollingIntervalRef.current || !animationPlayer.isLoaded) return;
    
    setIsPolling(true);
    const intervalMs = 1000 / pollingRate;
    
    pollingIntervalRef.current = setInterval(() => {
      try {
        const updateResult = animationPlayer.enhancedUpdate();
        
        if (updateResult.values) {
          setCurrentValues(updateResult.values);
        }
        
        updatePlayerStateFromWasm();
        
      } catch (error) {
        addLog(`Polling update failed: ${error.message}`, 'error');
      }
    }, intervalMs);
    
    addLog(`Started polling at ${pollingRate} FPS`);
  }, [animationPlayer.isLoaded, animationPlayer.enhancedUpdate, pollingRate, updatePlayerStateFromWasm, addLog]);

  const stopPolling = useCallback(() => {
    if (pollingIntervalRef.current) {
      clearInterval(pollingIntervalRef.current);
      pollingIntervalRef.current = null;
      setIsPolling(false);
      addLog('Stopped polling');
    }
  }, [addLog]);

  // Update polling rate
  const updatePollingRate = useCallback((newRate) => {
    const wasPolling = isPolling;
    if (wasPolling) {
      stopPolling();
    }
    setPollingRate(newRate);
    if (wasPolling) {
      // Restart with new rate after a brief delay
      setTimeout(() => startPolling(), 100);
    }
  }, [isPolling, stopPolling, startPolling]);

  // Playback control methods
  const play = useCallback(() => {
    if (!animationPlayer.isLoaded) return;
    
    try {
      animationPlayer.play(activePlayerId);
      startPolling(); // Auto-start polling when playing
      addLog(`Started playback for ${activePlayerId}`);
    } catch (error) {
      addLog(`Play failed: ${error.message}`, 'error');
    }
  }, [animationPlayer.isLoaded, animationPlayer.play, activePlayerId, startPolling, addLog]);

  const pause = useCallback(() => {
    if (!animationPlayer.isLoaded) return;
    
    try {
      animationPlayer.pause(activePlayerId);
      stopPolling(); // Auto-stop polling when pausing
      addLog(`Paused playback for ${activePlayerId}`);
    } catch (error) {
      addLog(`Pause failed: ${error.message}`, 'error');
    }
  }, [animationPlayer.isLoaded, animationPlayer.pause, activePlayerId, stopPolling, addLog]);

  const stop = useCallback(() => {
    if (!animationPlayer.isLoaded) return;
    
    try {
      animationPlayer.stop(activePlayerId);
      stopPolling(); // Auto-stop polling when stopping
      setCurrentTime(0);
      addLog(`Stopped playback for ${activePlayerId}`);
    } catch (error) {
      addLog(`Stop failed: ${error.message}`, 'error');
    }
  }, [animationPlayer.isLoaded, animationPlayer.stop, activePlayerId, stopPolling, addLog]);

  const seek = useCallback((time) => {
    if (!animationPlayer.isLoaded) return;
    
    try {
      animationPlayer.seek(activePlayerId, time);
      setCurrentTime(time);
      
      // Update values at the new time position
      const updateResult = animationPlayer.enhancedUpdate(0);
      if (updateResult.values) {
        setCurrentValues(updateResult.values);
      }
      
      updatePlayerStateFromWasm();
      addLog(`Seeked to ${time.toFixed(2)}s`);
    } catch (error) {
      addLog(`Seek failed: ${error.message}`, 'error');
    }
  }, [animationPlayer.isLoaded, animationPlayer.seek, animationPlayer.enhancedUpdate, 
      activePlayerId, updatePlayerStateFromWasm, addLog]);

  // Configuration update methods
  const updatePlayerConfig = useCallback((newConfig) => {
    if (!animationPlayer.isLoaded) return;
    
    try {
      setConfig(prev => ({ ...prev, ...newConfig }));
      
      if (newConfig.speed !== undefined) {
        const configJson = JSON.stringify({ speed: newConfig.speed });
        animationPlayer.updatePlayerConfig(activePlayerId, configJson);
        addLog(`Speed set to ${newConfig.speed}x`);
      }
      
      if (newConfig.mode !== undefined) {
        const configJson = JSON.stringify({ mode: newConfig.mode });
        animationPlayer.updatePlayerConfig(activePlayerId, configJson);
        
        // For loop and ping_pong modes, ensure endTime is set if not specified
        if ((newConfig.mode === 'loop' || newConfig.mode === 'ping_pong') && config.endTime === null) {
          const animationDuration = 4.0; // Test animation duration
          const endTimeConfigJson = JSON.stringify({ end_time: animationDuration });
          animationPlayer.updatePlayerConfig(activePlayerId, endTimeConfigJson);
          setConfig(prev => ({ ...prev, endTime: animationDuration }));
          addLog(`Auto-set end time to ${animationDuration}s for ${newConfig.mode} mode`);
        }
        
        addLog(`Playback mode set to ${newConfig.mode}`);
      }
      
      if (newConfig.startTime !== undefined) {
        const configJson = JSON.stringify({ start_time: newConfig.startTime });
        animationPlayer.updatePlayerConfig(activePlayerId, configJson);
        addLog(`Start time set to ${newConfig.startTime}s`);
      }
      
      if (newConfig.endTime !== undefined) {
        const endTime = newConfig.endTime === null ? null : newConfig.endTime;
        const configJson = JSON.stringify({ end_time: endTime });
        animationPlayer.updatePlayerConfig(activePlayerId, configJson);
        if (endTime === null) {
          addLog('End time cleared');
        } else {
          addLog(`End time set to ${endTime}s`);
        }
      }
      
    } catch (error) {
      addLog(`Config update failed: ${error.message}`, 'error');
    }
  }, [animationPlayer.isLoaded, animationPlayer.updatePlayerConfig, activePlayerId, config.endTime, addLog]);

  // Convenience methods for common config updates
  const setSpeed = useCallback((speed) => {
    updatePlayerConfig({ speed });
  }, [updatePlayerConfig]);

  const setPlaybackMode = useCallback((mode) => {
    updatePlayerConfig({ mode });
  }, [updatePlayerConfig]);

  const setStartTime = useCallback((startTime) => {
    updatePlayerConfig({ startTime });
  }, [updatePlayerConfig]);

  const setEndTime = useCallback((endTime) => {
    updatePlayerConfig({ endTime });
  }, [updatePlayerConfig]);

  const setTimeRange = useCallback((startTime, endTime) => {
    updatePlayerConfig({ startTime, endTime });
  }, [updatePlayerConfig]);

  // Manual update
  const manualUpdate = useCallback(() => {
    if (!animationPlayer.isLoaded) return;
    
    try {
      const updateResult = animationPlayer.enhancedUpdate();
      if (updateResult.values) {
        setCurrentValues(updateResult.values);
      }
      updatePlayerStateFromWasm();
      addLog('Manual update performed');
    } catch (error) {
      addLog(`Manual update failed: ${error.message}`, 'error');
    }
  }, [animationPlayer.isLoaded, animationPlayer.enhancedUpdate, updatePlayerStateFromWasm, addLog]);

  // Load animation from data
  const loadAnimationFromData = useCallback(async (animationData, playerName = 'custom_player') => {
    if (!animationPlayer.isLoaded) {
      throw new Error('Animation player not initialized');
    }
    
    try {
      addLog('Loading animation from provided data...');
      
      // Stop current playback if playing
      if (isPlaying) {
        stop();
      }
      
      // Load the new animation data
      const animationJson = typeof animationData === 'string' 
        ? animationData 
        : JSON.stringify(animationData);
      animationPlayer.loadAnimation(animationJson);
      
      // Create new player
      animationPlayer.createPlayer(playerName);
      
      // Extract animation ID
      const animData = typeof animationData === 'object' ? animationData : JSON.parse(animationData);
      const animationId = animData.id || Object.keys(animData)[0] || 'unknown';
      
      // Add instance
      animationPlayer.addInstance(playerName, animationId);
      
      // Update active player
      setActivePlayerId(playerName);
      
      // Reset state
      setCurrentTime(0);
      setCurrentValues({});
      setIsPlaying(false);
      setPlayerState('stopped');
      setProgress(0);
      
      // Update values at time 0
      const updateResult = animationPlayer.enhancedUpdate(0);
      if (updateResult.values) {
        setCurrentValues(updateResult.values);
      }
      
      updatePlayerStateFromWasm();
      addLog('Animation data loaded successfully');
      
    } catch (error) {
      addLog(`Failed to load animation: ${error.message}`, 'error');
      throw error;
    }
  }, [animationPlayer.isLoaded, animationPlayer.loadAnimation, animationPlayer.createPlayer, 
      animationPlayer.addInstance, animationPlayer.enhancedUpdate, isPlaying, stop, 
      updatePlayerStateFromWasm, addLog]);

  // Baking utilities
  const bakeAnimation = useCallback((animationId, frameRate = 60, config = {}) => {
    if (!animationPlayer.isLoaded) {
      throw new Error('Animation player not initialized');
    }
    
    try {
      addLog(`Baking animation '${animationId}' at ${frameRate} FPS...`);
      const bakedData = animationPlayer.bakeAnimationWithConfig(animationId, frameRate, config);
      addLog(`Animation baked successfully: ${bakedData.frame_count || 0} frames`);
      return bakedData;
    } catch (error) {
      addLog(`Baking failed: ${error.message}`, 'error');
      throw error;
    }
  }, [animationPlayer.isLoaded, animationPlayer.bakeAnimationWithConfig, addLog]);

  const bakeCurrent = useCallback((frameRate = 60, config = {}) => {
    return bakeAnimation('test_animation', frameRate, config);
  }, [bakeAnimation]);

  // Get frame info for display
  const getFrameInfo = useCallback(() => {
    if (!animationPlayer.isLoaded) {
      return {
        currentTime: 0,
        duration: 0,
        playing: false,
        progress: 0,
        playbackRate: 1.0
      };
    }

    const playerIds = animationPlayer.getPlayerIds();
    if (playerIds.length === 0) {
      return {
        currentTime: 0,
        duration: 0,
        playing: false,
        progress: 0,
        playbackRate: 1.0
      };
    }

    const playerId = playerIds.includes(activePlayerId) ? activePlayerId : playerIds[0];
    
    try {
      const state = animationPlayer.getPlayerState(playerId);
      return {
        currentTime: currentTime,
        duration: state.end_time || 4.0, // Default from test animation
        playing: isPlaying,
        progress: progress,
        playbackRate: state.speed || 1.0
      };
    } catch (error) {
      return {
        currentTime: 0,
        duration: 0,
        playing: false,
        progress: 0,
        playbackRate: 1.0
      };
    }
  }, [animationPlayer.isLoaded, animationPlayer.getPlayerIds, animationPlayer.getPlayerState, 
      activePlayerId, currentTime, isPlaying, progress]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      stopPolling();
    };
  }, [stopPolling]);

  // Context value
  const value = {
    // WASM and enhanced functionality
    ...animationPlayer,
    
    // React state
    currentValues,
    currentTime,
    playerState,
    isPlaying,
    progress,
    config,
    logs,
    activePlayerId,
    
    // Polling state
    isPolling,
    pollingRate,
    
    // Control methods
    play,
    pause,
    stop,
    seek,
    manualUpdate,
    
    // Configuration
    updatePlayerConfig,
    setSpeed,
    setPlaybackMode,
    setStartTime,
    setEndTime,
    setTimeRange,
    
    // Polling control
    startPolling,
    stopPolling,
    updatePollingRate,
    
    // Animation loading
    loadAnimationFromData,
    
    // Baking
    bakeAnimation,
    bakeCurrent,
    
    // Utility
    addLog,
    clearLogs,
    getFrameInfo,
    setActivePlayerId,
  };

  return (
    <AnimationPlayerContext.Provider value={value}>
      {children}
    </AnimationPlayerContext.Provider>
  );
};

export default AnimationPlayerProvider;
