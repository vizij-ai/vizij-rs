// Engine-related constants
pub mod mc_const {
    // Default tick rate (Hz)
    pub const DEFAULT_TPS: u32 = 100;
    pub const DEFAULT_BROADCAST_TPS: u32 = 30;

    pub const KEY_ENGINE_PREFIX: &str = "engine";
    // Blackboard key for engine ticks per second
    pub const KEY_ENGINE_TARGET_TPS: &str = "engine.target_tps";
    pub const KEY_ENGINE_TARGET_BROADCAST_TPS: &str = "engine.target_broadcast_tps";
    pub const KEY_ENGINE_MEASURED_TPS: &str = "engine.measured_tps";
    pub const KEY_ENGINE_MEASURED_BROADCAST_TPS: &str = "engine.measured_broadcast_tps";

    // Blackboard key for engine status
    pub const KEY_ENGINE_TICK: &str = "engine.tick";

    // Blackboard key for engine uptime
    pub const KEY_ENGINE_TIME: &str = "engine.time";

    // Blackboard key for embodiment
    pub const KEY_ENG_EMBODIMENT: &str = "engine.embodiment";
    pub const KEY_ENG_EMBODIMENT_NAME: &str = "engine.embodiment.name";

    // Blackboard key for engine current state
    pub const KEY_ENGINE_CURRENT_STATE: &str = "engine.current_state";

    // Blackboard key for embodiment outputs configuration
    // Each output adds an entry to output_names, and a node at engine.output.<name>
    pub const KEY_ENG_OUTPUT_NAMES: &str = "engine.output_names";
    pub const KEY_ENG_OUTPUT_PREFIX: &str = "engine.output";

    // Blackboard keys for engine output parts
    // E.g. "engine.output.<name>.type"
    pub const KEY_ENG_OUTPUT_TYPE_SUFFIX: &str = "type";
    pub const KEY_ENG_OUTPUT_REST_SUFFIX: &str = "rest";
    pub const KEY_ENG_OUTPUT_MIN_SUFFIX: &str = "min";
    pub const KEY_ENG_OUTPUT_MAX_SUFFIX: &str = "max";
    pub const KEY_ENG_OUTPUT_VALUE_SUFFIX: &str = "value";

    // Prefix for engine output values in the blackboard
    pub const KEY_ENG_OUTPUT_VALUE_PREFIX: &str = "engine.out";

    // Blackboard prefix for events
    pub const KEY_EVENT_PREFIX: &str = "event";
    pub const KEY_EVENT_TRIGGERED_SUFFIX: &str = "triggered";
    pub const KEY_EVENT_ARGS_SUFFIX: &str = "args";
    pub const KEY_EVENT_FIRE_SUFFIX: &str = "fire";

    // Blackboard prefix for state
    pub const KEY_STATE_PREFIX: &str = "state";
    pub const KEY_STATE_ACTIVE_SUFFIX: &str = "active";
    pub const KEY_STATE_ENTERED_SUFFIX: &str = "entered";
    pub const KEY_STATE_EXITED_SUFFIX: &str = "exited";
    pub const KEY_STATE_EVENTS_ALLOWED_SUFFIX: &str = "events_allowed";

    // Blackboard prefix for nodes
    pub const KEY_GRAPH_PREFIX: &str = "graph";
    pub const KEY_NODE_ACTIVE_SUFFIX: &str = "active";
    pub const KEY_NODE_TYPE_SUFFIX: &str = "type";
    pub const KEY_NODE_IN_SUFFIX: &str = "in";
    pub const KEY_NODE_OUT_SUFFIX: &str = "out";
    pub const KEY_NODE_INPLUG_SUFFIX: &str = "in_plug";
    pub const KEY_NODE_OUTPLUG_SUFFIX: &str = "out_plug";

    // Blackboard prefix for PlugIn nodes
    pub const KEY_NODE_PLUGIN_REF: &str = "node_ref";
    pub const KEY_NODE_PLUGIN_VAR: &str = "var_name";

    pub const KEY_NODE_OUTPUT_VAR: &str = "output";
}

// Daemon-related constants
pub mod mcd_const {
    // WebSocket related constants
    pub const DEFAULT_PORT: u16 = 8080;
    pub const DEFAULT_HOST: &str = "127.0.0.1";
}
