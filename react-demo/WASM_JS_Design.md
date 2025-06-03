# Design Document

This document contains the design and implementation plan for the react-demo using the wasm animation player.


## WASM Best Practices and Recommendations

When integrating a Rust-based WASM module for animation playback in React, there are several architectural patterns and best practices to consider. Here's a comprehensive breakdown:

### WASM Module Integration Patterns

**Hook-based Approach (Recommended for most cases)**
Create a custom hook that encapsulates your animation class. This provides clean encapsulation and automatic cleanup:

```javascript
function useAnimation() {
  const animationRef = useRef(null);
  const [isLoaded, setIsLoaded] = useState(false);
  
  useEffect(() => {
    // Initialize WASM module
    const initAnimation = async () => {
      const wasmModule = await import('./animation_wasm');
      animationRef.current = new wasmModule.AnimationPlayer();
      setIsLoaded(true);
    };
    
    initAnimation();
    
    return () => {
      // Cleanup WASM resources
      if (animationRef.current) {
        animationRef.current.free?.(); // If your WASM exposes cleanup
        animationRef.current = null;
      }
    };
  }, []);
  
  return {
    animation: animationRef.current,
    isLoaded,
    // Expose wrapper methods here
  };
}
```

**Context-based Approach (For shared state)**
Use React Context when multiple components need access to the same animation instance or when you need global animation state management:

```javascript
const AnimationContext = createContext();

function AnimationProvider({ children }) {
  const [animationInstances, setAnimationInstances] = useState(new Map());
  const wasmModuleRef = useRef(null);
  
  // Context is ideal for managing multiple animation instances
  const createAnimation = useCallback((id) => {
    if (wasmModuleRef.current) {
      const instance = new wasmModuleRef.current.AnimationPlayer();
      setAnimationInstances(prev => new Map(prev).set(id, instance));
      return instance;
    }
  }, []);
  
  return (
    <AnimationContext.Provider value={{ createAnimation, animationInstances }}>
      {children}
    </AnimationContext.Provider>
  );
}
```

### Memory Management Best Practices

**Explicit Resource Cleanup**
WASM modules don't participate in JavaScript's garbage collection for their internal memory. Always implement cleanup:

```javascript
useEffect(() => {
  return () => {
    // Critical: Free WASM memory
    if (animationRef.current?.free) {
      animationRef.current.free();
    }
    // Clear any RAF callbacks
    if (animationFrameRef.current) {
      cancelAnimationFrame(animationFrameRef.current);
    }
  };
}, []);
```

**Memory Pool Pattern**
For frequent animation creation/destruction, consider implementing a pool:

```javascript
class AnimationPool {
  constructor(wasmModule, initialSize = 5) {
    this.wasmModule = wasmModule;
    this.available = [];
    this.inUse = new Set();
    
    // Pre-allocate instances
    for (let i = 0; i < initialSize; i++) {
      this.available.push(new wasmModule.AnimationPlayer());
    }
  }
  
  acquire() {
    let instance = this.available.pop();
    if (!instance) {
      instance = new this.wasmModule.AnimationPlayer();
    }
    this.inUse.add(instance);
    return instance;
  }
  
  release(instance) {
    if (this.inUse.has(instance)) {
      instance.reset?.(); // Reset state without deallocating
      this.inUse.delete(instance);
      this.available.push(instance);
    }
  }
}
```

### Performance Optimization Strategies

**Lazy Loading and Code Splitting**
Load WASM modules only when needed:

```javascript
const useAnimationLazy = () => {
  const [module, setModule] = useState(null);
  
  const loadAnimation = useCallback(async () => {
    if (!module) {
      const wasmModule = await import(
        /* webpackChunkName: "animation-wasm" */ './animation_wasm'
      );
      setModule(wasmModule);
      return wasmModule;
    }
    return module;
  }, [module]);
  
  return { loadAnimation, isLoaded: !!module };
};
```

**Animation Frame Management**
For smooth animation playback, coordinate with React's rendering cycle:

```javascript
function useAnimationFrame(callback, isPlaying) {
  const requestRef = useRef();
  const previousTimeRef = useRef();
  
  const animate = useCallback((time) => {
    if (previousTimeRef.current !== undefined) {
      const deltaTime = time - previousTimeRef.current;
      callback(deltaTime);
    }
    previousTimeRef.current = time;
    requestRef.current = requestAnimationFrame(animate);
  }, [callback]);
  
  useEffect(() => {
    if (isPlaying) {
      requestRef.current = requestAnimationFrame(animate);
    } else {
      cancelAnimationFrame(requestRef.current);
    }
    
    return () => cancelAnimationFrame(requestRef.current);
  }, [isPlaying, animate]);
}
```

### State Management Integration

**With React State**
Keep WASM data synchronized with React state for UI updates:

```javascript
function useAnimationState(animationInstance) {
  const [currentTime, setCurrentTime] = useState(0);
  const [tracks, setTracks] = useState([]);
  const [isPlaying, setIsPlaying] = useState(false);
  
  const updateFromWasm = useCallback(() => {
    if (animationInstance) {
      setCurrentTime(animationInstance.getCurrentTime());
      setTracks(animationInstance.getActiveTracks());
    }
  }, [animationInstance]);
  
  useAnimationFrame(updateFromWasm, isPlaying);
  
  return { currentTime, tracks, isPlaying, setIsPlaying };
}
```

**With External State Management**
For complex applications, integrate with Redux/Zustand:

```javascript
// Zustand store
const useAnimationStore = create((set, get) => ({
  animations: new Map(),
  addAnimation: (id, instance) => 
    set(state => ({ 
      animations: new Map(state.animations).set(id, instance) 
    })),
  updateAnimation: (id, time) => {
    const animation = get().animations.get(id);
    if (animation) {
      animation.setTime(time);
      // Batch updates to prevent excessive re-renders
      set(state => ({ lastUpdate: Date.now() }));
    }
  }
}));
```

### Error Handling and Debugging

**Robust Error Boundaries**
WASM errors can crash your application:

```javascript
function AnimationErrorBoundary({ children }) {
  const [hasError, setHasError] = useState(false);
  
  useEffect(() => {
    const handleWasmError = (event) => {
      console.error('WASM Error:', event);
      setHasError(true);
    };
    
    window.addEventListener('unhandledrejection', handleWasmError);
    return () => window.removeEventListener('unhandledrejection', handleWasmError);
  }, []);
  
  if (hasError) {
    return <div>Animation failed to load</div>;
  }
  
  return children;
}
```

### Recommended Architecture

For your animation use case, I'd recommend:

1. **Hook for individual animations**: Use `useAnimation()` for components that need their own animation instance
2. **Context for shared resources**: Use context to share the WASM module loading state and common animation utilities
3. **Separate data layer**: Keep animation data (tracks, keyframes) in React state while using WASM for calculations
4. **RAF coordination**: Use `useAnimationFrame` to sync WASM updates with React renders

This approach gives you clean separation of concerns, proper resource management, and optimal performance for animation-heavy applications. The key is treating WASM as a computational engine while keeping React responsible for UI state and rendering coordination.