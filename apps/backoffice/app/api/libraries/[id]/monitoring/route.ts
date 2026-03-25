import { revalidatePath } from "next/cache";
import { NextRequest, NextResponse } from "next/server";
import { updateLibraryMonitoring } from "@/lib/api";

export async function PATCH(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;
  try {
    const { monitor_enabled, scan_mode, watcher_enabled, metadata_refresh_mode, download_detection_mode } = await request.json();
    const data = await updateLibraryMonitoring(id, monitor_enabled, scan_mode, watcher_enabled, metadata_refresh_mode, download_detection_mode);
    revalidatePath("/libraries");
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to update monitoring settings";
    console.error("[monitoring PATCH]", message);
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
