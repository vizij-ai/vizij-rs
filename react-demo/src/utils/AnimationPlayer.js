// Unified Animation Player - Combines features from both implementations
import wasminit, { WasmAnimationEngine, create_test_animation } from 'animation-player';
// import { console_log, WasmAnimationEngine } from '../../pkg/animation_player.js';

export async function initWasm() {

  await wasminit();
} 


/**
 * Unified AnimationPlayer class that provides comprehensive animation playback capabilities
 * with WASM integration, event system, performance monitoring, and flexible update mechanisms.
 * 
 * @class AnimationPlayer
 */
export class AnimationPlayer {
  /**
   * Create an AnimationPlayer instance
   * @param {Object} [options] - Configuration options
   * @param {Object} [options.config] - Engine configuration
   * @param {Object} [options.polling] - Polling options
   * @param {Object} [options.logging] - Logging configuration
   */
  constructor(options = {}) {
    // Initialize WASM engine
    const config = options.config ? JSON.stringify(options.config) : null;
    this.engine = new WasmAnimationEngine(config);
    
    // Update mechanism state
    this.updateInterval = null;
    this.isPolling = false;
    this.lastUpdateTime = 0;
    this.lastFrameTime = 0;
    
    // Event system
    this.eventListeners = new Map();
    this.subscribers = new Set();
    
    // Performance tracking
    this.metrics = {
      fps: 0, // frames per second
      frame_time_ms: 0, // lag from call to update
      memory_usage_mb: 0,
      active_animations: 0,
      total_updates: 0,
      last_update_time: 0
    };
    this.frameTimeHistory = [];
    this.maxFrameHistory = 60;
    
    // Configuration
    this.pollingOptions = {
      updateRate: 60,
      enabled: false,
      ...options.polling
    };
    
    this.logLevel = options.logging?.level || 'info';
    this.enableConsoleLogging = options.logging?.enableConsole !== false;
    
    // Value change tracking
    this.lastValues = {};
    this.lastDerivatives = {};
    this.derivativeHistory = {};
    
    // Time series history tracking
    this.valueHistory = {}; // Stores arrays of values by key name
    this.historyMetadata = {
      timestamps: [], // Timestamp for each capture point
      captureCount: 0,
      startTime: null,
      lastCaptureTime: null
    };
    
    // History configuration
    this.historyOptions = {
      enabled: true,
      maxLength: 1000, // Maximum number of values to keep per key
      captureInterval: 1, // Capture every N polling cycles (1 = every cycle)
      includeTimestamps: true,
      ...options.history
    };
    
    this.captureCounter = 0; // Counter for capture interval
    
    this.log('info', 'AnimationPlayer initialized');
  }

  getTestAnimationData() {
    return create_test_animation();
  }

  // ========================================
  // Animation Loading Methods
  // ========================================

  /**
   * Load animation from data object or JSON string
   * @param {Object|string} animationData - Animation data or JSON string
   * @returns {Promise<void>}
   */
  async loadAnimation(animationData) {
    try {
      const json = typeof animationData === 'string' 
        ? animationData 
        : JSON.stringify(animationData);
      console.log("Sending animation to engine", json)
      this.engine.load_animation(json);
      this.emit('animation_loaded', { 
        animation_id: typeof animationData === 'object' ? animationData.id : 'unknown'
      });
      this.log('info', 'Animation loaded successfully');
    } catch (error) {
      this.emit('error', { error: error.message });
      this.log('error', `Failed to load animation: ${error.message}`);
      throw error;
    }
  }

  /**
   * Load animation from URL
   * @param {string} url - URL to fetch animation data from
   * @returns {Promise<void>}
   */
  async loadAnimationFromUrl(url) {
    try {
      const response = await fetch(url);
      if (!response.ok) {
        throw new Error(`Failed to fetch animation: ${response.status}`);
      }
      const animationData = await response.json();
      await this.loadAnimation(animationData);
    } catch (error) {
      this.emit('error', { error: error.message });
      this.log('error', `Failed to load animation from URL: ${error.message}`);
      throw error;
    }
  }

