import { NextResponse, NextRequest } from "next/server";
import { revalidatePath } from "next/cache";
import { apiFetch } from "@/lib/api";
import { ApiError } from "@/lib/api/errors";
import type { OrphanDeleteResultDto, OrphanSeriesPageDto } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

const PAGE_SIZE = 200;
/** In-flight delete requests. Keeps a large purge from exhausting the DB pool. */
const DELETE_CONCURRENCY = 5;

type DeleteSelection = { ids: string[] } | { all: true };

const INVALID_BODY =
  'Expected a JSON body with a non-empty "ids" array of UUIDs or {"all": true}';

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * The id is interpolated into the upstream URL, so a non-UUID value such as
 * `x/../../../../books/<uuid>` would resolve to a different admin endpoint.
 * `encodeURIComponent` is not enough (dots are not encoded): validate the
 * shape before building any path.
 */
function isUuid(value: string): boolean {
  return UUID_RE.test(value);
}

/** Collect every deletable orphan id, walking the paginated admin endpoint. */
async function listAllOrphanIds(): Promise<string[]> {
  const ids: string[] = [];
  let page = 1;

  for (;;) {
    const data = await apiFetch<OrphanSeriesPageDto>(
      `/admin/series/orphans?page=${page}&limit=${PAGE_SIZE}`,
    );
    // Trust the response's effective limit, not the requested one: the API may
    // clamp it lower, and stopping early would report success without purging.
    const limit = data.limit > 0 ? data.limit : PAGE_SIZE;

    // Never race an in-progress import: those series are left for the operator.
    // Re-validate ids from the upstream payload: they end up in a URL too.
    ids.push(
      ...data.items
        .filter((item) => !item.has_active_download && isUuid(item.id))
        .map((item) => item.id),
    );

    const fetched = (page - 1) * limit + data.items.length;
    if (data.items.length === 0 || fetched >= data.total) break;
    page += 1;
  }

  return ids;
}

/**
 * Fail closed: only an explicit non-empty `ids` array or `{"all": true}` is
 * accepted. A missing body, malformed JSON, `{}` or `{"ids": []}` must never
 * be interpreted as "delete everything". `all` and `ids` are mutually
 * exclusive so neither can silently win; ids are deduplicated.
 */
async function parseSelection(request: NextRequest): Promise<DeleteSelection | null> {
  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return null;
  }

  if (body === null || typeof body !== "object" || Array.isArray(body)) return null;

  const { ids, all } = body as { ids?: unknown; all?: unknown };

  if (all !== undefined) {
    if (all !== true || ids !== undefined) return null;
    return { all: true };
  }

  if (!Array.isArray(ids) || ids.length === 0) return null;
  if (!ids.every((id): id is string => typeof id === "string" && isUuid(id))) {
    return null;
  }

  return { ids: [...new Set(ids)] };
}

function isNotFound(error: unknown): boolean {
  return error instanceof ApiError && error.status === 404;
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

/**
 * Settle `fn` for every item with at most `limit` promises in flight. Results
 * are stored by index so the summary order matches the input order.
 */
async function mapWithConcurrency<T, R>(
  items: T[],
  limit: number,
  fn: (item: T) => Promise<R>,
): Promise<PromiseSettledResult<R>[]> {
  const results: PromiseSettledResult<R>[] = new Array(items.length);
  let next = 0;

  async function worker() {
    for (;;) {
      const index = next;
      next += 1;
      if (index >= items.length) return;
      try {
        results[index] = { status: "fulfilled", value: await fn(items[index]!) };
      } catch (error) {
        results[index] = { status: "rejected", reason: error };
      }
    }
  }

  const workers = Array.from(
    { length: Math.min(Math.max(limit, 1), items.length) },
    () => worker(),
  );
  await Promise.all(workers);

  return results;
}

/** Delete every id (bounded concurrency), so one failure cannot abort the rest. */
async function deleteMany(ids: string[]): Promise<OrphanDeleteResultDto> {
  const results = await mapWithConcurrency(ids, DELETE_CONCURRENCY, deleteOne);
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
  const search = request.nextUrl.search ?? "";
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
