import React, { createContext, useContext, useRef, useEffect, useState, useCallback } from 'react';
import { useWasm } from '../hooks/useWasm';

const AnimationEngineContext = createContext(null);

export const AnimationEngineProvider = ({ children }) => {
  const wasm = useWasm();
  const animationFrameRef = useRef(null);
  const lastFrameTimeRef = useRef(0);
  const [latestValues, setLatestValues] = useState({});
  const [playerIds, setPlayerIds] = useState([]);
  const [animationIds, setAnimationIds] = useState([]);

  const targetInterval = 100; // milliseconds (10 fps)

  const updateLoop = useCallback(() => {
    if (!wasm.isLoaded) {
      animationFrameRef.current = requestAnimationFrame(updateLoop);
      return;
    }

    const now = performance.now();
    if (lastFrameTimeRef.current === 0) {
      lastFrameTimeRef.current = now;
    }

    const delta = now - lastFrameTimeRef.current;

    if (delta >= targetInterval) {
      lastFrameTimeRef.current = now;

      // Convert delta to seconds for wasm.update
      const deltaSeconds = delta / 1000.0;

      // Update engine and store values
      const values = Object.fromEntries(wasm.update(deltaSeconds));
      setLatestValues(values);

      // Update list of players
      setPlayerIds(wasm.getPlayerIds());
      setAnimationIds(wasm.getAnimationIds());
    }

    animationFrameRef.current = requestAnimationFrame(updateLoop);
  }, [wasm, targetInterval]);

  useEffect(() => {
    // Start/stop the update loop
    if (wasm.isLoaded) {
      animationFrameRef.current = requestAnimationFrame(updateLoop);
    }
    return () => {
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, [wasm.isLoaded, updateLoop]);

  const createPlayer = useCallback((playerName) => {
    if (!wasm.isLoaded) return;
    const newPlayerId = wasm.createPlayer();
    wasm.updatePlayerConfig(newPlayerId, JSON.stringify({ name: playerName }));
    setPlayerIds(wasm.getPlayerIds());
    return newPlayerId;
  }, [wasm]);

  const value = {
    ...wasm, // Expose all raw WASM methods
    latestValues,
    playerIds,
    animationIds,
    createPlayer,
  };

  return (
    <AnimationEngineContext.Provider value={value}>
      {children}
    </AnimationEngineContext.Provider>
  );
};

export const useAnimationEngine = () => {
  const context = useContext(AnimationEngineContext);
  if (!context) {
    throw new Error('useAnimationEngine must be used within an AnimationEngineProvider');
  }
  return context;
};
