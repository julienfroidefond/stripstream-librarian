import { NextRequest } from "next/server";
import { config } from "@/lib/api";

export async function GET(request: NextRequest) {
  const { baseUrl, token } = config();

  const stream = new ReadableStream({
    async start(controller) {
      controller.enqueue(new TextEncoder().encode(""));

      let lastData: string | null = null;
      let isActive = true;
      let consecutiveErrors = 0;
      let intervalId: ReturnType<typeof setInterval> | null = null;
      let heartbeatId: ReturnType<typeof setInterval> | null = null;

      const send = (msg: string): boolean => {
        try {
          controller.enqueue(new TextEncoder().encode(msg));
          return true;
        } catch {
          isActive = false;
          return false;
        }
      };

      const fetchJobs = async () => {
        if (!isActive) return;

        try {
          const response = await fetch(`${baseUrl}/index/status`, {
            headers: { Authorization: `Bearer ${token}` },
          });

          if (response.ok && isActive) {
            consecutiveErrors = 0;
            const data = await response.json();
            const dataStr = JSON.stringify(data);

            // Always send data (client needs fresh timestamps for active jobs)
            if (isActive) {
              lastData = dataStr;
              send(`data: ${dataStr}\n\n`);
            }

            // Adapt interval: 2s when active jobs exist, 10s when idle
            const hasActiveJobs = data.some((j: { status: string }) =>
              j.status === "running" || j.status === "pending" || j.status === "extracting_pages" || j.status === "generating_thumbnails"
            );
            const nextInterval = hasActiveJobs ? 2000 : 10000;
            restartInterval(nextInterval);
          }
        } catch (error) {
          if (isActive) {
            consecutiveErrors++;
            if (consecutiveErrors === 1 || consecutiveErrors % 30 === 0) {
              console.warn(`SSE fetch error (${consecutiveErrors} consecutive):`, error);
            }
          }
        }
      };

      const restartInterval = (ms: number) => {
        if (intervalId !== null) clearInterval(intervalId);
        intervalId = setInterval(fetchJobs, ms);
      };

      // Heartbeat every 15s to keep connection alive
      heartbeatId = setInterval(() => {
        if (isActive) send(": heartbeat\n\n");
      }, 15000);

      // Initial fetch + start polling
      await fetchJobs();

      // Cleanup
      request.signal.addEventListener("abort", () => {
        isActive = false;
        if (intervalId !== null) clearInterval(intervalId);
        if (heartbeatId !== null) clearInterval(heartbeatId);
        controller.close();
      });
    },
  });

  return new Response(stream, {
    headers: {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      "Connection": "keep-alive",
    },
  });
}
