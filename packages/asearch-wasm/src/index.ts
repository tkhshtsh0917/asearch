export type {
  AmbigPolicy,
  Doc,
  IndexOptions,
  NormalizerConfig,
  ResourceLimits,
  SearchOptions,
  SearchResult,
  SearchStats,
} from "./bulk.js";
export { AsearchIndex, buildIndex, byLength, fixed } from "./bulk.js";
export type { InitInput, Options } from "./core.js";
export { Asearch, MatchMode } from "./core.js";

import type { InitInput } from "./core.js";
export declare let ready: Promise<void>;
export declare function init(input?: InitInput): Promise<void>;
