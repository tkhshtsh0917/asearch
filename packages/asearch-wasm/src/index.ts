export type {
  AmbigPolicy,
  Doc,
  IndexOptions,
  LoadOptions,
  NormalizerConfig,
  ResourceLimits,
  SearchOptions,
  SearchResult,
  SearchStats,
} from "./bulk.js";
export {
  AsearchIndex,
  buildIndex,
  byLength,
  fixed,
  SearchSession,
} from "./bulk.js";
export type { InitInput, Options } from "./core.js";
export { Asearch, MatchMode } from "./core.js";

import type { InitInput } from "./core.js";
export declare let ready: Promise<void>;
export declare function init(input?: InitInput): Promise<void>;
