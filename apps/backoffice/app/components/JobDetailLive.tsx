"use client";

import { useRouter } from "next/navigation";

import { useJobStream } from "@/lib/jobs/useJobStream";

interface JobDetailLiveProps {
  jobId: string;
  isTerminal: boolean;
}

/** Re-renders the job detail page while the job is still running. */
export function JobDetailLive({ jobId, isTerminal }: JobDetailLiveProps) {
  const router = useRouter();

  useJobStream(jobId, {
    enabled: !isTerminal,
    onMessage: () => router.refresh(),
  });

  return null;
}
