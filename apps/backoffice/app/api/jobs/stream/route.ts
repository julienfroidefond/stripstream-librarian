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

            // Send if data changed
            if (dataStr !== lastData && isActive) {
              lastData = dataStr;
              try {
                controller.enqueue(
                  new TextEncoder().encode(`data: ${dataStr}\n\n`)
                );
              } catch (err) {
                // Controller closed, ignore
                isActive = false;
              }
            }
          }
        } catch (error) {
          if (isActive) {
            consecutiveErrors++;
            // Only log first failure and every 30th to avoid spam
            if (consecutiveErrors === 1 || consecutiveErrors % 30 === 0) {
              console.warn(`SSE fetch error (${consecutiveErrors} consecutive):`, error);
            }
          }
        }
      };
      
      // Initial fetch
      await fetchJobs();
      
      // Poll every 2 seconds
      const interval = setInterval(async () => {
        if (!isActive) {
          clearInterval(interval);
          return;
        }
        await fetchJobs();
      }, 2000);
      
      // Cleanup
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
