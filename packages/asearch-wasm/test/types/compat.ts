import * as Up from "../../../../vendor/upstream/asearch.ts/lib/index.js";
import * as Ours from "../../dist/index.js";

const i: Pick<Up.Asearch, keyof Up.Asearch> = new Ours.Asearch("a");
const c: {
  new (
    pattern: string,
    mode?: Up.MatchMode,
  ): Pick<Up.Asearch, keyof Up.Asearch>;
  MAXLENGTH: number;
} = Ours.Asearch;
const m: Up.MatchMode = Ours.MatchMode.Include;
const reverse: Ours.MatchMode = Up.MatchMode.Include;
const upstreamInstance: Pick<Up.Asearch, keyof Up.Asearch> = new Up.Asearch(
  "a",
);
void [i, c, m, reverse, upstreamInstance];
const ideal = new Ours.Asearch("a", Ours.MatchMode.Exact, { profile: "ideal" });
ideal.free();
