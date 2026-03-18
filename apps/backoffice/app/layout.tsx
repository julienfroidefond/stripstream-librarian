import type { Metadata } from "next";
import Image from "next/image";
import Link from "next/link";
import type { ReactNode } from "react";
import "./globals.css";
import { ThemeProvider } from "./theme-provider";
import { ThemeToggle } from "./theme-toggle";
import { JobsIndicator } from "./components/JobsIndicator";
import { NavIcon, Icon } from "./components/ui";
import { MobileNav } from "./components/MobileNav";
import { LocaleProvider } from "../lib/i18n/context";
import { getServerLocale, getServerTranslations } from "../lib/i18n/server";
import type { TranslationKey } from "../lib/i18n/fr";

export const metadata: Metadata = {
  title: "StripStream Backoffice",
  description: "Administration backoffice pour StripStream Librarian"
};

type NavItem = {
  href: "/" | "/books" | "/series" | "/libraries" | "/jobs" | "/tokens" | "/settings";
  labelKey: TranslationKey;
  icon: "dashboard" | "books" | "series" | "libraries" | "jobs" | "tokens" | "settings";
};

const navItems: NavItem[] = [
  { href: "/", labelKey: "nav.dashboard", icon: "dashboard" },
  { href: "/books", labelKey: "nav.books", icon: "books" },
  { href: "/series", labelKey: "nav.series", icon: "series" },
  { href: "/libraries", labelKey: "nav.libraries", icon: "libraries" },
  { href: "/jobs", labelKey: "nav.jobs", icon: "jobs" },
  { href: "/tokens", labelKey: "nav.tokens", icon: "tokens" },
];

export default async function RootLayout({ children }: { children: ReactNode }) {
  const locale = await getServerLocale();
  const { t } = await getServerTranslations();

  return (
    <html lang={locale} suppressHydrationWarning>
      <body className="min-h-screen bg-background text-foreground font-sans antialiased bg-grain">
        <ThemeProvider>
          <LocaleProvider initialLocale={locale}>
          {/* Header avec effet glassmorphism */}
          <header className="sticky top-0 z-50 w-full border-b border-border/40 bg-background/70 backdrop-blur-xl backdrop-saturate-150 supports-[backdrop-filter]:bg-background/60">
            <nav className="container mx-auto flex h-16 items-center justify-between px-4">
              {/* Brand */}
              <Link
                href="/"
                className="flex items-center gap-3 hover:opacity-80 transition-opacity duration-200"
              >
                <Image
                  src="/logo.png"
                  alt="StripStream"
                  width={36}
                  height={36}
                  className="rounded-lg"
                />
                <div className="flex items-baseline gap-2">
                  <span className="text-xl font-bold tracking-tight text-foreground">
                    StripStream
                  </span>
                  <span className="text-sm text-muted-foreground font-medium hidden md:inline">
                    {t("common.backoffice")}
                  </span>
                </div>
              </Link>

              {/* Navigation Links */}
              <div className="flex items-center gap-2">
                <div className="hidden md:flex items-center gap-1">
                  {navItems.map((item) => (
                    <NavLink key={item.href} href={item.href} title={t(item.labelKey)}>
                      <NavIcon name={item.icon} />
                      <span className="ml-2 hidden lg:inline">{t(item.labelKey)}</span>
                    </NavLink>
                  ))}
                </div>

                {/* Actions */}
                <div className="flex items-center gap-1 pl-4 ml-2 border-l border-border/60">
                  <JobsIndicator />
                  <Link
                    href="/settings"
                    className="hidden md:flex p-2 rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
                    title={t("nav.settings")}
                  >
                    <Icon name="settings" size="md" />
                  </Link>
                  <ThemeToggle />
                  <MobileNav navItems={navItems.map(item => ({ ...item, label: t(item.labelKey) }))} />
                </div>
              </div>
            </nav>
          </header>

          {/* Main Content */}
          <main className="container mx-auto px-4 sm:px-6 lg:px-8 py-8 pb-16">
            {children}
          </main>
          </LocaleProvider>
        </ThemeProvider>
      </body>
    </html>
  );
}

// Navigation Link Component
function NavLink({ href, title, children }: { href: NavItem["href"]; title?: string; children: React.ReactNode }) {
  return (
    <Link
      href={href}
      title={title}
      className="
        flex items-center
        px-2 lg:px-3 py-2
        rounded-lg
        text-sm font-medium
        text-muted-foreground
        hover:text-foreground
        hover:bg-accent
        transition-colors duration-200
        active:scale-[0.98]
      "
    >
      {children}
    </Link>
  );
}
