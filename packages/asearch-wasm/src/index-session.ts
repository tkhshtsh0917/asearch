import type { SearchResult } from "./index-result.js";
import { fail, inputTypeError } from "./index-validation.js";

export interface SessionBackend {
  update(pattern: string): SearchResult;
  reset(): void;
  free(): void;
}
interface State {
  backend: SessionBackend;
  busy: boolean;
}
const states = new WeakMap<SearchSession, State | null>();
const finalizer = new FinalizationRegistry<SessionBackend>((backend) =>
  backend.free(),
);
const secret = Symbol("SearchSession factory");

function state(receiver: SearchSession): State {
  const value = states.get(receiver);
  if (value === null) fail("DISPOSED", "SearchSession has been disposed");
  if (!value) throw inputTypeError("Invalid SearchSession receiver");
  return value;
}

/** Stateful input with its own bounded Rust workspace over a shared corpus. */
export class SearchSession {
  private constructor(backend: SessionBackend, token: symbol) {
    if (token !== secret) throw inputTypeError("Use index.session()");
    states.set(this, { backend, busy: false });
    finalizer.register(this, backend, this);
  }
  /** @internal */
  static create(backend: SessionBackend): SearchSession {
    return new SearchSession(backend, secret);
  }
  update(pattern: string): SearchResult {
    const value = state(this);
    if (value.busy)
      fail("CONFLICT", "SearchSession update is already in progress");
    value.busy = true;
    try {
      return value.backend.update(pattern);
    } finally {
      value.busy = false;
    }
  }
  reset(): void {
    const value = state(this);
    if (value.busy)
      fail("CONFLICT", "SearchSession update is already in progress");
    value.backend.reset();
  }
  free(): void {
    const value = states.get(this);
    if (value === undefined)
      throw inputTypeError("Invalid SearchSession receiver");
    if (value === null) return;
    states.set(this, null);
    finalizer.unregister(this);
    value.backend.free();
  }
  declare [Symbol.dispose]: () => void;
}
Object.defineProperty(
  SearchSession.prototype,
  Symbol.dispose ?? Symbol.for("Symbol.dispose"),
  {
    value: SearchSession.prototype.free,
    configurable: true,
    writable: true,
  },
);
