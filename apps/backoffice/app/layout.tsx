import type { Metadata } from "next";
import Image from "next/image";
import Link from "next/link";
import type { ReactNode } from "react";
import "./globals.css";
import { ThemeProvider } from "./theme-provider";
import { ThemeToggle } from "./theme-toggle";

export const metadata: Metadata = {
  title: "Stripstream Backoffice",
  description: "Backoffice administration for Stripstream Librarian"
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body>
        <ThemeProvider>
          <nav className="top-nav">
            <Link href="/" className="brand">
              <Image src="/logo.png" alt="Stripstream" width={36} height={36} />
              <span className="brand-name">StripStream</span>
              <span className="brand-subtitle">backoffice</span>
            </Link>
            <div className="links-wrap">
              <div className="links">
                <Link href="/">Dashboard</Link>
                <Link href="/libraries">Libraries</Link>
                <Link href="/jobs">Jobs</Link>
                <Link href="/tokens">Tokens</Link>
              </div>
              <ThemeToggle />
            </div>
          </nav>
          <main>{children}</main>
        </ThemeProvider>
      </body>
    </html>
  );
}
