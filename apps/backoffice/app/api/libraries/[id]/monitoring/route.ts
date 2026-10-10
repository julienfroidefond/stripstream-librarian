import { revalidatePath } from "next/cache";
import { NextRequest, NextResponse } from "next/server";
import { updateLibraryMonitoring } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const PATCH = withRoute(
  async (request: NextRequest, { params }: { params: Promise<{ id: string }> }) => {
    const { id } = await params;
    const { monitor_enabled, scan_mode, watcher_enabled, metadata_refresh_mode, download_detection_mode } = await request.json();
    const data = await updateLibraryMonitoring(id, monitor_enabled, scan_mode, watcher_enabled, metadata_refresh_mode, download_detection_mode);
    revalidatePath("/libraries");
    return NextResponse.json(data);
  },
  {
    fallback: "Failed to update monitoring settings",
    onError: (_error, message) => console.error("[monitoring PATCH]", message),
  }
);
