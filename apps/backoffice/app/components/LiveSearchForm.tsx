"use client";

import { useCallback, useEffect, useId, useRef, useState, useTransition } from "react";
import { useRouter } from "next/navigation";
import { useTranslation } from "@/lib/i18n/context";
import { Icon } from "./ui";

// SVG path data for filter icons, keyed by field name
const FILTER_ICONS: Record<string, string> = {
  // Library - building/collection
  library: "M8 14v3m4-3v3m4-3v3M3 21h18M3 10h18M3 7l9-4 9 4M4 10h16v11H4V10z",
  // Reading status - open book
  status: "M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253",
  // Series status - signal/activity
  series_status: "M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z",
  // Missing books - warning triangle
  has_missing: "M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z",
  // Metadata provider - tag
  metadata_provider: "M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z",
  // Sort - arrows up/down
  sort: "M3 4h13M3 8h9m-9 4h6m4 0l4-4m0 0l4 4m-4-4v12",
  // Format - document/file
  format: "M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z",
  // Metadata - link/chain
  metadata: "M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1",
  // Books filter - bookmark
  books_filter: "M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z",
  // Volume type - layers
  volume_type: "M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2M7 7V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10",
  // Ratings - star
  rated_only: "M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.196-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118L2.098 10.1c-.783-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z",
};

interface FieldDef {
  name: string;
  type: "text" | "select";
  placeholder?: string;
  label: string;
  options?: { value: string; label: string }[];
  className?: string;
}

interface LiveSearchFormProps {
  fields: FieldDef[];
  basePath: string;
  debounceMs?: number;
  initialValues?: Record<string, string>;
  hiddenValues?: Record<string, string>;
  /** Page-level spacing (margins) applied to the filter bar. */
  className?: string;
}

/** Convert a basePath to a cookie name: /series → filters_series */
function filterCookieName(basePath: string): string {
  return `filters_${basePath.replace(/^\//, "").replace(/\//g, "_")}`;
}

function setCookie(name: string, value: string, days = 365) {
  const expires = new Date(Date.now() + days * 864e5).toUTCString();
  document.cookie = `${name}=${encodeURIComponent(value)};path=/;expires=${expires};SameSite=Lax`;
}

function deleteCookie(name: string) {
  document.cookie = `${name}=;path=/;expires=Thu, 01 Jan 1970 00:00:00 GMT`;
}

