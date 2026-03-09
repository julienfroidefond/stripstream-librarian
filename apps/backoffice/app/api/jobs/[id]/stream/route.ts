import { NextRequest } from "next/server";

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;
  const apiBaseUrl = process.env.API_BASE_URL || "http://api:7080";
  const apiToken = process.env.API_BOOTSTRAP_TOKEN;
  
  if (!apiToken) {
    return new Response(
      `data: ${JSON.stringify({ error: "API token not configured" })}\n\n`,
      { status: 500, headers: { "Content-Type": "text/event-stream" } }
    );
  }
  
  const stream = new ReadableStream({
    async start(controller) {
      // Send initial headers for SSE
      controller.enqueue(new TextEncoder().encode(""));
      
      let lastData: string | null = null;
      let isActive = true;
      
      const fetchJob = async () => {
        if (!isActive) return;
        
        try {
          const response = await fetch(`${apiBaseUrl}/index/jobs/${id}`, {
            headers: {
              Authorization: `Bearer ${apiToken}`,
            },
          });
          
          if (response.ok && isActive) {
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
            console.error("SSE fetch error:", error);
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
