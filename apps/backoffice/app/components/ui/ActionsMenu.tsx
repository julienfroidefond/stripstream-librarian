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

import { useIsMounted } from "@/lib/useIsMounted";
import { iconActionClassName } from "./iconAction";

interface MenuContext {
  close: () => void;
  /** Denser spacing for the popin squeezed inside a cover. */
  compact: boolean;
}

const ActionsMenuContext = createContext<MenuContext | null>(null);

interface ActionsMenuProps {
  children: ReactNode;
  label?: string;
  /** Accessible name of the close control of the cover overlay. */
  closeLabel?: string;
  align?: "left" | "right";
  /**
   * Mount the popin contents only after the first open. Card-level menus use
   * this so the (heavy) modal children are not mounted for every card.
   */
  lazy?: boolean;
  /** Trigger look: labelled button (default), round pill over a cover, neutral square in a row. */
  variant?: "default" | "cover" | "inline";
}

const MOBILE_BREAKPOINT_PX = 640;

/**
 * Selector of the element a `cover` popin is rendered into (the card cover,
 * tagged by `CoverCard`). It then covers the artwork instead of opening a
 * dropdown that the viewport can clip — the bottom cards of a grid used to get
 * their menu pushed off-screen.
 */
export const COVER_ANCHOR_ATTRIBUTE = "data-cover-card";

