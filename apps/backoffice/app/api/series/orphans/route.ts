import { NextResponse, NextRequest } from "next/server";
import { revalidatePath } from "next/cache";
import { apiFetch } from "@/lib/api";
import type { OrphanSeriesPageDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

const PAGE_SIZE = 200;

/** Collect every orphan series id, walking the paginated admin endpoint. */
async function listAllOrphanIds(): Promise<string[]> {
  const ids: string[] = [];
  let page = 1;

  for (;;) {
    const data = await apiFetch<OrphanSeriesPageDto>(
      `/admin/series/orphans?page=${page}&limit=${PAGE_SIZE}`,
    );
    ids.push(...data.items.map((item) => item.id));
    if (data.items.length < PAGE_SIZE || ids.length >= data.total) break;
    page += 1;
  }

  return ids;
}

export const GET = withRoute(async () => {
  const data = await apiFetch<OrphanSeriesPageDto>("/admin/series/orphans");
  return NextResponse.json(data);
}, { fallback: "Failed to load orphan series" });

export const DELETE = withRoute(async (request: NextRequest) => {
  let ids: string[] | undefined;
  try {
    const body = (await request.json()) as { ids?: unknown };
    if (Array.isArray(body?.ids) && body.ids.length > 0) {
      ids = body.ids.filter((id): id is string => typeof id === "string");
    }
  } catch {
    // No body: bulk-delete every orphan.
  }

  const targets = ids ?? (await listAllOrphanIds());

  for (const id of targets) {
    await apiFetch(`/series/${id}`, { method: "DELETE" });
  }

  revalidatePath("/series");
  return NextResponse.json({ deleted: targets.length });
}, { fallback: "Failed to delete orphan series" });
