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

export function ActionsMenu({ children, label, align = "right" }: ActionsMenuProps) {
  const [isOpen, setIsOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const popinRef = useRef<HTMLDivElement>(null);
  const [popinStyle, setPopinStyle] = useState<React.CSSProperties>({});

  const close = useCallback(() => setIsOpen(false), []);

  const updatePosition = useCallback(() => {
    if (!buttonRef.current) return;
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
  }, [align]);

  useEffect(() => {
    if (!isOpen) return;
    updatePosition();
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [isOpen, updatePosition]);

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
  const popin = (
    <div
      ref={popinRef}
      style={isOpen ? popinStyle : { display: "none" }}
      className="
        z-[90]
        bg-popover/95 backdrop-blur-md
        rounded-xl
        shadow-elevation-2
        border border-border/60
        overflow-hidden
        animate-fade-in
        py-1
      "
      role="menu"
      aria-hidden={!isOpen}
    >
      <ActionsMenuContext.Provider value={{ close }}>{children}</ActionsMenuContext.Provider>
    </div>
  );

  return (
    <>
      <button
        ref={buttonRef}
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={`
          inline-flex items-center justify-center
          h-9 w-9 rounded-md
          ring-1 ring-input bg-background
          text-muted-foreground hover:text-foreground hover:bg-accent
          transition-colors duration-200
          focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2
          ${isOpen ? "ring-2 ring-ring ring-offset-2 ring-offset-background" : ""}
        `}
        aria-haspopup="menu"
        aria-expanded={isOpen}
        aria-label={label ?? "Menu"}
      >
        <svg className="w-4 h-4" viewBox="0 0 24 24" fill="currentColor">
          <circle cx="12" cy="5" r="2" />
          <circle cx="12" cy="12" r="2" />
          <circle cx="12" cy="19" r="2" />
        </svg>
      </button>
      {typeof document !== "undefined" && createPortal(popin, document.body)}
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
        w-full px-3 py-2 text-sm text-left
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
