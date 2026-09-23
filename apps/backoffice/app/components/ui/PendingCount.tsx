"use client";

import { useLinkStatus } from "next/link";

import { Icon } from "./Icon";

interface PendingCountProps {
  count: number;
  className?: string;
}

/**
 * Renders a chip's counter, swapping it for a spinner while the surrounding
 * `<Link>` navigation is in flight.
 *
 * Must be rendered inside a `<Link>`: `useLinkStatus` reports the pending state
 * of its closest ancestor link. This keeps the page a Server Component while
 * still giving immediate feedback on filter clicks.
 */
export function PendingCount({ count, className = "text-xs font-semibold tabular-nums" }: PendingCountProps) {
  const { pending } = useLinkStatus();

  if (pending) {
    return <Icon name="spinner" size="sm" className="animate-spin" />;
  }

  return <span className={className}>{count}</span>;
}
