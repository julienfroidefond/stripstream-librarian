"use client";

import { useRef, useCallback, useEffect, useTransition, useState } from "react";
import { useRouter } from "next/navigation";
import { useTranslation } from "../../lib/i18n/context";
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

export function LiveSearchForm({ fields, basePath, debounceMs = 300, initialValues = {} }: LiveSearchFormProps) {
  const router = useRouter();
  const { t } = useTranslation();
  const [isPending, startTransition] = useTransition();
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const formRef = useRef<HTMLFormElement>(null);
  const [formKey, setFormKey] = useState(0);
  const isOwnNavRef = useRef(false);
  const isFirstRender = useRef(true);

  const cookieName = filterCookieName(basePath);

  // Serialize initialValues for effect dependency
  const initialValuesKey = JSON.stringify(initialValues);

  const buildUrl = useCallback((): string => {
    if (!formRef.current) return basePath;
    const formData = new FormData(formRef.current);
    const params = new URLSearchParams();
    for (const [key, value] of formData.entries()) {
      const str = value.toString().trim();
      if (str) params.set(key, str);
    }
    const qs = params.toString();
    return qs ? `${basePath}?${qs}` : basePath;
  }, [basePath]);

  const saveFilters = useCallback(() => {
    if (!formRef.current) return;
    const formData = new FormData(formRef.current);
    const filters: Record<string, string> = {};
    for (const [key, value] of formData.entries()) {
      const str = value.toString().trim();
      if (str) filters[key] = str;
    }
    try {
      if (Object.keys(filters).length > 0) {
        setCookie(cookieName, JSON.stringify(filters));
      } else {
        deleteCookie(cookieName);
      }
    } catch {}
  }, [cookieName]);

  const doNavigate = useCallback((url: string) => {
    isOwnNavRef.current = true;
    startTransition(() => { router.replace(url as any); });
  }, [router]);

  const navigate = useCallback((immediate: boolean) => {
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
  }, [buildUrl, debounceMs, saveFilters, doNavigate]);

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

  // When initialValues change (from server re-render after navigation),
  // remount form only if it was an external navigation (back/forward)
  useEffect(() => {
    if (isFirstRender.current) {
      isFirstRender.current = false;
      return;
    }
    if (isOwnNavRef.current) {
      isOwnNavRef.current = false;
      return;
    }
    // External navigation (back/forward) — remount to sync form values
    setFormKey(k => k + 1);
  }, [initialValuesKey]);

  const hasFilters = fields.some((f) => {
    const val = initialValues[f.name];
    return val && val.trim() !== "";
  });

  const textFields = fields.filter((f) => f.type === "text");
  const selectFields = fields.filter((f) => f.type === "select");

  return (
    <form
      key={formKey}
      ref={formRef}
      action={basePath}
      method="GET"
      onSubmit={(e) => {
        e.preventDefault();
        if (timerRef.current) clearTimeout(timerRef.current);
        saveFilters();
        doNavigate(buildUrl());
      }}
      className="space-y-4"
    >
      {/* Search input with icon */}
      {textFields.map((field) => (
        <div key={field.name} className="relative">
          <Icon name="search" size="md" className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none" />
          {isPending && (
            <Icon name="spinner" size="sm" className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground animate-spin pointer-events-none" />
          )}
          <input
            name={field.name}
            type="text"
            placeholder={field.placeholder}
            defaultValue={initialValues[field.name] || ""}
            onChange={() => navigate(false)}
            className="flex h-11 w-full rounded-lg border border-input bg-background pl-10 pr-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
          />
        </div>
      ))}

      {/* Filters row */}
      {selectFields.length > 0 && (
        <>
          {textFields.length > 0 && (
            <div className="border-t border-border/60" />
          )}
          <div className="flex flex-wrap gap-3 items-center">
            {selectFields.map((field) => (
              <div key={field.name} className="flex items-center gap-1.5">
                {FILTER_ICONS[field.name] && (
                  <svg className="w-3.5 h-3.5 text-muted-foreground shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d={FILTER_ICONS[field.name]} />
                  </svg>
                )}
                <label className="text-xs font-medium text-muted-foreground whitespace-nowrap">
                  {field.label}
                </label>
                <select
                  name={field.name}
                  defaultValue={initialValues[field.name] || ""}
                  onChange={() => navigate(true)}
                  className="h-8 rounded-md border border-input bg-background px-2 py-1 text-xs ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                >
                  {field.options?.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>
            ))}
            {hasFilters && (
              <button
                type="button"
                onClick={() => {
                  formRef.current?.reset();
                  try { deleteCookie(cookieName); } catch {}
                  doNavigate(basePath);
                  setFormKey(k => k + 1);
                }}
                className="
                  inline-flex items-center gap-1
                  h-8 px-2.5
                  text-xs font-medium
                  text-muted-foreground
                  rounded-md
                  hover:bg-accent hover:text-accent-foreground
                  transition-colors duration-200
                "
              >
                <Icon name="x" size="sm" className="!w-3.5 !h-3.5" />
                {t("common.clear")}
              </button>
            )}
          </div>
        </>
      )}
    </form>
  );
}
