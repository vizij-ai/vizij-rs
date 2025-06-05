# Animation Player Architecture

This document provides a detailed overview of the animation player architecture, focusing on the AnimationEngine and AnimationPlayer components and their relationship in managing individual animations.

## Overview

The animation system follows a hierarchical architecture where the **AnimationEngine** manages multiple **AnimationPlayers**, and each player manages multiple **AnimationInstances**. This design allows for complex multi-layered animations with independent timing, looping, and blending capabilities.

## AnimationPlayer Architecture

```mermaid
graph TB
    subgraph "AnimationPlayer"
        AP[Animation Player]
        
        subgraph "Core State"
            ID[id: String]
            CurrentTime[current_time: AnimationTime]
            Metrics[metrics: PlaybackMetrics]
        end
        
        subgraph "Instance Management"
            Instances[instances: HashMap<String, AnimationInstance>]
            AddInst[add_instance]
            RemoveInst[remove_instance]
        end
        
        subgraph "Time Operations"
            GoTo[go_to]
            Increment[increment]
            Decrement[decrement]
        end
        
        subgraph "Value Calculation"
            CalcValues[calculate_values]
            InterpTrack[interpolate_track_value_for_instance]
            GetEffTime[get_effective_time_for_track]
        end
        
        subgraph "Animation Instances"
            AI1[AnimationInstance 1]
            AI2[AnimationInstance 2]
            AI3[AnimationInstance N...]
            
            subgraph "Instance Components"
                InstSettings[InstanceSettings]
                LoopState[Loop State]
                EffectiveTime[Effective Time Calculation]
            end
        end
    end
    
    AP --> ID
    AP --> CurrentTime
    AP --> Metrics
    AP --> Instances
    
    Instances --> AI1
    Instances --> AI2
    Instances --> AI3
    
    AI1 --> InstSettings
    AI1 --> LoopState
    AI1 --> EffectiveTime
    
    GoTo --> CalcValues
    Increment --> CalcValues
    Decrement --> CalcValues
    
    CalcValues --> InterpTrack
    CalcValues --> Instances
    
    style AP fill:#e1f5fe
    style CalcValues fill:#fff3e0
    style Instances fill:#f3e5f5
    style AI1 fill:#e8f5e8
    style AI2 fill:#e8f5e8
    style AI3 fill:#e8f5e8
```

## Engine-Player Relationship and Animation Management

```mermaid
graph TB
    subgraph "Animation System Flow"
        subgraph "AnimationEngine Level"
            Engine[AnimationEngine]
            
            subgraph "Shared Resources"
                AnimData[AnimationData Storage]
                InterpRegistry[InterpolationRegistry]
                EventSystem[EventDispatcher]
            end
            
            subgraph "Engine Update Loop"
                FrameDelta[Frame Delta Input]
                UpdateLoop[update Method]
                PlayerIteration[Iterate Over Players]
                StateCheck[Check Player State]
                TimeCalculation[Calculate Animation Delta]
                BoundsHandling[Handle Time Bounds/Looping]
                CollectValues[Collect All Values]
            end
        end
        
        subgraph "Player Level"
            Player1[AnimationPlayer 1]
            Player2[AnimationPlayer 2]
            PlayerN[AnimationPlayer N]
            
            subgraph "Player Components"
                PlayerState[PlayerState]
                PlayerTime[Current Time]
                PlayerInstances[Animation Instances]
            end
        end
        
        subgraph "Instance Level"
            subgraph "Instance 1"
                Inst1[AnimationInstance]
                Settings1[InstanceSettings]
                LoopState1[Loop State]
                EffTime1[Effective Time]
            end
            
            subgraph "Instance 2"
                Inst2[AnimationInstance]
                Settings2[InstanceSettings]
                LoopState2[Loop State]
                EffTime2[Effective Time]
            end
        end
        
        subgraph "Animation Data"
            AnimData1[AnimationData 1]
            AnimData2[AnimationData 2]
            
            subgraph "Data Components"
                Tracks[Animation Tracks]
                Keypoints[Keypoints]
                Transitions[Transitions]
            end
        end
    end
    
    %% Flow connections
    Engine --> AnimData
    Engine --> InterpRegistry
    Engine --> EventSystem
    
    FrameDelta --> UpdateLoop
    UpdateLoop --> PlayerIteration
    PlayerIteration --> Player1
    PlayerIteration --> Player2
    PlayerIteration --> PlayerN
    
    Player1 --> PlayerState
    Player1 --> PlayerTime
    Player1 --> PlayerInstances
    
    PlayerInstances --> Inst1
    PlayerInstances --> Inst2
    
    Inst1 --> Settings1
    Inst1 --> LoopState1
    Inst1 --> EffTime1
    
    Settings1 --> AnimData1
    Settings2 --> AnimData2
    
    AnimData1 --> Tracks
    AnimData1 --> Keypoints
    AnimData1 --> Transitions
    
    StateCheck --> TimeCalculation
    TimeCalculation --> BoundsHandling
    BoundsHandling --> CollectValues
    
    style Engine fill:#e1f5fe
    style UpdateLoop fill:#fff3e0
    style Player1 fill:#f3e5f5
    style Inst1 fill:#e8f5e8
    style AnimData1 fill:#fff9c4
```

