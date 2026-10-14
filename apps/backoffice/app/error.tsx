"use client";

import { ErrorView } from "@/app/components/ErrorView";

export default function RootError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  return <ErrorView error={error} reset={reset} />;
}