  /**
   * Convenience method that loads animation and creates player with instance
   * @param {string} playerId - ID for the player
   * @param {Object|string} animationData - Animation data
   * @returns {Promise<void>}
   */
  async loadAnimationWithPlayer(playerId, animationData) {
    console.log("Loading Animation")
    await this.loadAnimation(animationData);

    console.log("Got animation, creating player on AnimmationPlayer.js", animationData)

    this.createPlayer(playerId);
    console.log("Player Created With id", playerId)
    
    // Extract animation ID from the data structure
    let animId;
    if (typeof animationData === 'object') {
      // Check if it has an explicit id field
      if (animationData.id) {
        animId = animationData.id;
      } else {
        // Extract the first key as animation ID (common pattern)
        const keys = Object.keys(animationData);
        animId = keys.length > 0 ? keys[0] : 'unknown';
      }
    } else if (typeof animationData === 'string') {
      const d = JSON.parse(animationData)
      console.log("PGetting id from", d)
      animId = d.id;
    } else {
      animId = 'unknown';
    }
    console.log("Animmation ID adding to player:", animId)

    
    this.addInstance(playerId, 'default_instance', animId);
  }

  // ========================================
  // Player Management Methods
  // ========================================

  /**
   * Create a new player
   * @param {string} playerId - Unique identifier for the player
   */
  createPlayer(playerId) {
    try {
      this.engine.create_player(playerId);
      this.emit('player_created', { player_id: playerId });
      this.log('info', `Player created: ${playerId}`);
    } catch (error) {
      this.emit('error', { error: error.message, player_id: playerId });
      this.log('error', `Failed to create player ${playerId}: ${error.message}`);
      throw error;
    }
  }

  /**
   * Add animation instance to a player
   * @param {string} playerId - Player ID
   * @param {string} instanceId - Instance ID
   * @param {string} animationId - Animation ID
   */
  addInstance(playerId, instanceId, animationId) {
    try {
      this.engine.add_instance(playerId, instanceId, animationId);
      this.emit('instance_added', { player_id: playerId, instance_id: instanceId, animation_id: animationId });
      this.log('info', `Instance added: ${instanceId} to player ${playerId}`);
    } catch (error) {
      this.emit('error', { error: error.message, player_id: playerId });
      this.log('error', `Failed to add instance to player ${playerId}: ${error.message}`);
      throw error;
    }
  }

  // ========================================
  // Playback Control Methods
  // ========================================

  /**
   * Start playback for a player
   * @param {string} playerId - Player ID
   */
  play(playerId) {
    try {
      this.engine.play(playerId);
      this.emit('play', { player_id: playerId });
      this.log('debug', `Started playback for player: ${playerId}`);
    } catch (error) {
      this.emit('error', { error: error.message, player_id: playerId });
      this.log('error', `Failed to play ${playerId}: ${error.message}`);
      throw error;
    }
  }

  /**
   * Pause playback for a player
   * @param {string} playerId - Player ID
   */
  pause(playerId) {
    try {
      this.engine.pause(playerId);
      this.emit('pause', { player_id: playerId });
      this.log('debug', `Paused playback for player: ${playerId}`);
    } catch (error) {
      this.emit('error', { error: error.message, player_id: playerId });
      this.log('error', `Failed to pause ${playerId}: ${error.message}`);
      throw error;
    }
  }

  /**
   * Stop playback for a player
   * @param {string} playerId - Player ID
   */
  stop(playerId) {
    try {
      this.engine.stop(playerId);
      this.emit('stop', { player_id: playerId });
      this.log('debug', `Stopped playback for player: ${playerId}`);
    } catch (error) {
      this.emit('error', { error: error.message, player_id: playerId });
      this.log('error', `Failed to stop ${playerId}: ${error.message}`);
      throw error;
    }
  }

  /**
   * Seek to specific time for a player
   * @param {string} playerId - Player ID
   * @param {number} time - Time in seconds
   */
  seek(playerId, time) {
    try {
      this.engine.seek(playerId, time);
      this.emit('seek', { player_id: playerId, time });
      this.log('debug', `Seeked player ${playerId} to time: ${time}`);
    } catch (error) {
      this.emit('error', { error: error.message, player_id: playerId });
      this.log('error', `Failed to seek ${playerId}: ${error.message}`);
      throw error;
    }
  }

  // ========================================
  // Update Methods
  // ========================================

  /**
   * Manually update animation by one frame
   * @param {number} [frameDelta] - Frame delta in seconds, calculated if not provided
   * @returns {Object} Current animation values
   */
  update(frameDelta) {
    try {
      const delta = frameDelta !== undefined ? frameDelta : this.calculateFrameDelta();
      const values = this.engine.update(delta);
      this.updateMetrics();
      return values;
    } catch (error) {
      this.emit('error', { error: error.message });
      this.log('error', `Update failed: ${error.message}`);
      return {};
    }
  }

