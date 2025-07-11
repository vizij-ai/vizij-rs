import { useAnimationEngine } from '../contexts/AnimationEngineContext';
import { useMemo } from 'react';

export const useAnimationInstance = (playerId, instanceId) => {
  const engine = useAnimationEngine();

  const instanceConfig = useMemo(() => {
    if (!engine.isLoaded || !playerId || !instanceId) return null;
    try {
      const playerState = engine.getPlayerState(playerId);
      return playerState?.instances.find(inst => inst.instanceId === instanceId) || null;
    } catch (e) {
      return null;
    }
  }, [engine, playerId, instanceId]);

  const updateConfig = (config) => {
    if (!engine.isLoaded) return;
    const configJson = JSON.stringify(config);
    engine.updateInstanceConfig(playerId, instanceId, configJson);
  };

  return {
    instanceId,
    playerId,
    config: instanceConfig, // weight, timeScale, enabled
    updateConfig,
  };
};