## Individual Animation Update Flow

```mermaid
sequenceDiagram
    participant Engine as AnimationEngine
    participant Player as AnimationPlayer
    participant Instance as AnimationInstance
    participant AnimData as AnimationData
    participant InterpReg as InterpolationRegistry
    
    Note over Engine: Frame Update Begins
    Engine->>Engine: Calculate frame delta
    
    loop For each player
        Engine->>Player: Check player state
        alt Player is Playing
            Engine->>Player: Calculate animation delta
            Engine->>Player: Update player time
            
            Player->>Player: calculate_values()
            
            loop For each active instance
                Player->>Instance: Check if instance is active
                Player->>Instance: update_loop_state()
                Player->>Instance: get_effective_time()
                
                Instance->>Instance: Apply timescale
                Instance->>Instance: Handle looping mode
                Instance->>Instance: Add start offset
                
                Player->>AnimData: Get animation data by ID
                
                loop For each track
                    Player->>AnimData: Get track transition
                    Player->>AnimData: Call track.value_at_time()
                    AnimData->>InterpReg: Interpolate keypoints
                    InterpReg-->>AnimData: Return interpolated value
                    AnimData-->>Player: Return track value
                end
                
                Player->>Player: Combine instance values
            end
            
            Player->>Player: Update metrics
            Player-->>Engine: Return combined values
        else Player is Paused/Stopped
            Player->>Player: Return cached values
            Player-->>Engine: Return values
        end
    end
    
    Engine->>Engine: Update engine metrics
    Engine-->>Engine: Return all player values
```

## Key Concepts

### AnimationEngine Responsibilities

- **Player Lifecycle Management**: Creates, manages, and destroys animation players
- **Resource Management**: Loads and caches animation data, manages interpolation registry
- **Global Playback Control**: Provides play/pause/stop/seek operations for individual players
- **Performance Monitoring**: Tracks engine-wide metrics and performance
- **Event Coordination**: Manages event dispatching across the system

### AnimationPlayer Responsibilities

- **Instance Management**: Manages multiple animation instances with different settings
- **Time Coordination**: Maintains current playback time and handles time-based operations
- **Value Calculation**: Combines values from all active instances
- **Player-level Metrics**: Tracks performance metrics for this specific player

### AnimationInstance Responsibilities

- **Individual Animation Control**: Manages settings for a specific animation (timescale, looping, offsets)
- **Time Mapping**: Converts player time to effective animation time
- **Loop State Management**: Handles loop counting and ping-pong direction
- **Animation Data Reference**: Links to specific animation data by ID

### Value Calculation Flow

1. **Engine Update**: Called with frame delta time
2. **Player Iteration**: Engine iterates through all players
3. **State Check**: Check if player is in Playing state
4. **Time Calculation**: Apply speed multiplier and calculate new time
5. **Bounds Handling**: Handle looping/ping-pong at player level
6. **Instance Processing**: For each active instance:
   - Check if instance should be active at current time
   - Update instance loop state
   - Calculate effective time (apply timescale, handle instance-level looping, add offset)
   - Interpolate values from animation data
7. **Value Combination**: Combine values from all instances (currently simple overwrite, future: blending)
8. **Metrics Update**: Update player and engine metrics

### Multi-layer Animation Support

The architecture supports complex multi-layer animations through:

- **Multiple Instances per Player**: Each instance can reference different animation data
- **Independent Timing**: Each instance has its own timescale, start time, and duration
- **Flexible Looping**: Per-instance loop modes (Once, Loop, PingPong)
- **Offset Support**: Start offset allows instances to begin from any point in their animation
- **Blending Ready**: Architecture prepared for future blending between instances

This design enables scenarios like:

- Playing multiple animations simultaneously on the same object
- Layering animations with different timing characteristics
- Creating complex composite animations from simpler building blocks
- Managing large numbers of independent animations efficiently
