import Image from "next/image";
import Link from "next/link";
import type { ReactNode } from "react";
import { cookies } from "next/headers";
import { revalidatePath } from "next/cache";
import { ThemeToggle } from "@/app/theme-toggle";
import { JobsIndicator } from "@/app/components/JobsIndicator";
import { DownloadsIndicator } from "@/app/components/DownloadsIndicator";
import { NavIcon, Icon } from "@/app/components/ui";
import { NavLink } from "@/app/components/NavLink";
import { CollapsibleNav, CollapsibleNavProvider, NavToggleButton } from "@/app/components/CollapsibleNav";
import { LogoutButton } from "@/app/components/LogoutButton";
import { MobileNav } from "@/app/components/MobileNav";
import { UserSwitcher } from "@/app/components/UserSwitcher";
import { fetchUsers } from "@/lib/api";
import { getServerTranslations } from "@/lib/i18n/server";
import type { TranslationKey } from "@/lib/i18n/fr";

type NavItem = {
  href: "/" | "/books" | "/series" | "/authors" | "/libraries" | "/discovery" | "/jobs" | "/tokens" | "/settings" | "/downloads";
  labelKey: TranslationKey;
  icon: "dashboard" | "books" | "series" | "authors" | "libraries" | "search" | "jobs" | "tokens" | "settings" | "download";
  color?: string;
};

const navItems: NavItem[] = [
  { href: "/books", labelKey: "nav.books", icon: "books", color: "text-success" },
  { href: "/series", labelKey: "nav.series", icon: "series", color: "text-warning" },
  { href: "/authors", labelKey: "nav.authors", icon: "authors", color: "text-violet-500" },
  { href: "/libraries", labelKey: "nav.libraries", icon: "libraries", color: "text-primary" },
  { href: "/discovery", labelKey: "nav.discovery", icon: "search", color: "text-cyan-500" },
  { href: "/downloads", labelKey: "nav.downloads", icon: "download", color: "text-emerald-500" },
  { href: "/jobs", labelKey: "nav.jobs", icon: "jobs", color: "text-amber-500" },
  { href: "/tokens", labelKey: "nav.tokens", icon: "tokens", color: "text-rose-500" },
  { href: "/settings", labelKey: "nav.settings", icon: "settings", color: "text-slate-400" },
];

export default async function AppLayout({ children }: { children: ReactNode }) {
  const { t } = await getServerTranslations();
  const cookieStore = await cookies();
  const activeUserId = cookieStore.get("as_user_id")?.value || null;
  const users = await fetchUsers().catch(() => []);

  async function setActiveUserAction(formData: FormData) {
    "use server";
    const userId = formData.get("user_id") as string;
    const store = await cookies();
    if (userId) {
      store.set("as_user_id", userId, { path: "/", httpOnly: false, sameSite: "lax" });
    } else {
      store.delete("as_user_id");
    }
    revalidatePath("/", "layout");
  }

  return (
    <>
      <CollapsibleNavProvider>
      <header
        className="sticky top-0 z-50 w-full border-b border-border/40 bg-background/70 backdrop-blur-xl backdrop-saturate-150 supports-[backdrop-filter]:bg-background/60"
        style={{ paddingTop: "env(safe-area-inset-top)" }}
      >
        {/* Row 1: Logo + actions */}
        <div className="container mx-auto flex h-12 items-center justify-between px-4">
          <Link
            href="/"
            className="flex items-center gap-3 hover:opacity-80 transition-opacity duration-200"
          >
            <Image src="/logo.webp" alt="StripStream" width={32} height={32} className="rounded-lg" />
            <div className="flex items-baseline gap-2">
              <span className="text-lg font-bold tracking-tight text-foreground">StripStream</span>
              <span className="text-sm text-muted-foreground font-medium hidden xl:inline">
                {t("common.backoffice")}
              </span>
            </div>
          </Link>

          <div className="flex items-center gap-1.5">
            <div className="hidden md:block">
              <UserSwitcher
                users={users}
                activeUserId={activeUserId}
                setActiveUserAction={setActiveUserAction}
              />
            </div>
            <DownloadsIndicator />
            <JobsIndicator />
            <ThemeToggle />
            <div className="hidden md:block">
              <LogoutButton />
            </div>
            <MobileNav
              navItems={[
                { href: "/", label: t("nav.dashboard"), icon: "dashboard" },
                ...navItems.map(item => ({ ...item, label: t(item.labelKey) })),
              ]}
              users={users}
              activeUserId={activeUserId}
              setActiveUserAction={setActiveUserAction}
            />
            <NavToggleButton />
          </div>
        </div>

        {/* Row 2: Collapsible navigation */}
        <CollapsibleNav>
          {navItems.map((item) => (
            <NavLink key={item.href} href={item.href} title={t(item.labelKey)}>
              <NavIcon name={item.icon} className={item.color} />
              <span className="ml-2 hidden xl:inline">{t(item.labelKey)}</span>
            </NavLink>
          ))}
        </CollapsibleNav>
      </header>
      </CollapsibleNavProvider>

      <main
        className="container mx-auto px-4 sm:px-6 lg:px-8 py-8 pb-16"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
          paddingBottom: "max(4rem, env(safe-area-inset-bottom))",
        }}
      >
        {children}
      </main>
    </>
  );
}

