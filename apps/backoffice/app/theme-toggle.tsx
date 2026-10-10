"use client";

import { useTheme } from "next-themes";

import { useIsMounted } from "@/lib/useIsMounted";

// Sun Icon
const SunIcon = ({ className }: { className?: string }) => (
  <svg className={className} fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="4" strokeWidth="2" />
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 2v2m0 16v2M4.93 4.93l1.41 1.41m11.32 11.32l1.41 1.41M2 12h2m16 0h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
  </svg>
);

// Moon Icon
const MoonIcon = ({ className }: { className?: string }) => (
  <svg className={className} fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
  </svg>
);

// Monitor Icon (for system)
const MonitorIcon = ({ className }: { className?: string }) => (
  <svg className={className} fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <rect x="2" y="3" width="20" height="14" rx="2" strokeWidth="2" />
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M8 21h8m-4-4v4" />
  </svg>
);

export function ThemeToggle() {
  const { theme, setTheme, resolvedTheme, systemTheme } = useTheme();
  const mounted = useIsMounted();

  if (!mounted) {
    return (
      <button
        type="button"
        className="h-9 w-9 rounded-md flex items-center justify-center text-muted-foreground"
        disabled
      >
        <SunIcon className="h-4 w-4" />
      </button>
    );
  }

  const activeTheme = theme === "system" ? resolvedTheme : theme;

  const toggleTheme = () => {
    if (theme === "system") {
      setTheme(systemTheme === "dark" ? "light" : "dark");
    } else {
      setTheme(theme === "dark" ? "light" : "dark");
    }
  };

  return (
    <button
      type="button"
      onClick={toggleTheme}
      className="h-9 w-9 rounded-md flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-200"
      title={activeTheme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
      aria-label={activeTheme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
    >
      <div className="relative h-4 w-4">
        <SunIcon 
          className={`absolute inset-0 h-4 w-4 transition-all duration-300 rotate-0 scale-100 ${
            activeTheme === "dark" ? "rotate-90 scale-0 opacity-0" : "rotate-0 scale-100 opacity-100"
          }`} 
        />
        <MoonIcon 
          className={`absolute inset-0 h-4 w-4 transition-all duration-300 -rotate-90 scale-0 ${
            activeTheme === "dark" ? "rotate-0 scale-100 opacity-100" : "-rotate-90 scale-0 opacity-0"
          }`} 
        />
      </div>
    </button>
  );
}

// Full theme selector with dropdown
export function ThemeSelector() {
  const { theme, setTheme } = useTheme();
  const mounted = useIsMounted();

  if (!mounted) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <SunIcon className="h-4 w-4" />
        <span>Theme</span>
      </div>
    );
  }

  const themes = [
    { value: "light", label: "Light", icon: SunIcon },
    { value: "dark", label: "Dark", icon: MoonIcon },
    { value: "system", label: "System", icon: MonitorIcon },
  ];

  return (
    <div className="flex items-center gap-1 rounded-lg border border-border bg-background p-1">
      {themes.map(({ value, label, icon: Icon }) => (
        <button
          key={value}
          onClick={() => setTheme(value)}
          className={`
            flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-sm font-medium
            transition-colors duration-200
            ${theme === value 
              ? "bg-accent text-accent-foreground" 
              : "text-muted-foreground hover:text-foreground hover:bg-accent/50"
            }
          `}
          title={label}
        >
          <Icon className="h-4 w-4" />
          <span className="hidden sm:inline">{label}</span>
        </button>
      ))}
    </div>
  );
}
