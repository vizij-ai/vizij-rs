import React, { createContext, useContext, useEffect, useState, useCallback } from 'react';
import { AnimationPlayer, initWasm } from '../../utils/AnimationPlayer.js';

const AnimationPlayerContext = createContext(null);

export const useAnimationPlayer = () => {
  const context = useContext(AnimationPlayerContext);
  if (!context) {
    throw new Error('useAnimationPlayer must be used within an AnimationPlayerProvider');
  }
  return context;
};

export const AnimationPlayerProvider = ({ children }) => {
  const [player, setPlayer] = useState(null);
  const [wasmModule, setWasmModule] = useState(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState(null);
  const [isInitialized, setIsInitialized] = useState(false);

  // Player state
  const [currentValues, setCurrentValues] = useState({});
  const [playerState, setPlayerState] = useState('stopped');
  const [currentTime, setCurrentTime] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [metrics, setMetrics] = useState({
    fps: 0,
    frame_time_ms: 0,
    memory_usage_mb: 0,
    total_updates: 0
  });

  // Configuration state
  const [config, setConfig] = useState({
    speed: 1.0,
    mode: 'loop',
    startTime: 0,
    endTime: null,
    updateRate: 10
  });

  const [logs, setLogs] = useState([]);

  const addLog = useCallback((message, type = 'info') => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [...prev.slice(-49), { timestamp, message, type }]);
  }, []);

  // Initialize WASM and player
  useEffect(() => {
    let mounted = true;

    const initialize = async () => {
      try {
        setIsLoading(true);
        addLog('Loading WASM module...');


        await initWasm();
        
        if (!mounted) return;


        // Create player instance
        console.log("AnimationPlayer is", AnimationPlayer);
        const playerInstance = new AnimationPlayer({
          polling: {
            updateRate: config.updateRate,
            enabled: false
          },
          logging: {
            level: 'info',
            enableConsole: true
          }
        });

        setPlayer(playerInstance);
        addLog('AnimationPlayer created');

        // Load test animation
        const testAnimationData = playerInstance.getTestAnimationData();
        await playerInstance.loadAnimationWithPlayer('demo_player', testAnimationData);
        addLog('Test animation loaded');

        // Set up event listeners
        playerInstance.on('update', (update) => {
          if (!mounted) return;
          // console.log("JSX-Update call returned:", update, config)
          setCurrentValues(update.values);
          setCurrentTime(update.frame_info.current_time);
          setIsPlaying(update.frame_info.playing);
          setMetrics(playerInstance.getPlayerMetrics());
        });

        playerInstance.on('play', () => {
          if (!mounted) return;
          setPlayerState('playing');
          setIsPlaying(true);
          addLog('Playback started');
        });

        playerInstance.on('pause', () => {
          if (!mounted) return;
          setPlayerState('paused');
          setIsPlaying(false);
          addLog('Playback paused');
        });

        playerInstance.on('stop', () => {
          if (!mounted) return;
          setPlayerState('stopped');
          setIsPlaying(false);
          setCurrentTime(0);
          addLog('Playback stopped');
        });

        playerInstance.on('error', (errorData) => {
          if (!mounted) return;
          addLog(`Error: ${errorData.error}`, 'error');
        });

        setIsInitialized(true);
        setIsLoading(false);
        addLog('Animation player ready');

      } catch (err) {
        if (!mounted) return;
        console.error('Failed to initialize animation player:', err);
        setError(err.message);
        setIsLoading(false);
        addLog(`Initialization failed: ${err.message}`, 'error');
      }
    };

    initialize();

    return () => {
      mounted = false;
      if (player) {
        player.dispose();
      }
    };
  }, []);

  // Player control methods
  const play = useCallback(() => {
    if (player && isInitialized) {
      try {
        player.play('demo_player');
        // Automatically start polling when playing
        player.startPolling();
        addLog(`Started playback and auto-updates at ${config.updateRate} FPS`);
      } catch (err) {
        addLog(`Play failed: ${err.message}`, 'error');
      }
    }
  }, [player, isInitialized, config.updateRate]);

  const pause = useCallback(() => {
    if (player && isInitialized) {
      try {
        player.pause('demo_player');
        // Automatically stop polling when pausing
        player.stopPolling();
        addLog('Paused playback and stopped auto-updates');
      } catch (err) {
        addLog(`Pause failed: ${err.message}`, 'error');
      }
    }
  }, [player, isInitialized]);

  const stop = useCallback(() => {
    if (player && isInitialized) {
      try {
        player.stop('demo_player');
        // Automatically stop polling when stopping
        player.stopPolling();
        addLog('Stopped playback and auto-updates');
      } catch (err) {
        addLog(`Stop failed: ${err.message}`, 'error');
      }
    }
  }, [player, isInitialized]);

  const seek = useCallback((time) => {
    if (player && isInitialized) {
      try {
        player.seek('demo_player', time);
        setCurrentTime(time);
        
        // Update values at the new time position
        const values = player.update();
        setCurrentValues(values);
        setMetrics(player.getPlayerMetrics());
        
        addLog(`Seeked to ${time.toFixed(2)}s`);
      } catch (err) {
        addLog(`Seek failed: ${err.message}`, 'error');
      }
    }
  }, [player, isInitialized]);

  const updatePlayerConfig = useCallback((newConfig) => {
    if (player && isInitialized) {
      try {
        setConfig(prev => ({ ...prev, ...newConfig }));
        
        if (newConfig.speed !== undefined) {
          player.setSpeed('demo_player', newConfig.speed);
          addLog(`Speed set to ${newConfig.speed}x`);
        }
        
        if (newConfig.mode !== undefined) {
          console.log("Setting Player to use mode:", newConfig.mode)
          player.setPlaybackMode('demo_player', newConfig.mode);
          
          // For loop and ping_pong modes, ensure endTime is set to animation duration if not specified
          if ((newConfig.mode === 'loop' || newConfig.mode === 'ping_pong') && config.endTime === null) {
            const animationDuration = 4.0; // Test animation duration
            player.setEndTime('demo_player', animationDuration);
            setConfig(prev => ({ ...prev, endTime: animationDuration }));
            addLog(`Auto-set end time to ${animationDuration}s for ${newConfig.mode} mode`);
          }
          
          addLog(`Playback mode set to ${newConfig.mode}`);
        }
        
        if (newConfig.startTime !== undefined) {
          player.setStartTime('demo_player', newConfig.startTime);
          addLog(`Start time set to ${newConfig.startTime}s`);
        }
        
        if (newConfig.endTime !== undefined) {
          if (newConfig.endTime === null) {
            player.setEndTime('demo_player', null);
            addLog('End time cleared');
          } else {
            player.setEndTime('demo_player', newConfig.endTime);
            addLog(`End time set to ${newConfig.endTime}s`);
          }
        }

        if (newConfig.updateRate !== undefined) {
          player.setPollingRate(newConfig.updateRate);
          addLog(`Update rate set to ${newConfig.updateRate} FPS`);
        }
        
      } catch (err) {
        addLog(`Config update failed: ${err.message}`, 'error');
      }
    }
  }, [player, isInitialized]);

  const startPolling = useCallback(() => {
    if (player && isInitialized) {
      try {
        player.startPolling();
        addLog(`Started auto-updates at ${config.updateRate} FPS`);
      } catch (err) {
        addLog(`Failed to start polling: ${err.message}`, 'error');
      }
    }
  }, [player, isInitialized, config.updateRate]);

  const stopPolling = useCallback(() => {
    if (player && isInitialized) {
      try {
        player.stopPolling();
        addLog('Stopped auto-updates');
      } catch (err) {
        addLog(`Failed to stop polling: ${err.message}`, 'error');
      }
    }
  }, [player, isInitialized]);

  const manualUpdate = useCallback(() => {
    if (player && isInitialized) {
      try {
        const values = player.update();
        setCurrentValues(values);
        setMetrics(player.getPlayerMetrics());
        addLog('Manual update performed');
      } catch (err) {
        addLog(`Manual update failed: ${err.message}`, 'error');
      }
    }
  }, [player, isInitialized]);

  const clearLogs = useCallback(() => {
    setLogs([]);
  }, []);

  const loadAnimationFromData = useCallback(async (animationData, playerName = 'demo_player2') => {
    if (!player || !isInitialized) {
      throw new Error('Animation player not initialized');
    }
    
    try {
      addLog('Loading animation from provided data...');
      
      // Stop current playback if playing
      if (isPlaying) {
        player.stop('demo_player');
        player.stopPolling();
      }
      
      // Load the new animation data
      await player.loadAnimationWithPlayer(playerName, animationData);
      
      // Reset player state
      setCurrentTime(0);
      setCurrentValues({});
      setIsPlaying(false);
      setPlayerState('stopped');
      
      // Update values at time 0
      const values = player.update();
      setCurrentValues(values);
      setMetrics(player.getPlayerMetrics());
      
      addLog('Animation data loaded successfully');
    } catch (err) {
      addLog(`Failed to load animation: ${err.message}`, 'error');
      throw err;
    }
  }, [player, isInitialized, isPlaying, addLog]);

  const value = {
    // State
    player,
    wasmModule,
    isLoading,
    error,
    isInitialized,
    currentValues,
    playerState,
    currentTime,
    isPlaying,
    metrics,
    config,
    logs,

    // Actions
    play,
    pause,
    stop,
    seek,
    updatePlayerConfig,
    startPolling,
    stopPolling,
    manualUpdate,
    clearLogs,
    addLog,
    loadAnimationFromData
  };

  return (
    <AnimationPlayerContext.Provider value={value}>
      {children}
    </AnimationPlayerContext.Provider>
  );
};

export default AnimationPlayerProvider;
