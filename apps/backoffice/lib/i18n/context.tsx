"use client";

import { createContext, useContext, useCallback, useState, type ReactNode } from "react";
import type { Locale } from "./types";
import { LOCALE_COOKIE } from "./types";
import { getDictionarySync, createTranslateFunction } from "./dictionaries";
import type { TranslateFunction } from "./dictionaries";

interface LocaleContextValue {
  locale: Locale;
  t: TranslateFunction;
  setLocale: (locale: Locale) => void;
}

const LocaleContext = createContext<LocaleContextValue | null>(null);

interface LocaleProviderProps {
  initialLocale: Locale;
  children: ReactNode;
}

export function LocaleProvider({ initialLocale, children }: LocaleProviderProps) {
  const [locale] = useState<Locale>(initialLocale);
  const dict = getDictionarySync(locale);
  const t = createTranslateFunction(dict);

  const setLocale = useCallback(async (newLocale: Locale) => {
    // Set cookie
    document.cookie = `${LOCALE_COOKIE}=${newLocale};path=/;max-age=${365 * 24 * 60 * 60}`;

    // Save to DB
    try {
      const apiBase = process.env.NEXT_PUBLIC_API_URL || "http://localhost:7080";
      await fetch(`${apiBase}/settings`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ language: newLocale }),
      });
    } catch {
      // Best effort — cookie is the primary source for rendering
    }

    // Reload to apply new locale everywhere (server + client)
    window.location.reload();
  }, []);

  return (
    <LocaleContext.Provider value={{ locale, t, setLocale }}>
      {children}
    </LocaleContext.Provider>
  );
}

export function useTranslation(): LocaleContextValue {
  const ctx = useContext(LocaleContext);
  if (!ctx) {
    throw new Error("useTranslation must be used within a LocaleProvider");
  }
  return ctx;
}
