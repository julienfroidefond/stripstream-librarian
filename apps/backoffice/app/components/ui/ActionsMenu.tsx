"use client";

import {
  ButtonHTMLAttributes,
  ReactNode,
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";

interface MenuContext {
  close: () => void;
}

const ActionsMenuContext = createContext<MenuContext | null>(null);

interface ActionsMenuProps {
  children: ReactNode;
  label?: string;
  align?: "left" | "right";
}

const MOBILE_BREAKPOINT_PX = 640;

export function ActionsMenu({ children, label, align = "right" }: ActionsMenuProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [mounted, setMounted] = useState(false);
  const [isMobile, setIsMobile] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const popinRef = useRef<HTMLDivElement>(null);
  const [popinStyle, setPopinStyle] = useState<React.CSSProperties>({});

  useEffect(() => {
    setMounted(true);
    const updateBreakpoint = () => setIsMobile(window.innerWidth < MOBILE_BREAKPOINT_PX);
    updateBreakpoint();
    window.addEventListener("resize", updateBreakpoint);
    return () => window.removeEventListener("resize", updateBreakpoint);
  }, []);

  const close = useCallback(() => setIsOpen(false), []);

  const updatePosition = useCallback(() => {
    if (!buttonRef.current || isMobile) return;
    const rect = buttonRef.current.getBoundingClientRect();
    if (align === "right") {
      const rightEdge = window.innerWidth - rect.right;
      setPopinStyle({
        position: "fixed",
        top: `${rect.bottom + 8}px`,
        right: `${Math.max(rightEdge, 12)}px`,
        minWidth: "240px",
      });
    } else {
      setPopinStyle({
        position: "fixed",
        top: `${rect.bottom + 8}px`,
        left: `${rect.left}px`,
        minWidth: "240px",
      });
    }
  }, [align, isMobile]);

  useEffect(() => {
    if (!isOpen || isMobile) return;
    updatePosition();
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [isOpen, isMobile, updatePosition]);

  // Lock body scroll while the bottom sheet is open on mobile
  useEffect(() => {
    if (!isOpen || !isMobile) return;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  }, [isOpen, isMobile]);

  useEffect(() => {
    if (!isOpen) return;
    const onClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        buttonRef.current &&
        !buttonRef.current.contains(target) &&
        popinRef.current &&
        !popinRef.current.contains(target)
      ) {
        setIsOpen(false);
      }
    };
    const onEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") setIsOpen(false);
    };
    document.addEventListener("mousedown", onClickOutside);
    document.addEventListener("keydown", onEsc);
    return () => {
      document.removeEventListener("mousedown", onClickOutside);
      document.removeEventListener("keydown", onEsc);
    };
  }, [isOpen]);

  // Always-mounted popin: hiding via `display: none` keeps child modal-trigger
  // components mounted so their internal state survives menu open/close cycles.
  // Modals render via portal anyway — they remain visible when the menu hides.
  // On mobile, we render as a bottom sheet (full width, slides from bottom)
  // with a backdrop. On desktop, the classic anchored dropdown.
  const popin = (
    <>
      {/* Mobile-only backdrop */}
      {isMobile && (
        <div
          onClick={close}
          aria-hidden="true"
          style={{ display: isOpen ? undefined : "none" }}
          className="fixed inset-0 z-[80] bg-background/60 backdrop-blur-sm animate-fade-in"
        />
      )}
      <div
        ref={popinRef}
        style={
          isOpen
            ? isMobile
              ? undefined
              : popinStyle
            : { display: "none" }
        }
        className={
          isMobile
            ? `
              fixed inset-x-0 bottom-0 z-[90]
              bg-popover/95 backdrop-blur-md
              rounded-t-2xl
              shadow-elevation-2
              border-t border-border/60
              max-h-[85vh] overflow-y-auto
              pb-[env(safe-area-inset-bottom)]
              animate-fade-in
            `
            : `
              z-[90]
              bg-popover/95 backdrop-blur-md
              rounded-xl
              shadow-elevation-2
              border border-border/60
              overflow-hidden
              animate-fade-in
              py-1
            `
        }
        role="menu"
        aria-hidden={!isOpen}
      >
        {isMobile && (
          <>
            {/* Drag handle (visual only) */}
            <div className="flex justify-center pt-2 pb-1">
              <div className="w-10 h-1 rounded-full bg-muted-foreground/30" />
            </div>
            {label && (
              <div className="px-4 py-2 text-sm font-semibold text-foreground border-b border-border/60">
                {label}
              </div>
            )}
          </>
        )}
        <div className={isMobile ? "py-1" : ""}>
          <ActionsMenuContext.Provider value={{ close }}>{children}</ActionsMenuContext.Provider>
        </div>
      </div>
    </>
  );

  return (
    <>
      <button
        ref={buttonRef}
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={`
          inline-flex items-center gap-1.5
          px-3 py-1.5 rounded-lg
          border border-border bg-card
          text-sm font-medium
          text-muted-foreground hover:text-foreground hover:border-primary
          transition-colors duration-200
          focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2
          ${isOpen ? "ring-2 ring-ring ring-offset-2 ring-offset-background" : ""}
        `}
        aria-haspopup="menu"
        aria-expanded={isOpen}
      >
        <svg className="w-4 h-4" viewBox="0 0 24 24" fill="currentColor">
          <circle cx="5" cy="12" r="2" />
          <circle cx="12" cy="12" r="2" />
          <circle cx="19" cy="12" r="2" />
        </svg>
        <span>{label ?? "Plus"}</span>
        <svg
          className={`w-3 h-3 transition-transform duration-200 ${isOpen ? "rotate-180" : ""}`}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth={2}
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M6 9l6 6 6-6" />
        </svg>
      </button>
      {mounted && createPortal(popin, document.body)}
    </>
  );
}

interface ActionsMenuItemProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  icon?: ReactNode;
  children: ReactNode;
  variant?: "default" | "danger";
  /** When true, prevents the menu from closing on click (useful while a sub-modal opens). */
  keepOpen?: boolean;
}

export function ActionsMenuItem({
  icon,
  children,
  variant = "default",
  keepOpen = false,
  onClick,
  className = "",
  ...rest
}: ActionsMenuItemProps) {
  const ctx = useContext(ActionsMenuContext);
  const baseClass =
    variant === "danger"
      ? "text-destructive hover:bg-destructive/10"
      : "text-foreground hover:bg-accent";

  return (
    <button
      type="button"
      role="menuitem"
      onClick={(e) => {
        onClick?.(e);
        if (!keepOpen) ctx?.close();
      }}
      className={`
        flex items-center gap-2.5
        w-full px-4 py-3 sm:px-3 sm:py-2 text-sm text-left
        transition-colors
        disabled:opacity-50 disabled:pointer-events-none
        ${baseClass}
        ${className}
      `}
      {...rest}
    >
      {icon && <span className="flex-shrink-0 w-4 h-4 flex items-center justify-center">{icon}</span>}
      <span className="flex-1 truncate">{children}</span>
    </button>
  );
}

interface ActionsMenuSectionProps {
  label?: string;
  children: ReactNode;
}

export function ActionsMenuSection({ label, children }: ActionsMenuSectionProps) {
  return (
    <div className="py-1 first:pt-0 last:pb-0 border-t border-border/60 first:border-t-0">
      {label && (
        <div className="px-3 pt-1.5 pb-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          {label}
        </div>
      )}
      {children}
    </div>
  );
}
