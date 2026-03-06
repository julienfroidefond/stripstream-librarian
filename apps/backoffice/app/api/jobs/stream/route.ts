import { NextRequest } from "next/server";

export async function GET(request: NextRequest) {
  const apiBaseUrl = process.env.API_BASE_URL || "http://api:8080";
  const apiToken = process.env.API_BOOTSTRAP_TOKEN;
  
  if (!apiToken) {
    return new Response(
      `data: ${JSON.stringify({ error: "API token not configured" })}\n\n`,
      { status: 500, headers: { "Content-Type": "text/event-stream" } }
    );
  }
  
  const stream = new ReadableStream({
    async start(controller) {
      controller.enqueue(new TextEncoder().encode(""));
      
      let lastData: string | null = null;
      let isActive = true;
      
      const fetchJobs = async () => {
        if (!isActive) return;
        
        try {
          const response = await fetch(`${apiBaseUrl}/index/status`, {
            headers: {
              Authorization: `Bearer ${apiToken}`,
            },
          });
          
          if (response.ok && isActive) {
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
            console.error("SSE fetch error:", error);
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
