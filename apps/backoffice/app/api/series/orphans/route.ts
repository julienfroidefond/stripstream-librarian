import { NextResponse, NextRequest } from "next/server";
import { revalidatePath } from "next/cache";
import { apiFetch } from "@/lib/api";
import type { OrphanDeleteResultDto, OrphanSeriesPageDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

const PAGE_SIZE = 200;

type DeleteSelection = { ids: string[] } | { all: true };

const INVALID_BODY =
  'Expected a JSON body with a non-empty "ids" array or {"all": true}';

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

/**
 * Fail closed: only an explicit non-empty `ids` array or `{"all": true}` is
 * accepted. A missing body, malformed JSON, `{}` or `{"ids": []}` must never
 * be interpreted as "delete everything".
 */
async function parseSelection(request?: NextRequest): Promise<DeleteSelection | null> {
  if (!request) return null;

  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return null;
  }

  if (body === null || typeof body !== "object" || Array.isArray(body)) return null;

  const { ids, all } = body as { ids?: unknown; all?: unknown };
  if (all === true) return { all: true };

  if (!Array.isArray(ids) || ids.length === 0) return null;
  if (!ids.every((id): id is string => typeof id === "string" && id.trim().length > 0)) {
    return null;
  }

  return { ids: ids as string[] };
}

function isNotFound(error: unknown): boolean {
  return error instanceof Error && /\(404\)/.test(error.message);
}

/** Delete one orphan, treating a `404` as already deleted. */
async function deleteOne(id: string): Promise<"deleted" | "skipped"> {
  try {
    await apiFetch(`/admin/series/orphans/${id}`, { method: "DELETE" });
    return "deleted";
  } catch (error) {
    if (isNotFound(error)) return "skipped";
    throw error;
  }
}

/** Delete every id, settling all requests so one failure cannot abort the rest. */
async function deleteMany(ids: string[]): Promise<OrphanDeleteResultDto> {
  const results = await Promise.allSettled(ids.map((id) => deleteOne(id)));
  const summary: OrphanDeleteResultDto = { deleted: 0, skipped: 0, failed: [] };

  results.forEach((result, index) => {
    if (result.status === "fulfilled") {
      if (result.value === "skipped") summary.skipped += 1;
      else summary.deleted += 1;
    } else {
      summary.failed.push(ids[index]!);
    }
  });

  return summary;
}

export const GET = withRoute(async (request: NextRequest) => {
  const search = request?.nextUrl.search ?? "";
  const data = await apiFetch<OrphanSeriesPageDto>(`/admin/series/orphans${search}`);
  return NextResponse.json(data);
}, { fallback: "Failed to load orphan series" });

export const DELETE = withRoute(async (request: NextRequest) => {
  const selection = await parseSelection(request);
  if (!selection) {
    return NextResponse.json({ error: INVALID_BODY }, { status: 400 });
  }

  const ids = "all" in selection ? await listAllOrphanIds() : selection.ids;
  const summary = await deleteMany(ids);

  revalidatePath("/settings");
  revalidatePath("/series");

  return NextResponse.json(summary);
}, { fallback: "Failed to delete orphan series" });
