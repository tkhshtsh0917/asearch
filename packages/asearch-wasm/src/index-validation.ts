export function inputTypeError(message: string): TypeError {
  return Object.assign(new TypeError(message), { code: "INVALID_INPUT" });
}

export function fail(code: string, message: string): never {
  throw Object.assign(new Error(message), { code });
}

export function checkKeys(
  value: unknown,
  allowed: readonly string[],
  name: string,
): void {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw inputTypeError(`${name} must be an object`);
  if (
    Reflect.ownKeys(value).some(
      (key) => typeof key !== "string" || !allowed.includes(key),
    )
  )
    fail("INVALID_INPUT", `Unknown ${name} field`);
}

export function integer(
  value: unknown,
  name: string,
  min = 0,
  max = Number.MAX_SAFE_INTEGER,
): number {
  if (typeof value !== "number")
    throw inputTypeError(`${name} must be a number`);
  if (!Number.isSafeInteger(value) || value < min || value > max)
    fail("INVALID_INPUT", `${name} must be an integer in ${min}..${max}`);
  return value;
}
export function boolean(value: unknown, name: string): boolean {
  if (typeof value !== "boolean")
    throw inputTypeError(`${name} must be a boolean`);
  return value;
}

/** Only omitted fields receive defaults; null is an invalid value. */
export function optional<T>(value: T | undefined, fallback: T): T {
  return value === undefined ? fallback : value;
}
