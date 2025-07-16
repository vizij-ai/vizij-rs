import { useRef, useEffect, useState, useCallback } from 'react';
import wasminit, { WasmAnimationEngine, create_test_animation } from 'animation-player';

/**
 * Pure WASM wrapper hook that loads the module and provides direct access to engine methods
 * without modifying them. Handles memory management and cleanup.
 */
export function useWasm(config = null) {
  const engineRef = useRef(null);
  const [isLoaded, setIsLoaded] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState(null);

  // Initialize WASM module
  useEffect(() => {
    let mounted = true;

    const initWasm = async () => {
      if (engineRef.current) return; // Already initialized

      setIsLoading(true);
      setError(null);

      try {
        // Initialize WASM module
        await wasminit();

        if (!mounted) return;

        // Create engine instance
        const configJson = config ? JSON.stringify(config) : null;
        const engine = new WasmAnimationEngine(configJson);
      
        engineRef.current = engine;
        setIsLoaded(true);

      } catch (err) {
        if (mounted) {
          setError(err.message);
          console.error('WASM initialization failed:', err);
        }
      } finally {
        if (mounted) {
          setIsLoading(false);
        }
      }
    };

    initWasm();

    // Cleanup function
    return () => {
      mounted = false;
      if (engineRef.current) {
        // WASM modules don't have explicit cleanup, but we clear the reference
        engineRef.current = null;
      }
    };
  }, [config]);

  // Direct WASM method wrappers - these don't modify the underlying methods
  const loadAnimation = useCallback((animationJson) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.load_animation(animationJson);
  }, []);

  const unloadAnimation = useCallback((animationId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.unload_animation(animationId);
  }, []);

  const createPlayer = useCallback((playerId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.create_player(playerId);
  }, []);

  const removePlayer = useCallback((playerId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.remove_player(playerId);
  }, []);

  const addInstance = useCallback((playerId, animationId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.add_instance(playerId, animationId);
  }, []);

  const removeInstance = useCallback((playerId, instanceId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.remove_instance(playerId, instanceId);
  }, []);

  const play = useCallback((playerId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.play(playerId);
  }, []);

  const pause = useCallback((playerId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.pause(playerId);
  }, []);

  const stop = useCallback((playerId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.stop(playerId);
  }, []);

  const seek = useCallback((playerId, timeSeconds) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.seek(playerId, timeSeconds);
  }, []);

  const update = useCallback((frameDeltaSeconds) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.update(frameDeltaSeconds);
  }, []);

  const getPlayerSettings = useCallback((playerId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.get_player_settings(playerId);
  }, []);

  const getInstanceSettings = useCallback((playerId, instanceId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.get_instance_config(playerId, instanceId);
  }, []);

  const getPlayerState = useCallback((playerId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.get_player_state(playerId);
  }, []);

  const getPlayerTime = useCallback((playerId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.get_player_time(playerId);
  }, []);

  const getPlayerDuration = useCallback((playerId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.get_player_duration(playerId);
  }, []);

  const getAnimationIds = useCallback(() => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.animation_ids();
  }, []);

  const getPlayerProgress = useCallback((playerId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.get_player_progress(playerId);
  }, []);

  const getPlayerIds = useCallback(() => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.get_player_ids();
  }, []);

  const updatePlayerConfig = useCallback((playerId, configJson) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.update_player_config(playerId, configJson);
  }, []);

  const updateInstanceConfig = useCallback((playerId, instanceId, configJson) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.update_instance_config(playerId, instanceId, configJson);
  }, []);

  const exportAnimation = useCallback((animationId) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    const animationJson = engineRef.current.export_animation(animationId);
    return JSON.parse(animationJson);
  }, []);

  const bakeAnimation = useCallback((animationId, configJson) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    const bakedJson = engineRef.current.bake_animation(animationId, configJson);
    return JSON.parse(bakedJson);
  }, []);

  const getDerivatives = useCallback((playerId, derivativeWidthMs = 1.0) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.get_derivatives(playerId, derivativeWidthMs);
  }, []);

  const setEngineConfig = useCallback((config) => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    const json = typeof config === 'string' ? config : JSON.stringify(config);
    engineRef.current.set_engine_config(json);
  }, []);

  const getEngineConfig = useCallback(() => {
    if (!engineRef.current) throw new Error('WASM not loaded');
    return engineRef.current.get_engine_config();
  }, []);

  // Test animation helper
  const getTestAnimationData = useCallback(() => {
    return create_test_animation();
  }, []);

  return {
    // State
    isLoaded,
    isLoading,
    error,
    engine: engineRef.current,

    // Direct WASM methods
    loadAnimation,
    unloadAnimation,
    createPlayer,
    removePlayer,
    addInstance,
    removeInstance,
    play,
    pause,
    stop,
    seek,
    update,
    getPlayerState,
    getPlayerTime,
    getAnimationIds,
    getPlayerDuration,
    getPlayerProgress,
    getPlayerIds,
    getPlayerSettings,
    getInstanceSettings,
    updatePlayerConfig,
    updateInstanceConfig,
    exportAnimation,
    bakeAnimation,
    getDerivatives,
    setEngineConfig,
    getEngineConfig,
    getTestAnimationData,
  };
}
