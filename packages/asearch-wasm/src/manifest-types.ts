export interface ManifestSegment {
  id: string;
  url: string;
  bytes: number;
  texts: number;
  integrity: string;
}
export interface Manifest {
  format: "asearch-manifest/1";
  indexId: string;
  generation: number;
  index: {
    major: number;
    minor: number;
    symbol_unit: "code-point" | "utf16";
    normalizer: { id: "none" | "ja-basic"; version: number; options: number };
  };
  segments: ManifestSegment[];
}
export type ManifestDiff =
  | { kind: "stale" }
  | { kind: "none" }
  | { kind: "append"; add: ManifestSegment[] }
  | { kind: "replace"; segments: ManifestSegment[] };