  setPollingRate(updateRate) {
    this.pollingOptions.updateRate = updateRate
  }

  /**
   * Start automatic polling/streaming updates
   */
  startPolling() {
    if (this.isPolling) {
      this.stopPolling();
    }

    const updateRate = this.pollingOptions.updateRate;
    this.pollingOptions.enabled = true;
    this.isPolling = true;
    
    const intervalMs = 1000 / updateRate;
    const derivativeWidth = Math.round(1_000_000.0/updateRate);
    this.lastUpdateTime = performance.now();
    
    this.updateInterval = setInterval(() => {
      try {
        // Update metrics
        this.updateMetrics();
        const now = performance.now();
        const frameDelta = (now - this.lastUpdateTime) / 1000.0;
        this.lastUpdateTime = now;
        const values = this.engine.update(frameDelta);
        // Create update object for compatibility
        const update = {
          timestamp: Date.now(),
          values,
          player_id: 'all',
          frame_info: this.getFrameInfo(),
          metadata: { updateRate, interval: intervalMs }
        };
        
        // Check for value changes and emit events
        this.checkForValueChanges(values);
        
        // Get derivatives for first player
        const playerIds = this.getPlayerIds();
        let derivatives = {};
        if (playerIds.length > 0) {
          try {
            derivatives = this.getCurrentDerivatives(derivativeWidth); // 1ns derivative width
          } catch (error) {
            // Silently ignore derivative errors
          }
        }
        
        // Capture values and derivatives for time series history
        this.captureValueHistory(values, now);
        this.captureDerivativeHistory(derivatives, now);
        
        // Emit update event
        this.emit('update', update);
        
        // Call subscribers
        this.subscribers.forEach(callback => {
          try {
            callback(update);
          } catch (error) {
            this.log('error', `Subscriber callback error: ${error.message}`);
          }
        });

        // Check for performance warnings
        this.checkPerformanceWarnings();
        
      } catch (error) {
        this.emit('error', { error: error.message });
        this.log('error', `Polling update failed: ${error.message}`);
      }
    }, intervalMs);

    this.log('info', `Started polling at ${updateRate} FPS`);
  }

  /**
   * Alias for startPolling for backward compatibility
   * @param {number} [updateRate=60] - Updates per second
   */
  startStreaming(updateRate = 30) {
    return this.startPolling(updateRate);
  }

  /**
   * Stop automatic updates
   */
  stopPolling() {
    if (this.updateInterval) {
      clearInterval(this.updateInterval);
      this.updateInterval = null;
    }
    this.isPolling = false;
    this.pollingOptions.enabled = false;
    this.log('info', 'Stopped polling');
  }

  /**
   * Alias for stopPolling for backward compatibility
   */
  stopStreaming() {
    return this.stopPolling();
  }

  /**
   * Check if currently polling/streaming
   * @returns {boolean}
   */
  isPollingActive() {
    return this.isPolling;
  }

  // ========================================
  // Event System Methods
  // ========================================

  /**
   * Subscribe to animation updates
   * @param {Function} callback - Callback function to receive updates
   * @returns {Function|string} Unsubscribe function or subscription ID
   */
  subscribe(callback) {
    if (typeof callback === 'function') {
      this.subscribers.add(callback);
      this.log('debug', 'New subscriber added');
      
      // Return unsubscribe function
      return () => {
        this.subscribers.delete(callback);
        this.log('debug', 'Subscriber removed');
      };
    } else {
      // Legacy compatibility - return subscription ID
      const id = Math.random().toString(36).substr(2, 9);
      this.subscribers.set(id, callback);
      return id;
    }
  }

  /**
   * Unsubscribe from updates (legacy compatibility)
   * @param {string} subscriptionId - Subscription ID
   * @returns {boolean}
   */
  unsubscribe(subscriptionId) {
    return this.subscribers.delete(subscriptionId);
  }

  /**
   * Add event listener
   * @param {string} event - Event type
   * @param {Function} callback - Event handler
   */
  on(event, callback) {
    if (!this.eventListeners.has(event)) {
      this.eventListeners.set(event, []);
    }
    this.eventListeners.get(event).push(callback);
    this.log('debug', `Event listener added for: ${event}`);
  }

