import { cookies } from "next/headers";
import type { Locale } from "./types";
import { DEFAULT_LOCALE, LOCALE_COOKIE, LOCALES } from "./types";
import { getDictionarySync, createTranslateFunction } from "./dictionaries";
import type { TranslateFunction } from "./dictionaries";

export async function getServerLocale(): Promise<Locale> {
  const cookieStore = await cookies();
  const raw = cookieStore.get(LOCALE_COOKIE)?.value;
  if (raw && LOCALES.includes(raw as Locale)) {
    return raw as Locale;
  }
  return DEFAULT_LOCALE;
}

export async function getServerTranslations(): Promise<{ t: TranslateFunction; locale: Locale }> {
  const locale = await getServerLocale();
  const dict = getDictionarySync(locale);
  return { t: createTranslateFunction(dict), locale };
}