export function ActionsMenu({
  children,
  label,
  closeLabel = "Fermer",
  align = "right",
  lazy = false,
  variant = "default",
}: ActionsMenuProps) {
  const [isOpen, setIsOpen] = useState(false);
  // With `lazy`, once the menu has been opened the contents stay mounted so the
  // child modals keep their state across open/close cycles.
  const [hasOpened, setHasOpened] = useState(false);
  const mounted = useIsMounted();
  const [isMobile, setIsMobile] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const popinRef = useRef<HTMLDivElement>(null);
  const [popinStyle, setPopinStyle] = useState<React.CSSProperties>({});
  // Cover the popin is rendered into for the `cover` variant, if any.
  const [coverAnchor, setCoverAnchor] = useState<HTMLElement | null>(null);

  const toggle = useCallback(() => {
    setHasOpened(true);
    setIsOpen((open) => !open);
  }, []);

  useEffect(() => {
    const updateBreakpoint = () => setIsMobile(window.innerWidth < MOBILE_BREAKPOINT_PX);
    updateBreakpoint();
    window.addEventListener("resize", updateBreakpoint);
    return () => window.removeEventListener("resize", updateBreakpoint);
  }, []);

  useEffect(() => {
    if (variant !== "cover") return;
    const anchor = buttonRef.current?.closest(`[${COVER_ANCHOR_ATTRIBUTE}]`);
    setCoverAnchor(anchor instanceof HTMLElement ? anchor : null);
  }, [variant]);

  const close = useCallback(() => setIsOpen(false), []);

  // The overlay covers the whole artwork: no viewport anchoring, no clipping.
  const isCoverOverlay = variant === "cover" && !isMobile && coverAnchor !== null;

  const updatePosition = useCallback(() => {
    if (!buttonRef.current || isMobile) return;
    const rect = buttonRef.current.getBoundingClientRect();
    const top = rect.bottom + 8;
    const maxHeight = Math.max(160, window.innerHeight - top - 12);
    const base: React.CSSProperties = {
      position: "fixed",
      top: `${top}px`,
      minWidth: "240px",
      maxHeight: `${maxHeight}px`,
      overflowY: "auto",
    };
    if (align === "right") {
      setPopinStyle({ ...base, right: `${Math.max(window.innerWidth - rect.right, 12)}px` });
    } else {
      setPopinStyle({ ...base, left: `${rect.left}px` });
    }
  }, [align, isMobile]);

  useEffect(() => {
    if (!isOpen || isMobile || isCoverOverlay) return;
    updatePosition();
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [isOpen, isMobile, isCoverOverlay, updatePosition]);

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
            ? isMobile || isCoverOverlay
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
            : isCoverOverlay
              ? `
                absolute inset-0 z-40
                flex flex-col
                bg-popover/95 backdrop-blur-md
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
        {isCoverOverlay && (
          <div className="flex shrink-0 items-center gap-2 border-b border-border/60 px-3 py-2">
            {label && (
              <span className="truncate text-xs font-semibold text-foreground">{label}</span>
            )}
            <button
              type="button"
              onClick={close}
              aria-label={closeLabel}
              title={closeLabel}
              className="ml-auto inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <svg className="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round">
                <path d="M6 6l12 12M18 6L6 18" />
              </svg>
            </button>
          </div>
        )}
        <div
          className={
            isMobile ? "py-1" : isCoverOverlay ? "min-h-0 flex-1 overflow-y-auto py-0.5" : ""
          }
        >
          <ActionsMenuContext.Provider value={{ close, compact: isCoverOverlay }}>
            {children}
          </ActionsMenuContext.Provider>
        </div>
      </div>
    </>
  );

  const iconOnly = variant !== "default";
  const accessibleLabel = label ?? (iconOnly ? "More" : "Plus");

  const triggerClassName =
    variant === "cover"
      ? `${iconActionClassName("cover")} ${isOpen ? "ring-2 ring-ring" : ""}`
      : variant === "inline"
        ? `${iconActionClassName("inline")} ${isOpen ? "ring-2 ring-ring ring-offset-2" : ""}`
        : `
          inline-flex items-center gap-1.5
          px-3 py-1.5 rounded-lg
          border border-border bg-card
          text-sm font-medium
          text-muted-foreground hover:text-foreground hover:border-primary
          transition-colors duration-200
          focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2
          ${isOpen ? "ring-2 ring-ring ring-offset-2 ring-offset-background" : ""}
        `;

  return (
    <>
      <button
        ref={buttonRef}
        type="button"
        onClick={toggle}
        className={triggerClassName}
        aria-haspopup="menu"
        aria-expanded={isOpen}
        aria-label={iconOnly ? accessibleLabel : undefined}
        title={iconOnly ? accessibleLabel : undefined}
      >
        <svg className="w-4 h-4" viewBox="0 0 24 24" fill="currentColor">
          <circle cx="5" cy="12" r="2" />
          <circle cx="12" cy="12" r="2" />
          <circle cx="19" cy="12" r="2" />
        </svg>
        {!iconOnly && (
          <>
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
          </>
        )}
      </button>
      {mounted &&
        (!lazy || hasOpened) &&
        createPortal(popin, isCoverOverlay && coverAnchor ? coverAnchor : document.body)}
    </>
  );
}

interface ActionsMenuItemProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children" | "onClick"> {
  icon?: ReactNode;
  children: ReactNode;
  variant?: "default" | "danger";
  /** When true, prevents the menu from closing on click (useful while a sub-modal opens). */
  keepOpen?: boolean;
  /** Render the item as a link (e.g. a file download) instead of a button. */
  href?: string;
  /** Forwarded to the anchor when `href` is set. */
  download?: string | boolean;
  onClick?: (e: React.MouseEvent<HTMLElement>) => void;
}

export function ActionsMenuItem({
  icon,
  children,
  variant = "default",
  keepOpen = false,
  onClick,
  className = "",
  href,
  download,
  ...rest
}: ActionsMenuItemProps) {
  const ctx = useContext(ActionsMenuContext);
  const baseClass =
    variant === "danger"
      ? "text-destructive hover:bg-destructive/10"
      : "text-foreground hover:bg-accent";

  const classes = `
        flex items-center gap-2.5
        w-full ${
          ctx?.compact ? "px-3 py-2 text-xs" : "px-4 py-3 sm:px-3 sm:py-2 text-sm"
        } text-left
        transition-colors
        disabled:opacity-50 disabled:pointer-events-none
        ${baseClass}
        ${className}
      `;

  const handleClick = (e: React.MouseEvent<HTMLElement>) => {
    onClick?.(e);
    if (!keepOpen) ctx?.close();
  };

  const content = (
    <>
      {icon && <span className="flex-shrink-0 w-4 h-4 flex items-center justify-center">{icon}</span>}
      <span className="flex-1 truncate">{children}</span>
    </>
  );

  if (href) {
    return (
      <a role="menuitem" href={href} download={download} onClick={handleClick} className={classes}>
        {content}
      </a>
    );
  }

  return (
    <button type="button" role="menuitem" onClick={handleClick} className={classes} {...rest}>
      {content}
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
        <div className="px-3 pt-1.5 pb-1 text-3xs font-semibold uppercase tracking-wider text-muted-foreground">
          {label}
        </div>
      )}
      {children}
    </div>
  );
}
