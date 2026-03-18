"use client";

import { useState, useEffect } from "react";
import { createPortal } from "react-dom";
import Link from "next/link";
import { NavIcon } from "./ui";
import { useTranslation } from "../../lib/i18n/context";

type NavItem = {
  href: "/" | "/books" | "/series" | "/libraries" | "/jobs" | "/tokens" | "/settings";
  label: string;
  icon: "dashboard" | "books" | "series" | "libraries" | "jobs" | "tokens" | "settings";
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

export function MobileNav({ navItems }: { navItems: NavItem[] }) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [mounted, setMounted] = useState(false);

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
