type RawSearchParams = { [key: string]: string | string[] | undefined };

/** Extract a string param, returning undefined if missing or not a string. */
export function paramString(params: RawSearchParams, key: string): string | undefined {
  const v = params[key];
  return typeof v === "string" ? v : undefined;
}

/** Extract a string param with a default value. */
export function paramStringOr(params: RawSearchParams, key: string, defaultValue: string): string {
  return paramString(params, key) ?? defaultValue;
}

/** Extract a numeric param, returning a default if missing or not parseable. */
export function paramInt(params: RawSearchParams, key: string, defaultValue: number): number {
  const v = params[key];
  if (typeof v !== "string") return defaultValue;
  const n = parseInt(v, 10);
  return Number.isNaN(n) ? defaultValue : n;
}

/** Extract a boolean param (true only if value is exactly "true"). */
export function paramBool(params: RawSearchParams, key: string): boolean {
  return params[key] === "true";
}
