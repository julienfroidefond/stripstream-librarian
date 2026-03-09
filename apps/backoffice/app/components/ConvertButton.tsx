"use client";

import { useState } from "react";
import Link from "next/link";
import { Button } from "./ui";

interface ConvertButtonProps {
  bookId: string;
}

type ConvertState =
  | { type: "idle" }
  | { type: "loading" }
  | { type: "success"; jobId: string }
  | { type: "error"; message: string };

export function ConvertButton({ bookId }: ConvertButtonProps) {
  const [state, setState] = useState<ConvertState>({ type: "idle" });

  const handleConvert = async () => {
    setState({ type: "loading" });
    try {
      const res = await fetch(`/api/books/${bookId}/convert`, { method: "POST" });
      if (!res.ok) {
        const body = await res.json().catch(() => ({ error: res.statusText }));
        setState({ type: "error", message: body.error || "Conversion failed" });
        return;
      }
      const job = await res.json();
      setState({ type: "success", jobId: job.id });
    } catch (err) {
      setState({ type: "error", message: err instanceof Error ? err.message : "Unknown error" });
    }
  };

  if (state.type === "success") {
    return (
      <div className="flex items-center gap-2 text-sm text-success">
        <span>Conversion started.</span>
        <Link href={`/jobs/${state.jobId}`} className="text-primary hover:underline font-medium">
          View job →
        </Link>
      </div>
    );
  }

  if (state.type === "error") {
    return (
      <div className="flex flex-col gap-1">
        <span className="text-sm text-destructive">{state.message}</span>
        <button
          className="text-xs text-muted-foreground hover:underline text-left"
          onClick={() => setState({ type: "idle" })}
        >
          Dismiss
        </button>
      </div>
    );
  }

  return (
    <Button
      variant="secondary"
      size="sm"
      onClick={handleConvert}
      disabled={state.type === "loading"}
    >
      {state.type === "loading" ? "Converting…" : "Convert to CBZ"}
    </Button>
  );
}
