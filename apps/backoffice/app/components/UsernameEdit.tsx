"use client";

import { useOptimistic, useTransition, useRef, useState } from "react";

export function UsernameEdit({
  userId,
  currentUsername,
  action,
}: {
  userId: string;
  currentUsername: string;
  action: (formData: FormData) => Promise<void>;
}) {
  const [optimisticUsername, setOptimisticUsername] = useOptimistic(currentUsername);
  const [editing, setEditing] = useState(false);
  const [, startTransition] = useTransition();
  const inputRef = useRef<HTMLInputElement>(null);

  function startEdit() {
    setEditing(true);
    setTimeout(() => inputRef.current?.select(), 0);
  }

  function submit(value: string) {
    const trimmed = value.trim();
    if (!trimmed || trimmed === currentUsername) {
      setEditing(false);
      return;
    }
    setEditing(false);
    startTransition(async () => {
      setOptimisticUsername(trimmed);
      const fd = new FormData();
      fd.append("id", userId);
      fd.append("username", trimmed);
      await action(fd);
    });
  }

  if (editing) {
    return (
      <input
        ref={inputRef}
        defaultValue={optimisticUsername}
        className="text-sm font-medium text-foreground bg-background border border-border rounded px-2 py-0.5 focus:outline-none focus:ring-1 focus:ring-primary w-36"
        autoFocus
        onBlur={(e) => submit(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") submit((e.target as HTMLInputElement).value);
          if (e.key === "Escape") setEditing(false);
        }}
      />
    );
  }

  return (
    <button
      onClick={startEdit}
      className="flex items-center gap-1.5 group/edit text-left"
      title="Modifier"
    >
      <span className="text-sm font-medium text-foreground">{optimisticUsername}</span>
      <svg
        className="w-3.5 h-3.5 text-muted-foreground opacity-0 group-hover/edit:opacity-100 transition-opacity shrink-0"
        fill="none" stroke="currentColor" viewBox="0 0 24 24"
      >
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
          d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"
        />
      </svg>
    </button>
  );
}
