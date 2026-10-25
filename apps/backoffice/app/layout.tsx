import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import "./globals.css";
import { ThemeProvider } from "./theme-provider";
import { LocaleProvider } from "@/lib/i18n/context";
import { getServerLocale } from "@/lib/i18n/server";
import { APPLE_WEB_APP_META, SPLASH_LINKS } from "@/lib/pwa";

export const metadata: Metadata = {
  title: "Stripstream Librarian",
  description: "Gestionnaire de bibliothèque de bandes dessinées et ebooks",
  manifest: "/manifest.json",
  appleWebApp: {
    capable: true,
    statusBarStyle: "black-translucent",
    title: "Stripstream Librarian",
  },
  other: APPLE_WEB_APP_META,
  icons: {
    icon: [
      { url: "/icons/favicon.png", sizes: "32x32", type: "image/png" },
      { url: "/icons/icon-192.png", sizes: "192x192", type: "image/png" },
      { url: "/icons/icon-512.png", sizes: "512x512", type: "image/png" },
    ],
    apple: "/icons/apple-touch-icon.png",
  },
};

export const viewport: Viewport = {
  // `--color-background` du thème sombre, aligné sur `theme_color` du manifeste
  // PWA : la barre du navigateur, l'écran de démarrage et la page sont la même
  // couleur.
  themeColor: "#100f0e",
  width: "device-width",
  initialScale: 1,
  viewportFit: "cover",
};

export default async function RootLayout({ children }: { children: ReactNode }) {
  const locale = await getServerLocale();

  return (
    <html lang={locale} suppressHydrationWarning>
      <head>
        {/* Flag set before first paint so the scroll-reveal system can hide
            `[data-reveal]` surfaces without flashing them first. Without JS
            the attribute is simply never set and everything stays visible. */}
        <script
          dangerouslySetInnerHTML={{
            __html: "document.documentElement.classList.add('js-motion');",
          }}
        />
        {/* Explicit apple-touch-icons (iOS ignores manifest.json icons) */}
        <link rel="apple-touch-icon" sizes="180x180" href="/icons/apple-touch-icon.png" />
        <link rel="apple-touch-icon" sizes="167x167" href="/icons/apple-touch-icon-167.png" />
        <link rel="apple-touch-icon" sizes="152x152" href="/icons/apple-touch-icon-152.png" />
        <link rel="apple-touch-icon" sizes="120x120" href="/icons/apple-touch-icon-120.png" />
        <link rel="apple-touch-icon" href="/apple-touch-icon.png" />
        {SPLASH_LINKS.map((s) => (
          <link key={s.src} rel="apple-touch-startup-image" href={s.src} media={s.media} />
        ))}
      </head>
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
