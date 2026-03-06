import type { Metadata } from "next";
import Image from "next/image";
import Link from "next/link";
import type { ReactNode } from "react";
import "./globals.css";
import { ThemeProvider } from "./theme-provider";
import { ThemeToggle } from "./theme-toggle";
import { JobsIndicator } from "./components/JobsIndicator";
import { NavIcon } from "./components/ui";

export const metadata: Metadata = {
  title: "Stripstream Backoffice",
  description: "Backoffice administration for Stripstream Librarian"
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className="min-h-screen bg-background text-foreground font-sans antialiased">
        <ThemeProvider>
          {/* Navigation */}
          <nav className="sticky top-0 z-50 w-full border-b border-line bg-card/80 backdrop-blur-md">
            <div className="container mx-auto flex h-16 items-center justify-between px-4">
              {/* Brand */}
              <Link href="/" className="flex items-center gap-3 hover:opacity-80 transition-opacity">
                <Image 
                  src="/logo.png" 
                  alt="Stripstream" 
                  width={36} 
                  height={36} 
                  className="rounded-lg"
                />
                <div className="flex items-baseline gap-2">
                  <span className="text-xl font-bold tracking-tight">StripStream</span>
                  <span className="text-sm text-muted font-medium">backoffice</span>
                </div>
              </Link>

              {/* Navigation Links */}
              <div className="flex items-center gap-6">
                <div className="hidden md:flex items-center gap-1">
                  <NavLink href="/">
                    <NavIcon name="dashboard" /> Dashboard
                  </NavLink>
                  <NavLink href="/books">
                    <NavIcon name="books" /> Books
                  </NavLink>
                  <NavLink href="/libraries">
                    <NavIcon name="libraries" /> Libraries
                  </NavLink>
                  <NavLink href="/jobs">
                    <NavIcon name="jobs" /> Jobs
                  </NavLink>
                  <NavLink href="/tokens">
                    <NavIcon name="tokens" /> Tokens
                  </NavLink>
                </div>
                
                <div className="flex items-center gap-3 pl-6 border-l border-line">
                  <JobsIndicator />
                  <ThemeToggle />
                </div>
              </div>
            </div>
          </nav>

          {/* Main Content */}
          <main className="container mx-auto px-4 py-8">
            {children}
          </main>
        </ThemeProvider>
      </body>
    </html>
  );
}

// Navigation Link Component
function NavLink({ href, children }: { href: "/" | "/books" | "/libraries" | "/jobs" | "/tokens"; children: React.ReactNode }) {
  return (
    <Link 
      href={href} 
      className="flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium text-foreground/80 hover:text-foreground hover:bg-primary-soft transition-colors"
    >
      {children}
    </Link>
  );
}
