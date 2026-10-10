"use client";

import { createContext, useCallback, useContext, useSyncExternalStore } from "react";

const STORAGE_KEY = "nav-collapsed";

// Module-level store backed by localStorage. Using an external store (instead of
// useState + useEffect) keeps the persisted value in sync without synchronously
// calling setState inside an effect.
const listeners = new Set<() => void>();

function subscribe(listener: () => void) {
  listeners.add(listener);
  const onStorage = (e: StorageEvent) => {
    if (e.key === STORAGE_KEY) listener();
  };
  window.addEventListener("storage", onStorage);
  return () => {
    listeners.delete(listener);
    window.removeEventListener("storage", onStorage);
  };
}

function getSnapshot(): boolean {
  return localStorage.getItem(STORAGE_KEY) !== "false";
}

function getServerSnapshot(): boolean {
  return true;
}

function setCollapsed(next: boolean) {
  localStorage.setItem(STORAGE_KEY, String(next));
  listeners.forEach((listener) => listener());
}

const NavCollapseContext = createContext<{ collapsed: boolean; toggle: () => void }>({
  collapsed: false,
  toggle: () => {},
});

export function CollapsibleNavProvider({ children }: { children: React.ReactNode }) {
  const collapsed = useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);

  const toggle = useCallback(() => {
    setCollapsed(!getSnapshot());
  }, []);

  return (
    <NavCollapseContext.Provider value={{ collapsed, toggle }}>
      {children}
    </NavCollapseContext.Provider>
  );
}

export function NavToggleButton() {
  const { collapsed, toggle } = useContext(NavCollapseContext);

  return (
    <button
      type="button"
      onClick={toggle}
      className="
        hidden md:flex items-center justify-center
        w-7 h-7
        rounded-md
        text-muted-foreground/60 hover:text-muted-foreground
        hover:bg-accent
        transition-colors duration-200
      "
      aria-label={collapsed ? "Show navigation" : "Hide navigation"}
    >
      <svg className="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
        {collapsed ? (
          <>
            <path d="M4 6h16M4 12h16M4 18h16" strokeLinecap="round" />
          </>
        ) : (
          <>
            <path d="M4 6h16M4 12h10M4 18h14" strokeLinecap="round" />
          </>
        )}
      </svg>
    </button>
  );
}

export function CollapsibleNav({ children }: { children: React.ReactNode }) {
  const { collapsed } = useContext(NavCollapseContext);

  if (collapsed) return null;

  return (
    <div className="hidden md:flex container mx-auto items-center justify-center gap-1 px-4 pb-2">
      {children}
    </div>
  );
}
