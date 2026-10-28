export interface MigrationWeights {
  alpha: number;
  beta: number;
  gamma: number;
}

export type MigrationOrderMode = "scheduled" | "priority";

export interface MigrationCommunityNode {
  id: number;
  label: string;
  member_count: number;
  avg_pagerank: number;
  avg_harmonic: number;
  avg_betweenness: number;
  max_blast: number;
  /** Fast-Leiden majority vote (schema v3+). */
  community_id?: number | null;
  /** Schema v2 name; still accepted when reading older migration_graph.json. */
  louvain_community_id?: number | null;
}

/** Resolve cluster id from a migration macro node (v3 `community_id` or v2 alias). */
export function migrationCommunityClusterId(
  node: Pick<MigrationCommunityNode, "id" | "community_id" | "louvain_community_id">,
): number {
  return node.community_id ?? node.louvain_community_id ?? node.id;
}

export interface MigrationCommunityEdge {
  source: number;
  target: number;
  weight: number;
  kind: string;
}

export interface MigrationGraphPayload {
  schema_version: number;
  mode?: string;
  modularity: number;
  communities: MigrationCommunityNode[];
  edges: MigrationCommunityEdge[];
}

export interface MigrationPlanStep {
  step: number;
  community_id: number;
  label: string;
  priority_score: number;
  schedule_step: number;
  priority_rank: number;
  avg_pagerank: number;
  avg_harmonic: number;
  max_blast: number;
}

export interface MigrationPlanPayload {
  schema_version: number;
  preset: string;
  preset_label: string;
  weights: MigrationWeights;
  order_mode: MigrationOrderMode;
  steps: MigrationPlanStep[];
}

export type MigrationPresetId =
  | "hybrid_default"
  | "foundational_first"
  | "dense_cluster"
  | "risk_mitigation"
  | "custom";
