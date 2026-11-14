import { NextRequest } from "next/server";
import { config } from "@/lib/api";
import { isTerminalJobStatus } from "@/lib/jobs/status";

const POLL_INTERVAL_MS = 500;

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;
  const { baseUrl, token } = config();
  const encoder = new TextEncoder();

  const stream = new ReadableStream({
    async start(controller) {
      // Send initial headers for SSE
      controller.enqueue(encoder.encode(""));

      let lastData: string | null = null;
      let isActive = true;
      let consecutiveErrors = 0;
      let interval: ReturnType<typeof setInterval> | null = null;

      /** Idempotent teardown: terminal status, abort and enqueue failure may race. */
      const close = () => {
        if (!isActive) return;
        isActive = false;
        if (interval !== null) clearInterval(interval);
        try {
          controller.close();
        } catch {
          // Already closed by the runtime.
        }
      };

      // Registered before the first await so an early disconnect is not missed.
      if (request.signal.aborted) {
        close();
        return;
      }
      request.signal.addEventListener("abort", close, { once: true });

      const fetchJob = async () => {
        if (!isActive) return;

        try {
          const response = await fetch(`${baseUrl}/index/jobs/${id}`, {
            headers: { Authorization: `Bearer ${token}` },
          });

          if (response.ok && isActive) {
            consecutiveErrors = 0;
            const data = await response.json();
            const dataStr = JSON.stringify(data);

            // Only send if data changed
            if (dataStr !== lastData && isActive) {
              lastData = dataStr;
              try {
                controller.enqueue(encoder.encode(`data: ${dataStr}\n\n`));
              } catch {
                close();
                return;
              }

              // Stop polling if job is complete
              if (isTerminalJobStatus(data.status)) close();
            }
          }
        } catch (error) {
          if (isActive) {
            consecutiveErrors++;
            // Only log first failure and every 60th to avoid spam
            if (consecutiveErrors === 1 || consecutiveErrors % 60 === 0) {
              console.warn(`SSE fetch error (${consecutiveErrors} consecutive):`, error);
            }
          }
        }
      };

      // Initial fetch (errors are swallowed above: polling starts regardless).
      await fetchJob();
      if (!isActive) return;

      // Poll while the job is active
      interval = setInterval(() => {
        void fetchJob();
      }, POLL_INTERVAL_MS);
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
