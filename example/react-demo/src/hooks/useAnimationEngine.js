import { useContext } from 'react';
import { AnimationEngineContext } from '../contexts/AnimationEngineContext';

export const useAnimationEngine = () => {
  const context = useContext(AnimationEngineContext);
  if (!context) {
    throw new Error('useAnimationEngine must be used within an AnimationEngineProvider');
  }
  return context;
};
