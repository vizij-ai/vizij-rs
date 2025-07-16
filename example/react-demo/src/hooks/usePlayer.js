import { useAnimationEngine } from './useAnimationEngine';
// import { useAnimationEngine } from '../contexts/AnimationEngineContext';
import { useMemo } from 'react';

export const usePlayer = (playerId) => {
  const engine = useAnimationEngine();

  const playerState = useMemo(() => {
    if (!engine.isLoaded || !playerId) return null;
    try {
      const settings = engine.getPlayerSettings(playerId);
      const current_player_time = engine.getPlayerTime(playerId);
      const duration = engine.getPlayerDuration(playerId);
      const state = engine.getPlayerState(playerId);
      // console.log("settings", settings)
      // console.log("state", state)
      return { ...settings, duration, current_player_time, ...state };
    } catch (e) {
      return null; // Player might not exist yet
    }
  }, [engine, playerId]);

  const playerValues = useMemo(() => {
    if (!engine.latestValues || !engine.latestValues[playerId]) return {};
    
      return Object.fromEntries(engine.latestValues[playerId]) || {};
  }, [engine.latestValues, playerId]);

  // Player-specific actions
  const play = () => engine.play(playerId);
  const pause = () => engine.pause(playerId);
  const stop = () => engine.stop(playerId);
  const seek = (time) => engine.seek(playerId, time);
  const addInstance = (animId, config) => {
    const configJson = config ? JSON.stringify(config) : undefined;
    return engine.addInstance(playerId, animId, configJson);
  }
  const removeInstance = (instanceId) => engine.removeInstance(playerId, instanceId);
  const removePlayer = () => engine.removePlayer(playerId);
  const updatePlayerConfig = (config) => {
    const configJson = JSON.stringify(config);
    engine.updatePlayerConfig(playerId, configJson);
  }

  return {
    playerId,
    playerState, // Properties like playback state, time, progress
    playerValues, // The latest animated values for this player's tracks
    play,
    pause,
    stop,
    seek,
    addInstance,
    removeInstance,
    removePlayer,
    updatePlayerConfig,
  };
};
