"use client";

import { useEffect, useRef, useState } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { Icon, NavIcon } from "./ui";
import type { IconName } from "./ui/Icon";
import type { NavHref } from "@/lib/navigation";

export type NavDropdownItem = {
  href: NavHref;
  label: string;
  icon: IconName;
  color?: string;
};

function isActivePath(pathname: string, href: string) {
  return pathname === href || (href !== "/" && pathname.startsWith(href));
}

export function NavDropdown({
  label,
  icon,
  color,
  items,
}: {
  label: string;
  icon: IconName;
  color?: string;
  items: NavDropdownItem[];
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const pathname = usePathname();
  const isActive = items.some((item) => isActivePath(pathname, item.href));

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    function handleEscape(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", handleClickOutside);
    document.addEventListener("keydown", handleEscape);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleEscape);
    };
  }, []);

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        title={label}
        aria-haspopup="menu"
        aria-expanded={open}
        className={`flex items-center px-2 lg:px-3 py-2 rounded-lg text-sm font-medium transition-colors duration-200 active:scale-[0.98] ${
          isActive
            ? "text-primary bg-primary/10 hover:bg-primary/15"
            : "text-muted-foreground hover:text-foreground hover:bg-accent"
        }`}
      >
        <NavIcon name={icon} className={color} />
        <span className="ml-2 hidden xl:inline">{label}</span>
        <Icon name="chevronDown" size="sm" className="!w-3 !h-3 ml-1 opacity-60" />
      </button>

      {open && (
        <div
          role="menu"
          className="absolute left-1/2 -translate-x-1/2 top-full mt-1.5 w-44 rounded-lg border border-border/60 bg-popover shadow-lg z-50 overflow-hidden py-1"
        >
          {items.map((item) => (
            <Link
              key={item.href}
              href={item.href}
              role="menuitem"
              onClick={() => setOpen(false)}
              className={`flex items-center gap-2.5 px-3 py-2 text-sm transition-colors ${
                isActivePath(pathname, item.href)
                  ? "bg-accent text-foreground font-medium"
                  : "text-muted-foreground hover:bg-accent hover:text-foreground"
              }`}
            >
              <Icon name={item.icon} size="sm" className={item.color} />
              <span className="truncate">{item.label}</span>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
