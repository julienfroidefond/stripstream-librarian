/**
 * One draft and one write policy for the `app_settings` sections.
 *
 * A draft holds the current value of every settings key, the last value the
 * server acknowledged, and the per-key save status. Callers only patch the
 * slice they edit; the module owns:
 *
 * - merge semantics (`patch` keeps sibling fields, `set` replaces a scalar),
 * - dirty detection against the acknowledged baseline,
 * - explicit validation before any request,
 * - one in-flight write per key, with queued saves coalesced onto the latest
 *   edit (a burst of toggle clicks cannot reorder itself),
 * - optimistic edits and a rollback that never overwrites a newer edit.
 *
 * The module is framework-free: React reads it through
 * `SettingsDraftProvider`/`useSetting`, tests drive it directly.
 */

export interface SettingsDraftSaveResult {
  status: "saved" | "unchanged" | "invalid" | "error";
  /** Validation message (usually a translation key) or upstream error text. */
  message?: string;
}

export interface SettingsDraftSnapshot {
  /** Current draft values, keyed by settings section. */
  values: Readonly<Record<string, unknown>>;
  /** Keys whose draft differs from the last acknowledged value. */
  dirtyKeys: readonly string[];
  /** Keys with an in-flight request. */
  savingKeys: readonly string[];
  /** Last validation/upstream error per key, cleared on the next edit. */
  errors: Readonly<Record<string, string>>;
}

/**
 * One section's slice. The object is cached per key, so a card subscribing to
 * `prowlarr` does not re-render when another card edits `limits`.
 */
export interface SettingsSectionSnapshot<T = unknown> {
  value: T | undefined;
  dirty: boolean;
  saving: boolean;
  error: string | undefined;
}

export interface SettingsDraftOptions {
  /** Values loaded from the server, already seeded with their defaults. */
  initial: Record<string, unknown>;
  /**
   * Persist one whole section and return the value the server acknowledged.
   * Returning `undefined` acknowledges the submitted value unchanged.
   */
  write: (key: string, value: unknown) => Promise<unknown>;
  /**
   * Return a message to block the write, or null when the value is valid.
   * The acknowledged baseline lets a rule distinguish a first-time enable from
   * an edit to an integration that was already enabled.
   */
  validate?: (key: string, value: unknown, acknowledged: unknown) => string | null;
  /** Called after every attempt except a no-op save. */
  onResult?: (key: string, result: SettingsDraftSaveResult) => void;
}

export interface SettingsDraft {
  subscribe(listener: () => void): () => void;
  getSnapshot(): SettingsDraftSnapshot;
  /** Cached per-section slice, stable until that section changes. */
  getSectionSnapshot<T = unknown>(key: string): SettingsSectionSnapshot<T>;
  /** Current draft value of one section. Treat it as read-only. */
  get<T = unknown>(key: string): T | undefined;
  /** Merge fields into an object section; keeps fields the card does not edit. */
  patch(key: string, patch: Record<string, unknown>): void;
  /** Replace a scalar or object section. */
  set(key: string, value: unknown): void;
  /** Drop unsaved edits and restore the acknowledged value. */
  reset(key: string): void;
  /** Validate, then persist the current section unless unchanged. */
  save(key: string): Promise<SettingsDraftSaveResult>;
  dispose(): void;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  if (typeof value !== "object" || value === null) return false;
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

function clone<T>(value: T): T {
  if (Array.isArray(value)) return value.map((item) => clone(item)) as T;
  if (isPlainObject(value)) {
    const copy: Record<string, unknown> = {};
    for (const [key, item] of Object.entries(value)) copy[key] = clone(item);
    return copy as T;
  }
  return value;
}

function deepEqual(left: unknown, right: unknown): boolean {
  if (Object.is(left, right)) return true;
  if (Array.isArray(left) || Array.isArray(right)) {
    if (!Array.isArray(left) || !Array.isArray(right) || left.length !== right.length) return false;
    return left.every((item, index) => deepEqual(item, right[index]));
  }
  if (isPlainObject(left) || isPlainObject(right)) {
    if (!isPlainObject(left) || !isPlainObject(right)) return false;
    const leftKeys = Object.keys(left);
    const rightKeys = Object.keys(right);
    if (leftKeys.length !== rightKeys.length) return false;
    return leftKeys.every((key) => key in right && deepEqual(left[key], right[key]));
  }
  return false;
}

function errorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error) return error;
  return "Unknown error";
}

