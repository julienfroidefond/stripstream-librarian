"use client";

import { useOptimistic, useTransition } from "react";

interface TokenUserSelectProps {
  tokenId: string;
  currentUserId?: string;
  users: { id: string; username: string }[];
  action: (formData: FormData) => Promise<void>;
  noUserLabel: string;
}

export function TokenUserSelect({ tokenId, currentUserId, users, action, noUserLabel }: TokenUserSelectProps) {
  const [optimisticValue, setOptimisticValue] = useOptimistic(currentUserId ?? "");
  const [, startTransition] = useTransition();

  return (
    <select
      value={optimisticValue}
      onChange={(e) => {
        const newValue = e.target.value;
        startTransition(async () => {
          setOptimisticValue(newValue);
          const fd = new FormData();
          fd.append("id", tokenId);
          fd.append("user_id", newValue);
          await action(fd);
        });
      }}
      className="flex h-8 rounded-md border border-input bg-background px-2 py-0 text-xs shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
    >
      <option value="">{noUserLabel}</option>
      {users.map((u) => (
        <option key={u.id} value={u.id}>{u.username}</option>
      ))}
    </select>
  );
}