  /**
   * Remove event listener
   * @param {string} event - Event type
   * @param {Function} callback - Event handler to remove
   */
  off(event, callback) {
    const listeners = this.eventListeners.get(event);
    if (listeners) {
      const index = listeners.indexOf(callback);
      if (index !== -1) {
        listeners.splice(index, 1);
        this.log('debug', `Event listener removed for: ${event}`);
      }
    }
  }

  /**
   * Emit an event to all listeners
   * @private
   * @param {string} event - Event type
   * @param {*} data - Event data
   */
  emit(event, data) {
    const listeners = this.eventListeners.get(event);
    if (listeners) {
      listeners.forEach(callback => {
        try {
          callback(data);
        } catch (error) {
          this.log('error', `Event listener error for ${event}: ${error.message}`);
        }
      });
    }
  }

  // ========================================
  // Player State Query Methods
  // ========================================

  /**
   * Get player state
   * @param {string} playerId - Player ID
   * @returns {string} Player state
   */
  getPlayerState(playerId) {
    try {
      let player_state = JSON.parse(this.engine.get_player_state(playerId));
      return player_state;
    } catch (error) {
      this.log('error', `Failed to get state for ${playerId}: ${error.message}`);
      return 'error';
    }
  }

  /**
   * Get current player time
   * @param {string} playerId - Player ID
   * @returns {number} Current time in seconds
   */
  getPlayerTime(playerId) {
    try {
      return this.engine.get_player_time(playerId);
    } catch (error) {
      this.log('error', `Failed to get time for ${playerId}: ${error.message}`);
      return 0;
    }
  }

  /**
   * Get player progress (0-1)
   * @param {string} playerId - Player ID
   * @returns {number} Progress from 0 to 1
   */
  getPlayerProgress(playerId) {
    try {
      return this.engine.get_player_progress(playerId);
    } catch (error) {
      this.log('error', `Failed to get progress for ${playerId}: ${error.message}`);
      return 0;
    }
  }

  /**
   * Get all player IDs
   * @returns {string[]} Array of player IDs
   */
  getPlayerIds() {
    try {
      return this.engine.get_player_ids();
    } catch (error) {
      this.log('error', `Failed to get player IDs: ${error.message}`);
      return [];
    }
  }

  /**
   * Get frame information for display
   * @returns {Object} Frame information
   */
  getFrameInfo() {
    const playerIds = this.getPlayerIds();
    if (playerIds.length === 0) {
      return {
        current_time: 0,
        duration: 0,
        playing: false,
        loop_count: 0,
        playback_rate: 1.0
      };
    }

    const playerId = playerIds[0]; // Use first player
    const playerState = this.getPlayerState(playerId)
    try {
      return {
        current_time: this.getPlayerTime(playerId),
        duration: playerState["end_time"], // Default from test animation
        playing: playerState["playback_state"] === 'Playing',
        loop_count: 0,
        playback_rate: playerState["speed"]
      };
    } catch (error) {
      return {
        current_time: 0,
        duration: 0,
        playing: false,
        loop_count: 0,
        playback_rate: 1.0
      };
    }
  }

  // ========================================
  // Performance Monitoring Methods
  // ========================================

  /**
   * Get performance statistics from engine
   * @returns {Object} Performance stats
   */
  getPerformanceStats() {
    try {
      return this.engine.get_metrics();
    } catch (error) {
      this.log('error', `Failed to get performance stats: ${error.message}`);
      return {};
    }
  }

  /**
   * Get player metrics
   * @returns {Object} Player metrics
   */
  getPlayerMetrics() {
    return { ...this.metrics };
  }

  // ========================================
  // Data Export Methods
  // ========================================

  /**
   * Update player configuration (e.g., loop, ping-pong)
   * @param {string} playerId - Player ID
   * @param {Object} config - Configuration object
   */
  updatePlayerConfig(playerId, config) {
    try {
      const configJson = JSON.stringify(config);
      this.engine.update_player_config(playerId, configJson);
      this.log('debug', `Updated config for player ${playerId}: ${configJson}`);
    } catch (error) {
      this.emit('error', { error: error.message, player_id: playerId });
      this.log('error', `Failed to update config for ${playerId}: ${error.message}`);
      throw error;
    }
  }

