"use client";

import { useState, useEffect, useTransition } from "react";
import { createPortal } from "react-dom";
import Link from "next/link";
import { NavIcon } from "./ui";
import { useTranslation } from "../../lib/i18n/context";
import type { UserDto } from "@/lib/api";

type NavItem = {
  href: "/" | "/books" | "/series" | "/authors" | "/libraries" | "/downloads" | "/jobs" | "/tokens" | "/settings";
  label: string;
  icon: "dashboard" | "books" | "series" | "authors" | "libraries" | "download" | "jobs" | "tokens" | "settings";
};

const HamburgerIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} className="w-5 h-5">
    <path d="M3 6h18M3 12h18M3 18h18" strokeLinecap="round" />
  </svg>
);

const XIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} className="w-5 h-5">
    <path d="M18 6L6 18M6 6l12 12" strokeLinecap="round" />
  </svg>
);

export function MobileNav({ navItems, users, activeUserId, setActiveUserAction }: {
  navItems: NavItem[];
  users: UserDto[];
  activeUserId: string | null;
  setActiveUserAction: (formData: FormData) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [mounted, setMounted] = useState(false);
  const [, startTransition] = useTransition();

  function select(userId: string | null) {
    startTransition(async () => {
      const fd = new FormData();
      fd.append("user_id", userId ?? "");
      await setActiveUserAction(fd);
    });
  }

  useEffect(() => {
    setMounted(true);
  }, []);

  const overlay = (
    <>
      {/* Backdrop */}
      <div
        className={`fixed inset-0 z-[60] bg-background/80 backdrop-blur-sm md:hidden transition-opacity duration-300 ${isOpen ? "opacity-100" : "opacity-0 pointer-events-none"}`}
        onClick={() => setIsOpen(false)}
        aria-hidden="true"
      />

      {/* Drawer */}
      <div
        className={`
          fixed inset-y-0 left-0 z-[70] w-64
          bg-background/95 backdrop-blur-xl
          border-r border-border/60
          flex flex-col
          transform transition-transform duration-300 ease-in-out
          md:hidden
          ${isOpen ? "translate-x-0" : "-translate-x-full"}
        `}
      >
        <div className="h-16 border-b border-border/40 flex items-center px-4">
          <span className="text-sm font-semibold text-muted-foreground tracking-wide uppercase">{t("nav.navigation")}</span>
        </div>

        <nav className="flex flex-col gap-1 p-3 flex-1">
          {navItems.map((item) => (
            <Link
              key={item.href}
              href={item.href}
              className="flex items-center gap-3 px-3 py-3 rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-200 active:scale-[0.98]"
              onClick={() => setIsOpen(false)}
            >
              <NavIcon name={item.icon} />
              <span className="font-medium">{item.label}</span>
            </Link>
          ))}

          <div className="border-t border-border/40 mt-2 pt-2 space-y-1">
            {users.length > 0 && (
              <>
                <p className="px-3 pt-1 text-xs font-semibold text-muted-foreground uppercase tracking-wide">{t("users.title")}</p>
                <button
                  onClick={() => { select(null); setIsOpen(false); }}
                  className={`w-full flex items-center gap-3 px-3 py-3 rounded-lg transition-colors duration-200 active:scale-[0.98] ${
                    !activeUserId ? "text-foreground bg-accent font-medium" : "text-muted-foreground hover:text-foreground hover:bg-accent"
                  }`}
                >
                  <svg className="w-4 h-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                      d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                  </svg>
                  <span>Admin</span>
                </button>
                {users.map((user) => (
                  <button
                    key={user.id}
                    onClick={() => { select(user.id); setIsOpen(false); }}
                    className={`w-full flex items-center gap-3 px-3 py-3 rounded-lg transition-colors duration-200 active:scale-[0.98] ${
                      activeUserId === user.id ? "text-foreground bg-accent font-medium" : "text-muted-foreground hover:text-foreground hover:bg-accent"
                    }`}
                  >
                    <svg className="w-4 h-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                        d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                    </svg>
                    <span className="truncate">{user.username}</span>
                  </button>
                ))}
              </>
            )}
            <div className="border-t border-border/40 mt-2 pt-2">
              <Link
                href="/settings"
                className="flex items-center gap-3 px-3 py-3 rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-200 active:scale-[0.98]"
                onClick={() => setIsOpen(false)}
              >
                <NavIcon name="settings" />
                <span className="font-medium">{t("nav.settings")}</span>
              </Link>
            </div>
          </div>
        </nav>
      </div>
    </>
  );

  return (
    <>
      {/* Hamburger button — reste dans le header */}
      <button
        className="md:hidden p-2 rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
        onClick={() => setIsOpen(!isOpen)}
        aria-label={isOpen ? t("nav.closeMenu") : t("nav.openMenu")}
        aria-expanded={isOpen}
      >
        {isOpen ? <XIcon /> : <HamburgerIcon />}
      </button>

      {/* Backdrop + Drawer portés directement sur document.body,
          hors du header et de son backdrop-filter */}
      {mounted && createPortal(overlay, document.body)}
    </>
  );
}
