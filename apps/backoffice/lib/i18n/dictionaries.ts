import type { Locale } from "./types";
import type { TranslationKey } from "./fr";

const dictionaries: Record<Locale, () => Promise<Record<TranslationKey, string>>> = {
  fr: () => import("./fr").then((m) => m.default),
  en: () => import("./en").then((m) => m.default),
};

export async function getDictionary(locale: Locale): Promise<Record<TranslationKey, string>> {
  return dictionaries[locale]();
}

// Synchronous versions for client-side use
import fr from "./fr";
import en from "./en";

const dictionariesSync: Record<Locale, Record<TranslationKey, string>> = { fr, en };

export function getDictionarySync(locale: Locale): Record<TranslationKey, string> {
  return dictionariesSync[locale];
}

export type TranslateFunction = (key: TranslationKey, params?: Record<string, string | number>) => string;

export function createTranslateFunction(dict: Record<TranslationKey, string>): TranslateFunction {
  return (key: TranslationKey, params?: Record<string, string | number>): string => {
    let value: string = dict[key] ?? key;
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        value = value.replace(new RegExp(`\\{\\{${k}\\}\\}`, "g"), String(v));
      }
    }
    return value;
  };
}
