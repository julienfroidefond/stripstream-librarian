"use client";

import { useRef, useCallback, useEffect } from "react";
import { useRouter, useSearchParams } from "next/navigation";

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
}

export function LiveSearchForm({ fields, basePath, debounceMs = 300 }: LiveSearchFormProps) {
  const router = useRouter();
  const searchParams = useSearchParams();
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const formRef = useRef<HTMLFormElement>(null);

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

  const navigate = useCallback((immediate: boolean) => {
    if (timerRef.current) clearTimeout(timerRef.current);
    if (immediate) {
      router.replace(buildUrl() as any);
    } else {
      timerRef.current = setTimeout(() => {
        router.replace(buildUrl() as any);
      }, debounceMs);
    }
  }, [router, buildUrl, debounceMs]);

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  const hasFilters = fields.some((f) => {
    const val = searchParams.get(f.name);
    return val && val.trim() !== "";
  });

  return (
    <form
      ref={formRef}
      onSubmit={(e) => {
        e.preventDefault();
        if (timerRef.current) clearTimeout(timerRef.current);
        router.replace(buildUrl() as any);
      }}
      className="flex flex-col sm:flex-row gap-3 items-start sm:items-end"
    >
      {fields.map((field) =>
        field.type === "text" ? (
          <div key={field.name} className={field.className || "flex-1 w-full"}>
            <label className="block text-sm font-medium text-foreground mb-1.5">
              {field.label}
            </label>
            <input
              name={field.name}
              type="text"
              placeholder={field.placeholder}
              defaultValue={searchParams.get(field.name) || ""}
              onChange={() => navigate(false)}
              className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
            />
          </div>
        ) : (
          <div key={field.name} className={field.className || "w-full sm:w-48"}>
            <label className="block text-sm font-medium text-foreground mb-1.5">
              {field.label}
            </label>
            <select
              name={field.name}
              defaultValue={searchParams.get(field.name) || ""}
              onChange={() => navigate(true)}
              className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
            >
              {field.options?.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>
        )
      )}
      {hasFilters && (
        <button
          type="button"
          onClick={() => router.replace(basePath as any)}
          className="
            inline-flex items-center justify-center
            h-10 px-4
            border border-input
            text-sm font-medium
            text-muted-foreground
            bg-background
            rounded-md
            hover:bg-accent hover:text-accent-foreground
            transition-colors duration-200
            w-full sm:w-auto
          "
        >
          Effacer
        </button>
      )}
    </form>
  );
}