  /**
   * Set playback speed for a player
   * @param {string} playerId - Player ID
   * @param {number} speed - Speed multiplier (-5.0 to 5.0)
   */
  setSpeed(playerId, speed) {
    if (speed < -5.0 || speed > 5.0) {
      throw new Error(`Speed must be between -5.0 and 5.0, got: ${speed}`);
    }
    this.updatePlayerConfig(playerId, { speed });
  }

  /**
   * Set playback mode for a player
   * @param {string} playerId - Player ID
   * @param {string} mode - Playback mode ('once', 'loop', 'ping_pong')
   */
  setPlaybackMode(playerId, mode) {
    const validModes = ['once', 'loop', 'ping_pong'];
    if (!validModes.includes(mode)) {
      throw new Error(`Invalid playback mode: ${mode}. Valid options: ${validModes.join(', ')}`);
    }
    this.updatePlayerConfig(playerId, { mode });
  }

  /**
   * Set start time for a player
   * @param {string} playerId - Player ID
   * @param {number} startTime - Start time in seconds (must be positive)
   */
  setStartTime(playerId, startTime) {
    if (startTime < 0) {
      throw new Error(`Start time must be positive, got: ${startTime}`);
    }
    this.updatePlayerConfig(playerId, { start_time: startTime });
  }

  /**
   * Set end time for a player
   * @param {string} playerId - Player ID
   * @param {number|null} endTime - End time in seconds (must be positive) or null for no limit
   */
  setEndTime(playerId, endTime) {
    if (endTime !== null && endTime < 0) {
      throw new Error(`End time must be positive or null, got: ${endTime}`);
    }
    this.updatePlayerConfig(playerId, { end_time: endTime });
  }

  /**
   * Set time range for a player
   * @param {string} playerId - Player ID
   * @param {number} startTime - Start time in seconds (must be positive)
   * @param {number|null} endTime - End time in seconds (must be positive) or null for no limit
   */
  setTimeRange(playerId, startTime, endTime) {
    if (startTime < 0) {
      throw new Error(`Start time must be positive, got: ${startTime}`);
    }
    if (endTime !== null && endTime < 0) {
      throw new Error(`End time must be positive or null, got: ${endTime}`);
    }
    if (endTime !== null && startTime >= endTime) {
      throw new Error(`Start time (${startTime}) must be less than end time (${endTime})`);
    }
    this.updatePlayerConfig(playerId, { start_time: startTime, end_time: endTime });
  }

  /**
   * Set looping for a player (legacy method)
   * @param {string} playerId - Player ID
   * @param {boolean} loop - Whether to loop
   */
  setLoop(playerId, loop) {
    this.setPlaybackMode(playerId, loop ? 'loop' : 'once');
  }

  /**
   * Set ping-pong mode for a player (legacy method)
   * @param {string} playerId - Player ID
   * @param {boolean} pingPong - Whether to ping-pong
   */
  setPingPong(playerId, pingPong) {
    this.setPlaybackMode(playerId, pingPong ? 'ping_pong' : 'once');
  }

  /**
   * Export animation data
   * @param {string} animationId - Animation ID to export
   * @returns {Object} Animation data
   */
  exportAnimation(animationId) {
    try {
      const json = this.engine.export_animation(animationId);
      return JSON.parse(json);
    } catch (error) {
      this.log('error', `Failed to export animation ${animationId}: ${error.message}`);
      throw error;
    }
  }

  // ========================================
  // Baking Methods
  // ========================================

  /**
   * Bake animation at specified frame rate
   * @param {string} animationId - Animation ID to bake
   * @param {number} frameRate - Frame rate for baking (default: 60)
   * @param {Object} [config] - Additional baking configuration
   * @returns {Object} Baked animation data
   */
  bakeAnimation(animationId, frameRate = 60, config = {}) {
    try {
      const bakingConfig = {
        frame_rate: frameRate,
        include_disabled_tracks: config.includeDisabledTracks || false,
        apply_track_weights: config.applyTrackWeights !== false,
        time_range: config.timeRange || null,
        interpolation_method: config.interpolationMethod || 'linear',
        include_derivatives: config.includeDerivatives || true,
        derivative_width: config.derivativeWidth || 1_000_000_000.0/frameRate,
        //1_000_000_000
      };
      console.log("Baking with config:", bakingConfig);
      const configJson = JSON.stringify(bakingConfig);
      const bakedDataJson = this.engine.bake_animation(animationId, configJson);
      const bakedData = JSON.parse(bakedDataJson);
      this.emit('animation_baked', { 
        animation_id: animationId, 
        frame_rate: frameRate,
        sample_count: bakedData.frame_count || 0
      });

      bakedData.duration = bakedData.duration/1_000_000_000
      
      this.log('info', `Animation '${animationId}' baked at ${frameRate} FPS`);
      return bakedData;
    } catch (error) {
      this.emit('error', { error: error.message, animation_id: animationId });
      this.log('error', `Failed to bake animation '${animationId}': ${error.message}`);
      throw error;
    }
  }

