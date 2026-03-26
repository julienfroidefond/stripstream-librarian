"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

export function NavLink({
  href,
  title,
  children,
}: {
  href: string;
  title?: string;
  children: React.ReactNode;
}) {
  const pathname = usePathname();
  const isActive = pathname === href || (href !== "/" && pathname.startsWith(href));

  return (
    <Link
      href={href as "/"}
      title={title}
      className={`
        flex items-center
        px-2 lg:px-3 py-2
        rounded-lg
        text-sm font-medium
        transition-colors duration-200
        active:scale-[0.98]
        ${isActive
          ? "text-primary bg-primary/10 hover:bg-primary/15"
          : "text-muted-foreground hover:text-foreground hover:bg-accent"
        }
      `}
    >
      {children}
    </Link>
  );
}
