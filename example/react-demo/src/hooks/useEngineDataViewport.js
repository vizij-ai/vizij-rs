import { useState, useEffect } from 'react';
import { useAnimationEngine } from '../contexts/AnimationEngineContext';

export const useEngineDataViewport = ({ durationMs = 5000, enabled = true }) => {
  const { latestValues } = useAnimationEngine();
  const [history, setHistory] = useState({}); // { [playerId]: { [trackName]: [[timestamp, value], ...] } }

  useEffect(() => {
    if (!enabled || !latestValues) return;

    const now = performance.now();
    const cutoff = now - durationMs;

    setHistory(prevHistory => {
      const newHistory = { ...prevHistory };

      // Add new values
      for (const [playerId, playerValues] of Object.entries(latestValues)) {
        if (!newHistory[playerId]) newHistory[playerId] = {};
        const playerValEntries = playerValues instanceof Map ? Array.from(playerValues.entries()) : Object.entries(playerValues);
        for (const [trackName, trackValue] of playerValEntries) {
          if (!newHistory[playerId][trackName]) newHistory[playerId][trackName] = [];
          
          // Extract numeric value
          let numericValue = 0;
          if (typeof trackValue === 'object' && trackValue !== null) {
             if ('Float' in trackValue) {
                numericValue = trackValue.Float;
             } else if ('Int' in trackValue) {
                numericValue = trackValue.Int;
             } else if ('Bool' in trackValue) {
                numericValue = trackValue.Bool ? 1 : 0;
             }
          } else if (typeof trackValue === 'number') {
            numericValue = trackValue;
          }

          newHistory[playerId][trackName].push([now, numericValue]);
        }
      }

      // Prune old values
      for (const playerId in newHistory) {
        for (const trackName in newHistory[playerId]) {
          newHistory[playerId][trackName] = newHistory[playerId][trackName].filter(
            point => point[0] >= cutoff
          );
          if (newHistory[playerId][trackName].length === 0) {
            delete newHistory[playerId][trackName];
          }
        }
        if (Object.keys(newHistory[playerId]).length === 0) {
            delete newHistory[playerId];
        }
      }

      return newHistory;
    });
  }, [latestValues, durationMs, enabled]);

  return history;
};
