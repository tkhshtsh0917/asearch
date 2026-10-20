export type {
  AmbigContext,
  AmbigPolicy,
  Doc,
  Explanation,
  IndexOptions,
  LoadOptions,
  NormalizerConfig,
  ResourceLimits,
  SearchOptions,
  SearchResult,
  SearchStats,
  SegmentInfo,
  SegmentSource,
} from "./bulk.js";
export {
  AsearchIndex,
  buildDelta,
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

export type { IntentEntry, IntentSearchOptions } from "./intent.js";
export { IntentIndex } from "./intent.js";
export type { Manifest, ManifestDiff, ManifestSegment } from "./manifest.js";
export { diffManifest } from "./manifest.js";
export type { MergedResults, MergePart } from "./merge-results.js";
export { mergeResults } from "./merge-results.js";
