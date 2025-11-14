# Vizij Blackboard - Frontend Implementation Guide for AI Agents

This document provides complete implementation instructions for building a React-based demo application for the Vizij Blackboard WebAssembly API. It is designed to be parsed and executed by AI coding agents.

## Table of Contents

1. [System Overview](#system-overview)
2. [Core Concepts](#core-concepts)
3. [API Reference](#api-reference)
4. [Integration Setup](#integration-setup)
5. [Required Demo Features](#required-demo-features)
6. [Component Implementation Patterns](#component-implementation-patterns)
7. [Implementation Guidelines](#implementation-guidelines)
8. [Error Handling](#error-handling)

---

## System Overview

The Vizij Blackboard is a hierarchical key-value storage system compiled to WebAssembly that provides:

- Dot-separated path notation for nested data (e.g., `robot.arm.joint1.angle`)
- Type-safe storage for JavaScript primitives (number, string, boolean) and homogeneous arrays
- UUID tracking for every node (both path nodes and item nodes)
- Efficient single-threaded implementation optimized for WASM/JavaScript environments
- Complete transparency: all operations return arrays of affected UUIDs

**Key Architecture Points:**
- Every path segment creates a namespace node
- Leaf nodes store actual values
- Every node has a unique UUID
- Leaf UUID is always the LAST element in returned arrays (the deepest node created/updated)
- Removing a path removes ALL child nodes recursively

---

## Core Concepts

### 1. Path Structure

```
robot.arm.joint1.angle
^---- path node (robot)
      ^-- path node (arm)
           ^----- path node (joint1)
                  ^---- item node (angle=45.0)
```

When you call `bb.set("robot.arm.joint1.angle", 45.0)`, the system:
1. Creates or finds path node "robot" (root of this hierarchy)
2. Creates or finds path node "arm" under "robot"
3. Creates or finds path node "joint1" under "arm"
4. Creates or updates item node "angle" under "joint1" with value 45.0 (leaf)
5. Returns array of UUIDs: [robot_id, arm_id, joint1_id, angle_id] where angle_id (last) is the leaf

### 2. Node Types

**Path Nodes:**
- Contain mappings of child names to UUIDs
- Act as namespaces
- Created automatically when setting nested paths

**Item Nodes:**
- Store actual values (primitives or arrays)
- Always leaf nodes in the tree

### 3. UUID Tracking Behavior

**CRITICAL**: All `set()` and `remove()` operations return arrays of UUIDs.

```typescript
// Simple value: single-element array
const [id] = bb.set("counter", 10);

// Nested path: multiple UUIDs, leaf is LAST
const ids = bb.set("robot.arm.angle", 45.0);
const leafId = ids[ids.length - 1];  // The angle item's UUID (leaf node)

// Update existing: returns same structure (includes updated nodes)
const updateIds = bb.set("robot.arm.angle", 50.0);  // Returns all UUIDs again

// Remove: returns all removed UUIDs (including children)
const removedIds = bb.remove("robot.arm");  // Removes arm and ALL descendants
```

### 4. Value Types

**Supported:**
- Number: `42`, `3.14`, `-100.5`
- String: `"hello"`, `"Robot-1"`
- Boolean: `true`, `false`
- Arrays: `[1, 2, 3]`, `["a", "b", "c"]` (must be homogeneous)

**Not Supported:**
- Objects (use nested paths instead)
- Mixed-type arrays
- null (use to remove)
- undefined (use to remove)

---

## API Reference

### Installation

```bash
npm install @vizij/blackboard-wasm
```

### Initialization

```typescript
import { init, VizijBlackboard } from "@vizij/blackboard-wasm";

// Call once at application startup
await init();

// Create instance
const bb = new VizijBlackboard("app-name");
```

### Class: VizijBlackboard

#### Constructor

```typescript
new VizijBlackboard(name: string): VizijBlackboard
```

Creates a blackboard instance.

#### Methods

##### set(path: string, value: any): string[]

Sets a value at the specified path, creating all necessary path nodes.

**Parameters:**
- `path` - Dot-separated string (e.g., `"robot.arm.angle"`)
- `value` - JavaScript primitive or homogeneous array

**Returns:**
- Array of UUID strings for all created/updated nodes
- Leaf item UUID is the LAST element (the deepest node in the path)
- Empty array if value is null/undefined (triggers removal)

**Examples:**

```typescript
// Simple value
const [id] = bb.set("temperature", 72.5);

// Nested path (creates robot/ -> arm/ -> joint1/ -> angle)
const ids = bb.set("robot.arm.joint1.angle", 45.0);
console.log(ids);  // [uuid_robot, uuid_arm, uuid_joint1, uuid_angle]
// Note: uuid_angle (last element) is the leaf item

// Array value
bb.set("positions", [1.0, 2.5, 3.0]);

// Remove by setting null/undefined
bb.set("temp", null);  // Returns []
```

##### get(path: string): any

Retrieves value at the specified path.

**Returns:**
- The stored value (number, string, boolean, or array)
- `undefined` if path doesn't exist

**Examples:**

```typescript
bb.set("temperature", 72.5);
const temp = bb.get("temperature");  // 72.5

const missing = bb.get("nonexistent");  // undefined
```

##### remove(path: string): string[]

Removes a value and ALL child nodes recursively.

**Returns:**
- Array of UUIDs for all removed nodes
- Empty array if path doesn't exist

**Examples:**

```typescript
// Remove single item
bb.set("temp", 72.5);
const [removedId] = bb.remove("temp");

// Remove path with children (removes EVERYTHING under robot.arm)
bb.set("robot.arm.joint1.angle", 45.0);
bb.set("robot.arm.joint2.angle", 30.0);
const removedIds = bb.remove("robot.arm");
// Removes: robot.arm (path), robot.arm.joint1 (path), robot.arm.joint1.angle (item),
//          robot.arm.joint2 (path), robot.arm.joint2.angle (item)
console.log(removedIds.length);  // 5
```

##### has(path: string): boolean

Checks if a path exists.

**Returns:**
- `true` if path or item exists
- `false` otherwise

**Examples:**

```typescript
bb.set("robot.active", true);
bb.has("robot.active");  // true
bb.has("robot");         // true (path node exists)
bb.has("robot.missing"); // false
```

##### clear(): void

Removes all data from the blackboard.

```typescript
bb.clear();
```

##### name(): string

Returns the blackboard's name.

```typescript
const bb = new VizijBlackboard("my-app");
console.log(bb.name());  // "my-app"
```

##### list_paths(): string[]

**Status: NOT IMPLEMENTED** - Returns empty array.

##### size(): number

**Status: NOT IMPLEMENTED** - Returns 0.

### Function: abi_version(): number

Returns ABI version (currently `1`). Used for compatibility checking.

---

## Integration Setup

### Step 1: Create Blackboard Context

Create `src/contexts/BlackboardContext.tsx`:

```typescript
import React, { createContext, useContext, useEffect, useState } from 'react';
import { init, VizijBlackboard } from '@vizij/blackboard-wasm';

interface BlackboardContextType {
  blackboard: VizijBlackboard | null;
  ready: boolean;
}

const BlackboardContext = createContext<BlackboardContextType>({
  blackboard: null,
  ready: false,
});

export const BlackboardProvider: React.FC<{ children: React.ReactNode }> = ({
  children
}) => {
  const [blackboard, setBlackboard] = useState<VizijBlackboard | null>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    const initBlackboard = async () => {
      try {
        await init();
        const bb = new VizijBlackboard("demo-app");
        setBlackboard(bb);
        setReady(true);
      } catch (error) {
        console.error('Failed to initialize blackboard:', error);
      }
    };

    initBlackboard();
  }, []);

  return (
    <BlackboardContext.Provider value={{ blackboard, ready }}>
      {children}
    </BlackboardContext.Provider>
  );
};

export const useBlackboard = () => {
  const context = useContext(BlackboardContext);
  if (!context.blackboard && context.ready) {
    throw new Error('Blackboard not initialized');
  }
  return context;
};
```

### Step 2: Wrap Application

In `src/App.tsx`:

```typescript
import { BlackboardProvider } from './contexts/BlackboardContext';
import { DemoApp } from './components/DemoApp';

function App() {
  return (
    <BlackboardProvider>
      <DemoApp />
    </BlackboardProvider>
  );
}

export default App;
```

### Step 3: Use in Components

```typescript
import { useBlackboard } from '../contexts/BlackboardContext';

export const MyComponent: React.FC = () => {
  const { blackboard, ready } = useBlackboard();

  if (!ready) {
    return <div>Loading...</div>;
  }

  const handleSet = () => {
    if (blackboard) {
      const ids = blackboard.set("example.path", 42);
      console.log('Created UUIDs:', ids);
    }
  };

  return <button onClick={handleSet}>Set Value</button>;
};
```

---

## Required Demo Features

Implement the following features to demonstrate blackboard capabilities:

### Feature 1: Hierarchical Tree Viewer (HIGH PRIORITY)

**Purpose:** Visualize the blackboard's namespace structure in real-time.

**Requirements:**
- Display all paths and items in a tree structure
- Show node types (path vs item) with distinct visual indicators
- Display values for item nodes
- Collapsible/expandable nodes
- Optional UUID display (toggle-able)
- Highlight nodes when they change
- Search/filter functionality

**Implementation Notes:**
- Since `list_paths()` is not implemented, you'll need to track paths manually
- Maintain a state object that mirrors the blackboard structure
- Update on every set/remove operation
- Use a tree UI library (e.g., `react-complex-tree`, `react-arborist`) or build custom

**Data Structure to Maintain:**

```typescript
interface TreeNode {
  name: string;
  path: string;
  uuid: string;
  type: 'path' | 'item';
  value?: any;
  children: Map<string, TreeNode>;
}
```

### Feature 2: Interactive Value Editor (HIGH PRIORITY)

**Purpose:** Allow users to create, read, update, and delete values.

**Requirements:**
- Path input field with validation
- Value input field with type auto-detection
- "Set" button that displays returned UUID array
- "Get" button that retrieves and displays value
- "Remove" button that shows removed UUIDs
- Display results clearly (number of nodes affected, list of UUIDs)
- Show which UUID is the leaf (last element - the deepest node)
- Recent operations log (last 10 operations)

**Type Parsing Logic:**

```typescript
function parseValue(input: string): any {
  // Try number
  if (!isNaN(Number(input)) && input.trim() !== '') {
    return Number(input);
  }
  // Try boolean
  if (input === 'true') return true;
  if (input === 'false') return false;
  // Try array
  if (input.startsWith('[') && input.endsWith(']')) {
    try {
      return JSON.parse(input);
    } catch {
      return input;  // Treat as string
    }
  }
  // Default to string
  return input;
}
```

### Feature 3: UUID Tracker Panel (MEDIUM PRIORITY)

**Purpose:** Demonstrate UUID tracking transparency.

**Requirements:**
- Display last operation's returned UUIDs
- Show count of affected nodes
- Highlight which UUID is the leaf (last element - the target node)
- "Copy UUID" functionality
- Link UUIDs back to their paths in the tree viewer

### Feature 4: Demo Scenario Loader (MEDIUM PRIORITY)

**Purpose:** Quickly populate blackboard with realistic data.

**Requirements:**
- At least 3 pre-defined scenarios
- "Load" button for each scenario
- Clear existing data before loading
- Visual feedback during loading (optional: animate tree updates)

**Scenario 1: Robot Configuration**

```typescript
const robotScenario = {
  "robot.name": "Atlas-V3",
  "robot.model": "humanoid",
  "robot.arm.joint1.angle": 45.0,
  "robot.arm.joint1.torque": 12.5,
  "robot.arm.joint1.max_torque": 20.0,
  "robot.arm.joint2.angle": 30.0,
  "robot.arm.joint2.torque": 8.3,
  "robot.arm.joint2.max_torque": 15.0,
  "robot.leg.joint1.angle": 15.0,
  "robot.leg.joint2.angle": -20.0,
  "robot.status.active": true,
  "robot.status.battery": 87,
  "robot.status.temperature": 68.5,
};
```

**Scenario 2: Application Settings**

```typescript
const settingsScenario = {
  "config.theme": "dark",
  "config.language": "en-US",
  "config.timezone": "America/New_York",
  "config.notifications.email": true,
  "config.notifications.push": false,
  "config.notifications.sms": false,
  "config.display.fps": 60,
  "config.display.resolution": [1920, 1080],
  "config.display.fullscreen": true,
  "config.audio.volume": 75,
  "config.audio.muted": false,
};
```

**Scenario 3: Sensor Data Stream**

```typescript
const sensorScenario = {
  "sensors.temperature.current": 72.5,
  "sensors.temperature.min": 68.0,
  "sensors.temperature.max": 75.0,
  "sensors.temperature.unit": "fahrenheit",
  "sensors.humidity.current": 45,
  "sensors.humidity.comfortable_range": [30, 60],
  "sensors.pressure.current": 1013.25,
  "sensors.pressure.unit": "hPa",
  "sensors.light.lux": 450,
  "sensors.light.natural": true,
  "sensors.readings.count": 1523,
  "sensors.readings.timestamps": [1000, 2000, 3000, 4000, 5000],
};
```

### Feature 5: Operation History Log (MEDIUM PRIORITY)

**Purpose:** Track all operations for debugging and understanding.

**Requirements:**
- Chronological list of operations
- Show: timestamp, operation type (SET/GET/REMOVE/CLEAR), path, value (if applicable)
- Display number of UUIDs affected
- Color-code by operation type
- Filter by operation type
- Clear log button
- Export to JSON

**Log Entry Interface:**

```typescript
interface LogEntry {
  timestamp: Date;
  operation: 'SET' | 'GET' | 'REMOVE' | 'CLEAR';
  path?: string;
  value?: any;
  uuids: string[];
  success: boolean;
  error?: string;
}
```

### Feature 6: Path Search (LOW PRIORITY)

**Purpose:** Find data in large blackboards.

**Requirements:**
- Search input field
- Filter by path substring match
- Highlight matching nodes in tree viewer
- Show count of matches

### Feature 7: Data Export/Import (LOW PRIORITY)

**Purpose:** Demonstrate serialization capabilities.

**Requirements:**
- Export current blackboard to JSON
- Import from JSON file or paste
- Download as file
- Validate imported structure

**Export Format:**

```json
{
  "robot": {
    "name": "Atlas-V3",
    "arm": {
      "joint1": {
        "angle": 45.0,
        "torque": 12.5
      }
    }
  }
}
```

**Import Logic:**

```typescript
function importFromJSON(bb: VizijBlackboard, obj: any, prefix: string = '') {
  for (const [key, value] of Object.entries(obj)) {
    const path = prefix ? `${prefix}.${key}` : key;

    if (typeof value === 'object' && !Array.isArray(value) && value !== null) {
      // Recurse for nested objects
      importFromJSON(bb, value, path);
    } else {
      // Set primitive or array
      bb.set(path, value);
    }
  }
}
```

---

## Component Implementation Patterns

### Pattern 1: Tree Viewer Component

```typescript
// src/components/TreeViewer.tsx
import React, { useState, useEffect } from 'react';
import { useBlackboard } from '../contexts/BlackboardContext';

interface TreeNode {
  name: string;
  path: string;
  uuid: string;
  type: 'path' | 'item';
  value?: any;
  children: Map<string, TreeNode>;
}

export const TreeViewer: React.FC<{
  tree: TreeNode,
  onNodeClick?: (node: TreeNode) => void
}> = ({ tree, onNodeClick }) => {
  const [expanded, setExpanded] = useState(true);
  const [showUuids, setShowUuids] = useState(false);

  const renderNode = (node: TreeNode, depth: number = 0) => {
    const hasChildren = node.children.size > 0;
    const isExpanded = expanded;

    return (
      <div key={node.uuid} style={{ marginLeft: `${depth * 20}px` }}>
        <div
          className={`tree-node ${node.type}`}
          onClick={() => onNodeClick?.(node)}
        >
          {hasChildren && (
            <span onClick={(e) => { e.stopPropagation(); setExpanded(!expanded); }}>
              {isExpanded ? '▼' : '▶'}
            </span>
          )}

          <span className="node-name">{node.name}</span>

          {node.type === 'item' && (
            <span className="node-value">
              {' = '}
              {JSON.stringify(node.value)}
            </span>
          )}

          {showUuids && (
            <span className="node-uuid" title={node.uuid}>
              {node.uuid.substring(0, 8)}...
            </span>
          )}
        </div>

        {hasChildren && isExpanded && (
          <div className="children">
            {Array.from(node.children.values()).map(child =>
              renderNode(child, depth + 1)
            )}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="tree-viewer">
      <div className="controls">
        <button onClick={() => setShowUuids(!showUuids)}>
          {showUuids ? 'Hide' : 'Show'} UUIDs
        </button>
      </div>
      {renderNode(tree)}
    </div>
  );
};
```

### Pattern 2: Value Editor Component

```typescript
// src/components/ValueEditor.tsx
import React, { useState } from 'react';
import { useBlackboard } from '../contexts/BlackboardContext';

interface OperationResult {
  success: boolean;
  uuids: string[];
  error?: string;
}

export const ValueEditor: React.FC<{
  onOperation?: (result: OperationResult) => void
}> = ({ onOperation }) => {
  const { blackboard } = useBlackboard();
  const [path, setPath] = useState('');
  const [value, setValue] = useState('');
  const [result, setResult] = useState<OperationResult | null>(null);

  const parseValue = (input: string): any => {
    if (!isNaN(Number(input)) && input.trim() !== '') {
      return Number(input);
    }
    if (input === 'true') return true;
    if (input === 'false') return false;
    if (input.startsWith('[') && input.endsWith(']')) {
      try {
        return JSON.parse(input);
      } catch {
        return input;
      }
    }
    return input;
  };

  const handleSet = () => {
    if (!blackboard || !path) return;

    try {
      const parsedValue = parseValue(value);
      const uuids = blackboard.set(path, parsedValue);
      const res = { success: true, uuids };
      setResult(res);
      onOperation?.(res);
    } catch (error) {
      const res = {
        success: false,
        uuids: [],
        error: (error as Error).message
      };
      setResult(res);
      onOperation?.(res);
    }
  };

  const handleGet = () => {
    if (!blackboard || !path) return;

    try {
      const val = blackboard.get(path);
      setValue(JSON.stringify(val));
    } catch (error) {
      setResult({
        success: false,
        uuids: [],
        error: (error as Error).message
      });
    }
  };

  const handleRemove = () => {
    if (!blackboard || !path) return;

    try {
      const uuids = blackboard.remove(path);
      const res = { success: true, uuids };
      setResult(res);
      onOperation?.(res);
      setPath('');
    } catch (error) {
      const res = {
        success: false,
        uuids: [],
        error: (error as Error).message
      };
      setResult(res);
      onOperation?.(res);
    }
  };

  return (
    <div className="value-editor">
      <h3>Value Editor</h3>

      <div className="input-group">
        <label>Path:</label>
        <input
          type="text"
          value={path}
          onChange={(e) => setPath(e.target.value)}
          placeholder="e.g., robot.arm.angle"
        />
      </div>

      <div className="input-group">
        <label>Value:</label>
        <input
          type="text"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder="42, 'text', true, [1,2,3]"
        />
      </div>

      <div className="button-group">
        <button onClick={handleSet}>Set</button>
        <button onClick={handleGet}>Get</button>
        <button onClick={handleRemove}>Remove</button>
      </div>

      {result && (
        <div className={`result ${result.success ? 'success' : 'error'}`}>
          {result.success ? (
            <>
              <h4>Success: {result.uuids.length} node(s) affected</h4>
              <div className="uuids">
                {result.uuids.map((uuid, idx) => (
                  <div key={uuid} className="uuid-item">
                    {idx === result.uuids.length - 1 && <strong>(leaf) </strong>}
                    {uuid}
                  </div>
                ))}
              </div>
            </>
          ) : (
            <div className="error-message">{result.error}</div>
          )}
        </div>
      )}
    </div>
  );
};
```

### Pattern 3: Scenario Loader Component

```typescript
// src/components/ScenarioLoader.tsx
import React, { useState } from 'react';
import { useBlackboard } from '../contexts/BlackboardContext';

interface Scenario {
  id: string;
  name: string;
  description: string;
  data: Record<string, any>;
}

const SCENARIOS: Scenario[] = [
  {
    id: 'robot',
    name: 'Robot Configuration',
    description: 'Complete robot setup with joints and sensors',
    data: {
      "robot.name": "Atlas-V3",
      "robot.arm.joint1.angle": 45.0,
      "robot.arm.joint1.torque": 12.5,
      "robot.arm.joint2.angle": 30.0,
      "robot.status.active": true,
      "robot.status.battery": 87,
    }
  },
  // Add other scenarios...
];

export const ScenarioLoader: React.FC<{
  onLoad?: () => void
}> = ({ onLoad }) => {
  const { blackboard } = useBlackboard();
  const [loading, setLoading] = useState(false);

  const loadScenario = async (scenario: Scenario) => {
    if (!blackboard) return;

    setLoading(true);

    try {
      // Clear existing data
      blackboard.clear();

      // Load scenario data
      for (const [path, value] of Object.entries(scenario.data)) {
        blackboard.set(path, value);
        // Optional: small delay for visual effect
        await new Promise(resolve => setTimeout(resolve, 50));
      }

      onLoad?.();
    } catch (error) {
      console.error('Failed to load scenario:', error);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="scenario-loader">
      <h3>Demo Scenarios</h3>
      <div className="scenarios">
        {SCENARIOS.map(scenario => (
          <div key={scenario.id} className="scenario-card">
            <h4>{scenario.name}</h4>
            <p>{scenario.description}</p>
            <button
              onClick={() => loadScenario(scenario)}
              disabled={loading}
            >
              {loading ? 'Loading...' : 'Load'}
            </button>
          </div>
        ))}
      </div>
    </div>
  );
};
```

### Pattern 4: Operation Logger

```typescript
// src/hooks/useOperationLogger.ts
import { useState, useCallback } from 'react';

interface LogEntry {
  id: string;
  timestamp: Date;
  operation: 'SET' | 'GET' | 'REMOVE' | 'CLEAR';
  path?: string;
  value?: any;
  uuids: string[];
  success: boolean;
  error?: string;
}

export const useOperationLogger = () => {
  const [logs, setLogs] = useState<LogEntry[]>([]);

  const logOperation = useCallback((entry: Omit<LogEntry, 'id' | 'timestamp'>) => {
    const newEntry: LogEntry = {
      ...entry,
      id: `${Date.now()}-${Math.random()}`,
      timestamp: new Date(),
    };
    setLogs(prev => [newEntry, ...prev].slice(0, 100)); // Keep last 100
  }, []);

  const clearLogs = useCallback(() => {
    setLogs([]);
  }, []);

  const exportLogs = useCallback(() => {
    const json = JSON.stringify(logs, null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `blackboard-logs-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }, [logs]);

  return { logs, logOperation, clearLogs, exportLogs };
};
```

---

## Implementation Guidelines

### State Management

**Track Tree Structure:**
Since `list_paths()` is not implemented, maintain tree state manually:

```typescript
// src/hooks/useBlackboardTree.ts
import { useState, useCallback } from 'react';

interface TreeNode {
  name: string;
  path: string;
  uuid: string;
  type: 'path' | 'item';
  value?: any;
  children: Map<string, TreeNode>;
}

export const useBlackboardTree = () => {
  const [root, setRoot] = useState<TreeNode>({
    name: 'root',
    path: '',
    uuid: 'root',
    type: 'path',
    children: new Map(),
  });

  const updateTree = useCallback((path: string, value: any, uuids: string[]) => {
    const parts = path.split('.');

    setRoot(prevRoot => {
      const newRoot = { ...prevRoot, children: new Map(prevRoot.children) };
      let current = newRoot;

      // Traverse/create path nodes
      for (let i = 0; i < parts.length - 1; i++) {
        const part = parts[i];
        const currentPath = parts.slice(0, i + 1).join('.');

        if (!current.children.has(part)) {
          current.children.set(part, {
            name: part,
            path: currentPath,
            uuid: uuids[i] || `path-${currentPath}`,
            type: 'path',
            children: new Map(),
          });
        }

        current = current.children.get(part)!;
      }

      // Set item node
      const itemName = parts[parts.length - 1];
      const itemPath = path;
      current.children.set(itemName, {
        name: itemName,
        path: itemPath,
        uuid: uuids[uuids.length - 1],
        type: 'item',
        value,
        children: new Map(),
      });

      return newRoot;
    });
  }, []);

  const removeFromTree = useCallback((path: string) => {
    const parts = path.split('.');

    setRoot(prevRoot => {
      const newRoot = { ...prevRoot, children: new Map(prevRoot.children) };

      if (parts.length === 1) {
        newRoot.children.delete(parts[0]);
        return newRoot;
      }

      let current = newRoot;
      for (let i = 0; i < parts.length - 1; i++) {
        const part = parts[i];
        if (!current.children.has(part)) return prevRoot;
        current = current.children.get(part)!;
      }

      current.children.delete(parts[parts.length - 1]);
      return newRoot;
    });
  }, []);

  const clearTree = useCallback(() => {
    setRoot({
      name: 'root',
      path: '',
      uuid: 'root',
      type: 'path',
      children: new Map(),
    });
  }, []);

  return { root, updateTree, removeFromTree, clearTree };
};
```

### Type Safety

Use TypeScript interfaces for all data structures:

```typescript
// src/types/blackboard.ts

export interface BlackboardNode {
  name: string;
  path: string;
  uuid: string;
  type: 'path' | 'item';
  value?: BlackboardValue;
}

export type BlackboardValue = number | string | boolean | number[] | string[] | boolean[];

export interface OperationResult {
  success: boolean;
  uuids: string[];
  error?: string;
}

export interface LogEntry {
  id: string;
  timestamp: Date;
  operation: 'SET' | 'GET' | 'REMOVE' | 'CLEAR';
  path?: string;
  value?: BlackboardValue;
  uuids: string[];
  success: boolean;
  error?: string;
}
```

### Error Handling

Wrap all blackboard operations in try-catch:

```typescript
const safeSet = (bb: VizijBlackboard, path: string, value: any): OperationResult => {
  try {
    const uuids = bb.set(path, value);
    return { success: true, uuids };
  } catch (error) {
    return {
      success: false,
      uuids: [],
      error: error instanceof Error ? error.message : 'Unknown error',
    };
  }
};
```

### Performance

- Debounce rapid updates (especially in tree viewer)
- Use React.memo for tree nodes
- Virtualize large trees (use `react-window` or `react-virtual`)
- Batch scenario loading with small delays for visual effect

---

## Error Handling

### Common Errors and Solutions

**Error: "Path already exists as a BBPath node, cannot set it with a Value"**

```typescript
// Wrong: trying to set a value where a path exists
bb.set("robot.arm", 45.0);  // If robot.arm.angle exists

// Solution: use different path or remove existing path first
bb.remove("robot.arm");
bb.set("robot.arm", 45.0);
```

**Error: "Path already exists as an Item with id XXX, cannot set KeyValue structure here"**

```typescript
// Wrong: trying to create nested path where item exists
bb.set("robot.arm", 45.0);
bb.set("robot.arm.angle", 30.0);  // Error!

// Solution: remove item first
bb.remove("robot.arm");
bb.set("robot.arm.angle", 30.0);
```

**Type Conversion Issues:**

```typescript
// Ensure proper type parsing
const parseValue = (input: string): any => {
  // Number check
  const num = Number(input);
  if (!isNaN(num) && input.trim() !== '') {
    return num;
  }

  // Boolean check
  if (input === 'true') return true;
  if (input === 'false') return false;

  // Array check
  if (input.startsWith('[') && input.endsWith(']')) {
    try {
      const parsed = JSON.parse(input);
      if (Array.isArray(parsed)) {
        // Verify homogeneous
        if (parsed.length > 0) {
          const firstType = typeof parsed[0];
          if (parsed.every(item => typeof item === firstType)) {
            return parsed;
          }
        }
      }
    } catch {
      // Fall through to string
    }
  }

  // Default to string
  return input;
};
```

### Validation Helpers

```typescript
const isValidPath = (path: string): boolean => {
  return /^[a-zA-Z0-9_]+(\.[a-zA-Z0-9_]+)*$/.test(path);
};

const isHomogeneousArray = (arr: any[]): boolean => {
  if (arr.length === 0) return true;
  const firstType = typeof arr[0];
  return arr.every(item => typeof item === firstType);
};

const validateValue = (value: any): { valid: boolean; error?: string } => {
  const validTypes = ['number', 'string', 'boolean'];

  if (Array.isArray(value)) {
    if (!isHomogeneousArray(value)) {
      return { valid: false, error: 'Arrays must be homogeneous' };
    }
    return { valid: true };
  }

  if (!validTypes.includes(typeof value)) {
    return { valid: false, error: `Unsupported type: ${typeof value}` };
  }

  return { valid: true };
};
```

---

## Implementation Checklist

### Phase 1: Core Setup
- [ ] Install @vizij/blackboard-wasm
- [ ] Create BlackboardContext with initialization
- [ ] Create useBlackboard hook
- [ ] Verify WASM loads correctly
- [ ] Test basic set/get/remove operations

### Phase 2: Essential Components
- [ ] Implement ValueEditor component
- [ ] Implement tree state management (useBlackboardTree)
- [ ] Implement basic TreeViewer component
- [ ] Connect ValueEditor to update tree state
- [ ] Add operation result display (UUIDs)

### Phase 3: Enhanced Features
- [ ] Implement ScenarioLoader with 3 scenarios
- [ ] Implement operation logger (useOperationLogger)
- [ ] Add OperationLog component
- [ ] Add UUID tracker panel
- [ ] Implement search/filter for tree

### Phase 4: Polish
- [ ] Add loading states
- [ ] Add error boundaries
- [ ] Implement data export/import
- [ ] Add keyboard shortcuts
- [ ] Style with CSS (clean, modern look)
- [ ] Add responsive layout
- [ ] Test all scenarios thoroughly

---

## Final Notes for AI Agents

**Key Implementation Priorities:**

1. **Tree State Management:** This is critical since list_paths() is not implemented. You must track the tree structure yourself.

2. **UUID Handling:** Always remember the leaf UUID is the LAST element in the returned array (the deepest/target node). Display this prominently.

3. **Error Handling:** Wrap all blackboard operations. The WASM can throw errors for invalid operations.

4. **Type Parsing:** Implement robust type detection for the value editor. Users will input strings that need parsing.

5. **Recursive Removal:** When demonstrating remove(), show that it deletes ALL children. This is important behavior to showcase.

**Testing Approach:**

1. Start with simple values (numbers, strings)
2. Test nested paths (robot.arm.angle)
3. Test arrays (homogeneous only)
4. Test remove on paths with children
5. Test clear operation
6. Load each scenario and verify tree structure
7. Test rapid operations (ensure no race conditions)

**Code Quality:**

- Use TypeScript strictly
- Add PropTypes or interfaces for all components
- Use functional components with hooks
- Follow React best practices (keys, memoization, etc.)
- Add comments for complex logic
- Keep components focused (single responsibility)

**UI/UX:**

- Clean, minimal design
- Clear visual feedback for operations
- Loading states for async operations
- Error messages that are user-friendly
- Tooltips for UUIDs and technical details
- Responsive layout (mobile-friendly)

This guide provides everything needed to build a complete demo application. Follow the patterns, implement the required features, and test thoroughly.