  /**
   * Generate baked data for the currently loaded animation
   * @param {number} frameRate - Frame rate for baking
   * @param {Object} [config] - Additional baking configuration
   * @returns {Object} Baked animation data
   */
  bakeCurrent(frameRate = 60, config = {}) {
    try {
      const playerIds = this.getPlayerIds();
      if (playerIds.length === 0) {
        throw new Error('No players available for baking');
      }
      
      // Use test_animation as the animation ID
      const animationId = 'test_animation';
      return this.bakeAnimation(animationId, frameRate, config);;
    } catch (error) {
      this.log('error', `Failed to bake current animation: ${error.message}`);
      throw error;
    }
  }

  /**
   * Get derivatives for all tracks at current time
   * @param {string} playerId - Player ID
   * @param {number} [derivativeWidth] - Width in milliseconds (default: 1)
   * @returns {Object} Derivative values
   */
  getDerivatives(playerId, derivativeWidth = 1.0) {
    try {
      return this.engine.get_derivatives(playerId, derivativeWidth);
    } catch (error) {
      this.log('error', `Failed to get derivatives for ${playerId}: ${error.message}`);
      throw error;
    }
  }

  /**
   * Get derivatives for the first available player
   * @param {number} [derivativeWidth] - Width in milliseconds
   * @returns {Object} Derivative values
   */
  getCurrentDerivatives(derivativeWidth = 1.0) {
    try {
      const playerIds = this.getPlayerIds();
      if (playerIds.length === 0) {
        return {};
      }
      return this.getDerivatives(playerIds[0], derivativeWidth);
    } catch (error) {
      this.log('error', `Failed to get current derivatives: ${error.message}`);
      return {};
    }
  }

  // ========================================
  // Private Helper Methods
  // ========================================

  /**
   * Calculate frame delta from last update
   * @private
   * @returns {number} Frame delta in seconds
   */
  calculateFrameDelta() {
    const now = performance.now();
    if (this.lastFrameTime === 0) {
      this.lastFrameTime = now;
      return 1.0 / 60.0; // Default to 60 FPS for first frame
    }
    const delta = (now - this.lastFrameTime) / 1000.0;
    this.lastFrameTime = now;
    return Math.min(delta, 1.0 / 30.0); // Cap at 30 FPS minimum
  }

  /**
   * Update performance metrics
   * @private
   */
  updateMetrics() {
    const now = performance.now(); // ms counter

    if (this.lastUpdateTime > 0) {
      const frameTime = now - this.lastUpdateTime;
      this.frameTimeHistory.push(frameTime);
      
      if (this.frameTimeHistory.length > this.maxFrameHistory) {
        this.frameTimeHistory.shift();
      }
      
      // Calculate average frame time and FPS
      const avgFrameTime = this.frameTimeHistory.reduce((a, b) => a + b, 0) / this.frameTimeHistory.length;
      this.metrics.frame_time_ms = avgFrameTime;
      this.metrics.fps = avgFrameTime > 0 ? 1000 / avgFrameTime : 0;
    }
    
    this.metrics.total_updates++;
    this.metrics.last_update_time = now;
    // this.lastUpdateTime = now;
    
    // Get engine stats
    try {
      const engineStats = this.getPerformanceStats();
      this.metrics.memory_usage_mb = engineStats.total_memory_mb || 0;
      this.metrics.active_animations = engineStats.playing_players || 0;
    } catch (error) {
      // Ignore errors in metrics collection
    }
  }

  /**
   * Check for value changes and emit events
   * @private
   * @param {Object} currentValues - Current animation values
   */
  checkForValueChanges(currentValues) {
    if (JSON.stringify(currentValues) !== JSON.stringify(this.lastValues)) {
      this.emit('values_changed', { 
        previous: this.lastValues, 
        current: currentValues,
        timestamp: Date.now()
      });
    }
    this.lastValues = currentValues;
  }