export function createSettingsDraft(options: SettingsDraftOptions): SettingsDraft {
  const { write, validate, onResult } = options;
  let values: Record<string, unknown> = clone(options.initial);
  let acknowledged: Record<string, unknown> = clone(options.initial);
  const errors = new Map<string, string>();
  const saving = new Set<string>();
  const chains = new Map<string, Promise<SettingsDraftSaveResult>>();
  const sectionCache = new Map<string, SettingsSectionSnapshot>();
  const listeners = new Set<() => void>();

  const buildSnapshot = (): SettingsDraftSnapshot => {
    const keys = new Set([...Object.keys(acknowledged), ...Object.keys(values)]);
    const dirtyKeys = [...keys].filter((key) => !deepEqual(values[key], acknowledged[key]));
    return {
      values,
      dirtyKeys,
      savingKeys: [...saving],
      errors: Object.fromEntries(errors),
    };
  };

  let snapshot = buildSnapshot();

  function notify() {
    snapshot = buildSnapshot();
    for (const listener of listeners) listener();
  }

  function clearError(key: string): boolean {
    return errors.delete(key);
  }

  /** Restore the last acknowledged value without touching the per-key error. */
  function restoreAcknowledged(key: string) {
    const restored = clone(acknowledged[key]);
    if (restored === undefined) {
      if (!(key in values)) return;
      const nextValues = { ...values };
      delete nextValues[key];
      values = nextValues;
      return;
    }
    if (deepEqual(values[key], restored)) return;
    values = { ...values, [key]: restored };
  }

  function performSave(key: string): Promise<SettingsDraftSaveResult> {
    const value = values[key];
    if (deepEqual(value, acknowledged[key])) {
      return Promise.resolve({ status: "unchanged" });
    }

    const message = validate?.(key, value, acknowledged[key]) ?? null;
    if (message) {
      // Reject before the request and undo the optimistic edit, but only while
      // it is still the current value: a newer edit stays authoritative.
      if (deepEqual(values[key], value)) restoreAcknowledged(key);
      errors.set(key, message);
      notify();
      const result: SettingsDraftSaveResult = { status: "invalid", message };
      onResult?.(key, result);
      return Promise.resolve(result);
    }

    clearError(key);
    saving.add(key);
    notify();

    const payload = clone(value);
    return Promise.resolve()
      .then(() => write(key, payload))
      .then((acknowledgedValue: unknown): SettingsDraftSaveResult => {
        // The API may answer with a different representation (redacted secrets,
        // server-side defaults). Adopt it as the new baseline.
        const nextAcknowledged =
          acknowledgedValue === undefined ? clone(payload) : clone(acknowledgedValue);
        acknowledged = { ...acknowledged, [key]: nextAcknowledged };

        // A newer edit may have landed while this request was in flight. In
        // that case keep the draft untouched; the queued save owns it.
        if (deepEqual(values[key], payload) && !deepEqual(nextAcknowledged, payload)) {
          values = { ...values, [key]: clone(nextAcknowledged) };
        }

        clearError(key);
        const result: SettingsDraftSaveResult = { status: "saved" };
        onResult?.(key, result);
        return result;
      })
      .catch((error: unknown): SettingsDraftSaveResult => {
        // Roll back only when the failed payload is still the draft value. If
        // the user edited again meanwhile, the newer edit must survive.
        if (deepEqual(values[key], payload)) {
          restoreAcknowledged(key);
        }

        const message = errorMessage(error);
        errors.set(key, message);
        const result: SettingsDraftSaveResult = { status: "error", message };
        onResult?.(key, result);
        return result;
      })
      .finally(() => {
        saving.delete(key);
        notify();
      });
  }

  return {
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },

    getSnapshot() {
      return snapshot;
    },

    getSectionSnapshot<T = unknown>(key: string): SettingsSectionSnapshot<T> {
      const value = values[key] as T | undefined;
      const dirty = !deepEqual(value, acknowledged[key]);
      const isSaving = saving.has(key);
      const error = errors.get(key);
      const previous = sectionCache.get(key);
      if (
        previous &&
        deepEqual(previous.value, value) &&
        previous.dirty === dirty &&
        previous.saving === isSaving &&
        previous.error === error
      ) {
        return previous as SettingsSectionSnapshot<T>;
      }

      const section: SettingsSectionSnapshot<T> = {
        value,
        dirty,
        saving: isSaving,
        error,
      };
      sectionCache.set(key, section);
      return section;
    },

    get<T = unknown>(key: string) {
      return values[key] as T | undefined;
    },

    patch(key, patch) {
      if (Object.keys(patch).length === 0) return;
      const current = values[key];
      const base = isPlainObject(current) ? current : {};
      const next = { ...base, ...clone(patch) };
      if (deepEqual(next, current)) return;
      values = { ...values, [key]: next };
      clearError(key);
      notify();
    },

    set(key, value) {
      if (deepEqual(values[key], value)) return;
      values = { ...values, [key]: clone(value) };
      clearError(key);
      notify();
    },

    reset(key) {
      const restored = clone(acknowledged[key]);
      if (restored === undefined) {
        if (!(key in values)) return;
        const nextValues = { ...values };
        delete nextValues[key];
        values = nextValues;
      } else {
        if (deepEqual(values[key], restored)) return;
        values = { ...values, [key]: restored };
      }
      clearError(key);
      notify();
    },

    save(key) {
      // Serialize writes per key. Each queued call reads the draft when it
      // starts, so a burst collapses to the latest value instead of replaying
      // stale snapshots in order.
      const previous = chains.get(key);
      const next = (previous ? previous.catch(() => undefined) : Promise.resolve(undefined)).then(
        () => performSave(key),
      );
      chains.set(key, next);
      void next
        .finally(() => {
          if (chains.get(key) === next) chains.delete(key);
        })
        .catch(() => undefined);
      return next;
    },

    dispose() {
      listeners.clear();
    },
  };
}
