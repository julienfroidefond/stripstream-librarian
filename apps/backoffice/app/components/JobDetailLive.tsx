"use client";

import { useEffect, useRef } from "react";
import { useRouter } from "next/navigation";
import { isTerminalJobStatus } from "@/lib/jobStatus";

interface JobDetailLiveProps {
  jobId: string;
  isTerminal: boolean;
}

export function JobDetailLive({ jobId, isTerminal }: JobDetailLiveProps) {
  const router = useRouter();
  const isTerminalRef = useRef(isTerminal);

  useEffect(() => {
    isTerminalRef.current = isTerminal;
  }, [isTerminal]);

  useEffect(() => {
    if (isTerminalRef.current) return;

    const eventSource = new EventSource(`/api/jobs/${jobId}/stream`);

    eventSource.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        router.refresh();

        if (isTerminalJobStatus(data.status)) {
          eventSource.close();
        }
      } catch {
        // ignore parse errors
      }
    };

    eventSource.onerror = () => {
      eventSource.close();
    };

    return () => {
      eventSource.close();
    };
  }, [jobId, router]);

  return null;
}