  /**
   * Check for performance warnings
   * @private
   */
  checkPerformanceWarnings() {
    const stats = this.getPerformanceStats();
    
    // Check FPS warning
    if (stats.average_fps && stats.average_fps < this.pollingOptions.updateRate * 0.8) {
      this.emit('performance_warning', {
        type: 'low_fps',
        expected: this.pollingOptions.updateRate,
        actual: stats.average_fps,
        message: `FPS below expected: ${stats.average_fps.toFixed(1)} < ${this.pollingOptions.updateRate * 0.8}`
      });
    }
    
    // Check memory warning
    if (stats.total_memory_mb && stats.total_memory_mb > 100) {
      this.emit('performance_warning', {
        type: 'high_memory',
        value: stats.total_memory_mb,
        message: `Memory usage high: ${stats.total_memory_mb.toFixed(1)} MB`
      });
    }
  }

  /**
   * Log a message
   * @private
   * @param {string} level - Log level
   * @param {string} message - Message to log
   */
  log(level, message) {
    if (!this.enableConsoleLogging) return;
    
    const levels = { debug: 0, info: 1, warn: 2, error: 3 };
    const currentLevel = levels[this.logLevel];
    const messageLevel = levels[level];
    
    if (messageLevel >= currentLevel) {
      const timestamp = new Date().toISOString();
      const logMessage = `[${timestamp}] [AnimationPlayer] [${level.toUpperCase()}] ${message}`;
      
      switch (level) {
        case 'debug':
          console.debug(logMessage);
          break;
        case 'info':
          console.info(logMessage);
          break;
        case 'warn':
          console.warn(logMessage);
          break;
        case 'error':
          console.error(logMessage);
          break;
      }
    }
  }

  // ========================================
  // Time Series History Methods
  // ========================================

