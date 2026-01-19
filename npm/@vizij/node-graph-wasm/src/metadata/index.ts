import registry from "./registry.js";

import type { NodeSignature, NodeType, Registry } from "../types.js";

type NodeIndex = Map<string, NodeSignature>;

const nodesByType: NodeIndex = new Map(
  registry.nodes.map((node) => [node.type_id.toLowerCase(), node]),
);

/** Return the baked node registry payload (read-only). */
export function getNodeRegistry(): Registry {
  return registry;
}

/**
 * Look up a node signature by type id.
 *
 * The lookup is case-insensitive and returns `undefined` when the id is unknown.
 */
export function findNodeSignature(
  typeId: NodeType | string,
): NodeSignature | undefined {
  return nodesByType.get(typeId.toString().toLowerCase());
}

/**
 * Get a node signature or throw if it is missing.
 *
 * @throws If the node type does not exist in the embedded registry.
 */
export function requireNodeSignature(
  typeId: NodeType | string,
): NodeSignature {
  const entry = findNodeSignature(typeId);
  if (!entry) {
    throw new Error(`Unknown node type '${typeId}' in registry.`);
  }
  return entry;
}

/** List all node type identifiers available in the registry. */
export function listNodeTypeIds(): NodeType[] {
  return registry.nodes.map((node) => node.type_id as NodeType);
}

/**
 * List the node signatures grouped by category.
 *
 * Categories are lowercased and default to `"uncategorized"` if missing.
 */
export function groupNodeSignaturesByCategory(): Map<
  string,
  NodeSignature[]
> {
  const map = new Map<string, NodeSignature[]>();
  for (const node of registry.nodes) {
    const key = (node.category || "uncategorized").toLowerCase();
    const existing = map.get(key);
    if (existing) {
      existing.push(node);
    } else {
      map.set(key, [node]);
    }
  }
  return map;
}

/** Registry data version embedded in the package. */
export const nodeRegistryVersion = registry.version;
