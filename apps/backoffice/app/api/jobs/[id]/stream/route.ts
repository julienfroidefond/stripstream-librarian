import { NextRequest } from "next/server";
import { config } from "@/lib/api";

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;
  const { baseUrl, token } = config();

  const stream = new ReadableStream({
    async start(controller) {
      // Send initial headers for SSE
      controller.enqueue(new TextEncoder().encode(""));
      
      let lastData: string | null = null;
      let isActive = true;
      let consecutiveErrors = 0;

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
                controller.enqueue(
                  new TextEncoder().encode(`data: ${dataStr}\n\n`)
                );
              } catch (err) {
                // Controller closed, ignore
                isActive = false;
                return;
              }

              // Stop polling if job is complete
              if (data.status === "success" || data.status === "failed" || data.status === "cancelled") {
                isActive = false;
                try {
                  controller.close();
                } catch (err) {
                  // Already closed, ignore
                }
              }
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
      
      // Initial fetch
      await fetchJob();
      
      // Poll every 500ms while job is active
      const interval = setInterval(async () => {
        if (!isActive) {
          clearInterval(interval);
          return;
        }
        await fetchJob();
      }, 500);
      
      // Cleanup on abort
      request.signal.addEventListener("abort", () => {
        isActive = false;
        clearInterval(interval);
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