/** One filter rendered as a compact pill; the native select stays on top (invisible) for behaviour + a11y. */
function FilterPill({
  field,
  value,
  onChange,
}: {
  field: FieldDef;
  value: string;
  onChange: (name: string, value: string) => void;
}) {
  const active = value !== "";
  const selected = field.options?.find((option) => option.value === value);
  const iconPath = FILTER_ICONS[field.name];

  return (
    <label
      title={`${field.label}: ${selected?.label ?? ""}`}
      className={`relative inline-flex h-9 max-w-[18rem] items-center gap-2 rounded-full border pl-3 pr-8 text-xs transition-all duration-200 ease-out focus-within:ring-2 focus-within:ring-ring/50 focus-within:ring-offset-2 focus-within:ring-offset-card ${
        active
          ? "border-primary/40 bg-primary/10 text-foreground"
          : "border-border/70 bg-background/60 text-muted-foreground hover:border-border hover:bg-accent/60 hover:text-foreground"
      }`}
    >
      {iconPath && (
        <svg
          aria-hidden="true"
          className={`pointer-events-none h-3.5 w-3.5 shrink-0 ${active ? "text-primary" : "text-muted-foreground/60"}`}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d={iconPath} />
        </svg>
      )}
      <span
        className={`pointer-events-none shrink-0 whitespace-nowrap text-2xs font-semibold uppercase tracking-wide ${
          active ? "text-primary" : "text-muted-foreground/80"
        }`}
      >
        {field.label}
      </span>
      <span
        className={`pointer-events-none h-3.5 w-px shrink-0 ${active ? "bg-primary/40" : "bg-border"}`}
      />
      <span className="pointer-events-none truncate font-medium">{selected?.label ?? ""}</span>
      <Icon
        name="chevronDown"
        size="sm"
        mono
        className="pointer-events-none absolute right-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground/70"
      />
      <select
        name={field.name}
        aria-label={field.label}
        value={value}
        onChange={(event) => onChange(field.name, event.target.value)}
        className="absolute inset-0 h-full w-full cursor-pointer appearance-none rounded-full opacity-0"
      >
        {field.options?.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

export function LiveSearchForm({
  fields,
  basePath,
  debounceMs = 300,
  initialValues = {},
  hiddenValues = {},
  className = "",
}: LiveSearchFormProps) {
  const router = useRouter();
  const { t } = useTranslation();
  const [isPending, startTransition] = useTransition();
  const filtersToggleId = useId();
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isOwnNavRef = useRef(false);
  const isFirstRender = useRef(true);

  const [values, setValues] = useState<Record<string, string>>(() => ({ ...initialValues }));
  // Mirrors `values` synchronously so debounced navigations never read stale state.
  const valuesRef = useRef(values);

  const cookieName = filterCookieName(basePath);
  const hiddenQuery = new URLSearchParams(
    Object.entries(hiddenValues).filter(([, value]) => value !== ""),
  ).toString();

  // Serialize initialValues for effect dependency
  const initialValuesKey = JSON.stringify(initialValues);

  const setValue = useCallback((name: string, value: string) => {
    valuesRef.current = { ...valuesRef.current, [name]: value };
    setValues(valuesRef.current);
  }, []);

  const buildUrl = useCallback((): string => {
    const params = new URLSearchParams(hiddenQuery);
    for (const [key, value] of Object.entries(valuesRef.current)) {
      const str = value.trim();
      if (str) params.set(key, str);
      else params.delete(key);
    }
    const qs = params.toString();
    return qs ? `${basePath}?${qs}` : basePath;
  }, [basePath, hiddenQuery]);

  const saveFilters = useCallback(() => {
    const filters: Record<string, string> = {};
    for (const field of fields) {
      const str = (valuesRef.current[field.name] ?? "").trim();
      if (str) filters[field.name] = str;
    }
    try {
      if (Object.keys(filters).length > 0) {
        setCookie(cookieName, JSON.stringify(filters));
      } else {
        deleteCookie(cookieName);
      }
    } catch {}
  }, [cookieName, fields]);

  const doNavigate = useCallback(
    (url: string) => {
      isOwnNavRef.current = true;
      window.history.replaceState(null, "", url);
      startTransition(() => {
        router.refresh();
      });
    },
    [router],
  );

  const navigate = useCallback(
    (immediate: boolean) => {
      if (timerRef.current) clearTimeout(timerRef.current);
      if (immediate) {
        saveFilters();
        doNavigate(buildUrl());
      } else {
        timerRef.current = setTimeout(() => {
          saveFilters();
          doNavigate(buildUrl());
        }, debounceMs);
      }
    },
    [buildUrl, debounceMs, saveFilters, doNavigate],
  );

  // Cleanup timer on unmount
  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  // Detect back/forward navigation via popstate
  useEffect(() => {
    const handlePopState = () => {
      isOwnNavRef.current = false;
    };
    window.addEventListener("popstate", handlePopState);
    return () => window.removeEventListener("popstate", handlePopState);
  }, []);

  // When initialValues change (from server re-render after navigation), sync the
  // controlled fields unless the change comes from this form's own navigation.
  useEffect(() => {
    if (isFirstRender.current) {
      isFirstRender.current = false;
      return;
    }
    if (isOwnNavRef.current) {
      isOwnNavRef.current = false;
      return;
    }
    try {
      const next = JSON.parse(initialValuesKey) as Record<string, string>;
      valuesRef.current = next;
      // Sync local controlled state from server props after an external navigation
      // (back/forward, link click). This is the documented "adjust state on prop change"
      // case; the effect is intentional and guarded by isOwnNavRef.
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setValues(next);
    } catch {}
  }, [initialValuesKey]);

  const clearField = useCallback(
    (name: string) => {
      setValue(name, "");
      navigate(true);
    },
    [navigate, setValue],
  );

  const clearAll = useCallback(() => {
    const cleared: Record<string, string> = {};
    for (const field of fields) cleared[field.name] = "";
    valuesRef.current = cleared;
    setValues(cleared);
    try {
      deleteCookie(cookieName);
    } catch {}
    navigate(true);
  }, [cookieName, fields, navigate]);

  const hasFilters = fields.some((field) => (values[field.name] ?? "").trim() !== "");
  const textFields = fields.filter((field) => field.type === "text");
  const selectFields = fields.filter((field) => field.type === "select");
  const activeFilterCount = selectFields.filter((field) => (values[field.name] ?? "").trim() !== "").length;

  return (
    <form
      action={basePath}
      method="GET"
      onSubmit={(event) => {
        event.preventDefault();
        navigate(true);
      }}
      className={`space-y-3 rounded-xl border border-border/60 bg-card p-3 shadow-soft ${className}`}
    >
      {hiddenQuery &&
        Object.entries(hiddenValues)
          .filter(([, value]) => value !== "")
          .map(([name, value]) => <input key={name} type="hidden" name={name} value={value} />)}

      {textFields.map((field) => {
        const value = values[field.name] ?? "";
        return (
          <div key={field.name} className="relative">
            <Icon
              name="search"
              size="md"
              mono
              className="pointer-events-none absolute left-3.5 top-1/2 -translate-y-1/2 text-muted-foreground/70"
            />
            <input
              name={field.name}
              type="text"
              value={value}
              placeholder={field.placeholder}
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => {
                setValue(field.name, event.target.value);
                navigate(false);
              }}
              onKeyDown={(event) => {
                if (event.key === "Escape" && value) {
                  event.preventDefault();
                  clearField(field.name);
                }
              }}
              className="h-11 w-full rounded-xl border border-border/70 bg-background/60 pl-11 pr-20 text-sm text-foreground shadow-soft outline-none transition-colors placeholder:text-muted-foreground/80 hover:border-border focus:border-primary/50 focus:bg-background focus:ring-4 focus:ring-primary/10"
            />
            <div className="absolute right-2 top-1/2 flex -translate-y-1/2 items-center gap-1">
              {isPending && (
                <Icon
                  name="spinner"
                  size="sm"
                  mono
                  className="animate-spin text-muted-foreground/70"
                />
              )}
              {value && (
                <button
                  type="button"
                  onClick={() => clearField(field.name)}
                  title={t("common.clear")}
                  aria-label={t("common.clear")}
                  className="inline-flex h-7 w-7 items-center justify-center rounded-full text-muted-foreground/70 transition-colors hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                >
                  <Icon name="x" size="sm" mono />
                </button>
              )}
            </div>
          </div>
        );
      })}

      {selectFields.length > 0 && (
        <>
          <input id={filtersToggleId} type="checkbox" aria-label={t("common.filters")} className="peer sr-only" />

          {/* Mobile: filters are collapsed behind a toggle to keep the bar compact. */}
          <label
            htmlFor={filtersToggleId}
            className={`flex h-9 w-fit cursor-pointer items-center gap-1.5 rounded-full border px-3 text-xs font-medium transition-colors peer-focus-visible:ring-2 peer-focus-visible:ring-ring peer-focus-visible:ring-offset-2 peer-checked:[&>svg:last-child]:rotate-180 sm:hidden ${
              activeFilterCount > 0
                ? "border-primary/40 bg-primary/10 text-primary"
                : "border-border/70 bg-background/60 text-muted-foreground"
            }`}
          >
            <svg aria-hidden="true" className="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M7 12h10m-7 6h4" />
            </svg>
            {t("common.filters")}
            {activeFilterCount > 0 && (
              <span className="inline-flex h-4 min-w-4 items-center justify-center rounded-full bg-primary px-1 text-3xs font-semibold text-primary-foreground">
                {activeFilterCount}
              </span>
            )}
            <Icon
              name="chevronDown"
              size="sm"
              mono
              className="h-3.5 w-3.5 transition-transform"
            />
          </label>

          <div className="hidden flex-wrap items-center gap-2 peer-checked:flex sm:flex">
            {selectFields.map((field) => (
              <FilterPill
                key={field.name}
                field={field}
                value={values[field.name] ?? ""}
                onChange={(name, value) => {
                  setValue(name, value);
                  navigate(true);
                }}
              />
            ))}
            {hasFilters && (
              <button
                type="button"
                onClick={clearAll}
                className="ml-auto inline-flex h-9 items-center gap-1 rounded-full px-2.5 text-xs font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <Icon name="x" size="sm" mono className="!h-3.5 !w-3.5" />
                {t("common.clear")}
              </button>
            )}
          </div>
        </>
      )}
    </form>
  );
}
