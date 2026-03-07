import type { Metadata } from "next";
import Image from "next/image";
import Link from "next/link";
import type { ReactNode } from "react";
import "./globals.css";
import { ThemeProvider } from "./theme-provider";
import { ThemeToggle } from "./theme-toggle";
import { JobsIndicator } from "./components/JobsIndicator";
import { NavIcon, Icon } from "./components/ui";

export const metadata: Metadata = {
  title: "StripStream Backoffice",
  description: "Backoffice administration for StripStream Librarian"
};

type NavItem = {
  href: "/" | "/books" | "/libraries" | "/jobs" | "/tokens" | "/settings";
  label: string;
  icon: "dashboard" | "books" | "libraries" | "jobs" | "tokens" | "settings";
};

const navItems: NavItem[] = [
  { href: "/", label: "Dashboard", icon: "dashboard" },
  { href: "/books", label: "Books", icon: "books" },
  { href: "/libraries", label: "Libraries", icon: "libraries" },
  { href: "/jobs", label: "Jobs", icon: "jobs" },
  { href: "/tokens", label: "Tokens", icon: "tokens" },
];

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className="min-h-screen bg-background text-foreground font-sans antialiased bg-grain">
        <ThemeProvider>
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
                  <span className="text-sm text-muted-foreground font-medium">
                    backoffice
                  </span>
                </div>
              </Link>

              {/* Navigation Links */}
              <div className="flex items-center gap-2">
                <div className="hidden md:flex items-center gap-1">
                  {navItems.map((item) => (
                    <NavLink key={item.href} href={item.href}>
                      <NavIcon name={item.icon} /> 
                      <span className="ml-2">{item.label}</span>
                    </NavLink>
                  ))}
                </div>
                
                {/* Actions */}
                <div className="flex items-center gap-1 pl-4 ml-2 border-l border-border/60">
                  <JobsIndicator />
                  <Link 
                    href="/settings"
                    className="p-2 rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
                    title="Settings"
                  >
                    <Icon name="settings" size="md" />
                  </Link>
                  <ThemeToggle />
                </div>
              </div>
            </nav>
          </header>

          {/* Main Content */}
          <main className="container mx-auto px-4 sm:px-6 lg:px-8 py-8 pb-16">
            {children}
          </main>
        </ThemeProvider>
      </body>
    </html>
  );
}

// Navigation Link Component
function NavLink({ href, children }: { href: NavItem["href"]; children: React.ReactNode }) {
  return (
    <Link 
      href={href}
      className="
        flex items-center 
        px-3 py-2 
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