  /**
   * Capture current values for time series history
   * @private
   * @param {Object} values - Current animation values
   * @param {number} timestamp - Current timestamp
   */
  captureValueHistory(values, timestamp) {
    if (!this.historyOptions.enabled) return;
    
    // Check capture interval
    this.captureCounter++;
    if (this.captureCounter % this.historyOptions.captureInterval !== 0) {
      return;
    }
    
    // Initialize start time if needed
    if (this.historyMetadata.startTime === null) {
      this.historyMetadata.startTime = timestamp;
    }
    const isMap = values instanceof Map;

    const entries = isMap ? Array.from(values.entries()) : Object.entries(values);
    for (const [playerId, playerValues] of entries) {
      // console.log(playerId, playerValues);
      // Extract numeric values from the wrapped format
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
        
        // Initialize array if needed
        if (!this.valueHistory[key]) {
          this.valueHistory[key] = [];
        }
        
        // Add value to history
        this.valueHistory[key].push(numericValue);
        
        // Maintain max length limit
        if (this.valueHistory[key].length > this.historyOptions.maxLength) {
          this.valueHistory[key].shift();
        }
      });
    }
    
    // Update metadata
    if (this.historyOptions.includeTimestamps) {
      this.historyMetadata.timestamps.push(timestamp);
      
      // Maintain timestamp array length
      if (this.historyMetadata.timestamps.length > this.historyOptions.maxLength) {
        this.historyMetadata.timestamps.shift();
      }
    }
    
    this.historyMetadata.captureCount++;
    this.historyMetadata.lastCaptureTime = timestamp;
  }

  /**
   * Get time series history for a specific animation key
   * @param {string} keyName - Animation key name (e.g., 'a.step', 'ab.linear')
   * @returns {number[]} Array of captured values over time
   */
  getValueHistory(keyName) {
    return this.valueHistory[keyName] ? [...this.valueHistory[keyName]] : [];
  }

  /**
   * Get complete time series data for all animation keys
   * @returns {Object} Object with key names as properties and value arrays
   */
  getAllValueHistory() {
    const result = {};
    Object.entries(this.valueHistory).forEach(([key, values]) => {
      result[key] = [...values];
    });
    return result;
  }

  /**
   * Clear all captured history data
   */
  clearValueHistory() {
    this.valueHistory = {};
    this.derivativeHistory = {};
    this.historyMetadata = {
      timestamps: [],
      captureCount: 0,
      startTime: null,
      lastCaptureTime: null
    };
    this.captureCounter = 0;
    this.log('info', 'Value history cleared');
  }

  /**
   * Capture current derivatives for time series history
   * @private
   * @param {Object} derivatives - Current derivative values
   * @param {number} timestamp - Current timestamp
   */
  captureDerivativeHistory(derivatives, timestamp) {
    if (!this.historyOptions.enabled || !derivatives) return;
    
    // Check capture interval
    if (this.captureCounter % this.historyOptions.captureInterval !== 0) {
      return;
    }
    
    // Handle Map or Object format similar to captureValueHistory
    const isMap = derivatives instanceof Map;
    const entries = isMap ? Array.from(derivatives.entries()) : Object.entries(derivatives);
    
    for (const [key, wrappedValue] of entries) {
      let numericValue;
      
      // Handle different value types (Float, Int, Bool, etc.) - same as captureValueHistory
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
      
      // Initialize array if needed
      if (!this.derivativeHistory[key]) {
        this.derivativeHistory[key] = [];
      }
      
      // Add derivative value to history
      this.derivativeHistory[key].push(numericValue);
      
      // Maintain max length limit
      if (this.derivativeHistory[key].length > this.historyOptions.maxLength) {
        this.derivativeHistory[key].shift();
      }
    }
  }

  /**
   * Get derivative history for a specific animation key
   * @param {string} keyName - Animation key name
   * @returns {number[]} Array of captured derivative values over time
   */
  getDerivativeHistory(keyName) {
    return this.derivativeHistory[keyName] ? [...this.derivativeHistory[keyName]] : [];
  }

  /**
   * Get complete derivative time series data for all animation keys
   * @returns {Object} Object with key names as properties and derivative arrays
   */
  getAllDerivativeHistory() {
    const result = {};
    Object.entries(this.derivativeHistory).forEach(([key, values]) => {
      result[key] = [...values];
    });
    return result;
  }

  /**
   * Get statistics about the captured history data
   * @returns {Object} History statistics
   */
  getHistoryStats() {
    const keys = Object.keys(this.valueHistory);
    const totalKeys = keys.length;
    const totalCaptures = this.historyMetadata.captureCount;
    
    // Calculate total values and average
    const totalValues = keys.reduce((sum, key) => sum + this.valueHistory[key].length, 0);
    const averageValuesPerKey = totalKeys > 0 ? totalValues / totalKeys : 0;
    
    // Estimate memory usage (rough calculation)
    const estimatedBytesPerValue = 8; // Assume 8 bytes per number
    const timestampBytes = this.historyMetadata.timestamps.length * 8;
    const memoryUsageEstimate = (totalValues * estimatedBytesPerValue) + timestampBytes;
    
    // Calculate capture rate
    let captureRate = 0;
    if (this.historyMetadata.startTime && this.historyMetadata.lastCaptureTime) {
      const duration = (this.historyMetadata.lastCaptureTime - this.historyMetadata.startTime) / 1000; // Convert to seconds
      captureRate = duration > 0 ? totalCaptures / duration : 0;
    }
    
    return {
      totalKeys,
      totalCaptures,
      totalValues,
      averageValuesPerKey,
      memoryUsageEstimate,
      captureRate,
      isCapturing: this.isPolling && this.historyOptions.enabled,
      oldestCaptureTime: this.historyMetadata.startTime,
      newestCaptureTime: this.historyMetadata.lastCaptureTime,
      captureInterval: this.historyOptions.captureInterval,
      maxLength: this.historyOptions.maxLength
    };
  }

  /**
   * Get metadata about the history capture process
   * @returns {Object} History metadata
   */
  getHistoryMetadata() {
    return { ...this.historyMetadata };
  }

  /**
   * Update history capture options
   * @param {Object} options - New history options
   */
  setHistoryOptions(options) {
    this.historyOptions = {
      ...this.historyOptions,
      ...options
    };
    this.log('info', `History options updated: ${JSON.stringify(options)}`);
  }

  // ========================================
  // Cleanup Methods
  // ========================================

  /**
   * Dispose of the animation player and clean up resources
   */
  dispose() {
    this.stopPolling();
    this.eventListeners.clear();
    this.subscribers.clear();
    this.log('info', 'AnimationPlayer disposed');
  }
}
