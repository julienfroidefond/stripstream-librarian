import Image from "next/image";
import Link from "next/link";
import type { ReactNode } from "react";
import { cookies } from "next/headers";
import { revalidatePath } from "next/cache";
import { ThemeToggle } from "@/app/theme-toggle";
import { JobsIndicator } from "@/app/components/JobsIndicator";
import { NavIcon, Icon } from "@/app/components/ui";
import { LogoutButton } from "@/app/components/LogoutButton";
import { MobileNav } from "@/app/components/MobileNav";
import { UserSwitcher } from "@/app/components/UserSwitcher";
import { fetchUsers } from "@/lib/api";
import { getServerTranslations } from "@/lib/i18n/server";
import type { TranslationKey } from "@/lib/i18n/fr";

type NavItem = {
  href: "/" | "/books" | "/series" | "/authors" | "/libraries" | "/jobs" | "/tokens" | "/settings";
  labelKey: TranslationKey;
  icon: "dashboard" | "books" | "series" | "authors" | "libraries" | "jobs" | "tokens" | "settings";
};

const navItems: NavItem[] = [
  { href: "/", labelKey: "nav.dashboard", icon: "dashboard" },
  { href: "/books", labelKey: "nav.books", icon: "books" },
  { href: "/series", labelKey: "nav.series", icon: "series" },
  { href: "/authors", labelKey: "nav.authors", icon: "authors" },
  { href: "/libraries", labelKey: "nav.libraries", icon: "libraries" },
  { href: "/jobs", labelKey: "nav.jobs", icon: "jobs" },
  { href: "/tokens", labelKey: "nav.tokens", icon: "tokens" },
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
      <header className="sticky top-0 z-50 w-full border-b border-border/40 bg-background/70 backdrop-blur-xl backdrop-saturate-150 supports-[backdrop-filter]:bg-background/60">
        <nav className="container mx-auto flex h-16 items-center justify-between px-4">
          <Link
            href="/"
            className="flex items-center gap-3 hover:opacity-80 transition-opacity duration-200"
          >
            <Image src="/logo.png" alt="StripStream" width={36} height={36} className="rounded-lg" />
            <div className="flex items-baseline gap-2">
              <span className="text-xl font-bold tracking-tight text-foreground">StripStream</span>
              <span className="text-sm text-muted-foreground font-medium hidden xl:inline">
                {t("common.backoffice")}
              </span>
            </div>
          </Link>

          <div className="flex items-center gap-2">
            <div className="hidden md:flex items-center gap-1">
              {navItems.map((item) => (
                <NavLink key={item.href} href={item.href} title={t(item.labelKey)}>
                  <NavIcon name={item.icon} />
                  <span className="ml-2 hidden xl:inline">{t(item.labelKey)}</span>
                </NavLink>
              ))}
            </div>

            <UserSwitcher
              users={users}
              activeUserId={activeUserId}
              setActiveUserAction={setActiveUserAction}
            />

            <div className="flex items-center gap-1 pl-4 ml-2 border-l border-border/60">
              <JobsIndicator />
              <Link
                href="/settings"
                className="hidden xl:flex p-2 rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
                title={t("nav.settings")}
              >
                <Icon name="settings" size="md" />
              </Link>
              <ThemeToggle />
              <LogoutButton />
              <MobileNav navItems={navItems.map(item => ({ ...item, label: t(item.labelKey) }))} />
            </div>
          </div>
        </nav>
      </header>

      <main className="container mx-auto px-4 sm:px-6 lg:px-8 py-8 pb-16">
        {children}
      </main>
    </>
  );
}

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
