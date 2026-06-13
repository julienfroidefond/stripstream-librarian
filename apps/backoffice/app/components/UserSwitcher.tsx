"use client";

import { useState, useTransition, useRef, useEffect } from "react";
import { useRouter } from "next/navigation";
import type { UserDto } from "@/lib/api";
import { Icon } from "./ui";

export function UserSwitcher({
  users,
  activeUserId,
  setActiveUserAction,
}: {
  users: UserDto[];
  activeUserId: string | null;
  setActiveUserAction: (formData: FormData) => Promise<void>;
}) {
  const [open, setOpen] = useState(false);
  const [, startTransition] = useTransition();
  const ref = useRef<HTMLDivElement>(null);
  const router = useRouter();

  const activeUser = users.find((u) => u.id === activeUserId) ?? null;

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  function select(userId: string | null) {
    setOpen(false);
    startTransition(async () => {
      const fd = new FormData();
      fd.append("user_id", userId ?? "");
      await setActiveUserAction(fd);
      router.refresh();
    });
  }

  if (users.length === 0) return null;

  const isImpersonating = activeUserId !== null;

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs font-medium border transition-colors ${
          isImpersonating
            ? "border-primary/40 bg-primary/10 text-primary hover:bg-primary/15"
            : "border-border/60 bg-muted/40 text-muted-foreground hover:text-foreground hover:bg-accent"
        }`}
      >
        {isImpersonating ? (
          <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
              d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
          </svg>
        ) : (
          <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
              d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
          </svg>
        )}
        <span className="max-w-[80px] truncate hidden sm:inline">
          {activeUser ? activeUser.username : "Admin"}
        </span>
        <Icon name="chevronDown" size="sm" className="!w-3 !h-3 opacity-60" />
      </button>

      {open && (
        <div className="absolute right-0 top-full mt-1.5 w-44 rounded-lg border border-border/60 bg-popover shadow-lg z-50 overflow-hidden py-1">
          <button
            onClick={() => select(null)}
            className={`w-full flex items-center gap-2.5 px-3 py-2 text-sm transition-colors ${
              !isImpersonating
                ? "bg-accent text-foreground font-medium"
                : "text-muted-foreground hover:bg-accent hover:text-foreground"
            }`}
          >
            <svg className="w-4 h-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
            </svg>
            Admin
            {!isImpersonating && (
              <Icon name="check" size="sm" className="!w-3.5 !h-3.5 ml-auto text-primary" />
            )}
          </button>

          <div className="h-px bg-border/60 my-1" />

          {users.map((user) => (
            <button
              key={user.id}
              onClick={() => select(user.id)}
              className={`w-full flex items-center gap-2.5 px-3 py-2 text-sm transition-colors ${
                activeUserId === user.id
                  ? "bg-accent text-foreground font-medium"
                  : "text-muted-foreground hover:bg-accent hover:text-foreground"
              }`}
            >
              <svg className="w-4 h-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                  d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
              </svg>
              <span className="truncate">{user.username}</span>
              {activeUserId === user.id && (
                <Icon name="check" size="sm" className="!w-3.5 !h-3.5 ml-auto text-primary shrink-0" />
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
