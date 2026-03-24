import type { Metadata } from "next";
import type { ReactNode } from "react";
import "./globals.css";
import { ThemeProvider } from "./theme-provider";
import { LocaleProvider } from "@/lib/i18n/context";
import { getServerLocale } from "@/lib/i18n/server";

export const metadata: Metadata = {
  title: "StripStream Backoffice",
  description: "Administration backoffice pour StripStream Librarian"
};

export default async function RootLayout({ children }: { children: ReactNode }) {
  const locale = await getServerLocale();

  return (
    <html lang={locale} suppressHydrationWarning>
      <body className="min-h-screen bg-background text-foreground font-sans antialiased bg-grain">
        <ThemeProvider>
          <LocaleProvider initialLocale={locale}>
            {children}
          </LocaleProvider>
        </ThemeProvider>
      </body>
    </html>
  );
}
