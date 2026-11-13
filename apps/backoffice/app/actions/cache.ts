"use server";

import { invalidateAfter } from "@/lib/cache";

export async function refreshBooksAction() {
  invalidateAfter({ type: "catalog.refreshRequested", scope: "books" });
}

export async function refreshSeriesAction() {
  invalidateAfter({ type: "catalog.refreshRequested", scope: "series" });
}

export async function refreshSeriesByIdAction(seriesId: string) {
  invalidateAfter({ type: "catalog.refreshRequested", scope: "series-detail", seriesId });
}

export async function refreshAfterJobAction() {
  invalidateAfter({ type: "jobs.finished" });
}
