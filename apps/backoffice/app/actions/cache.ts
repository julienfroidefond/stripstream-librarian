"use server";

import { updateTag } from "next/cache";

export async function refreshBooksAction() {
  updateTag("books");
}

export async function refreshSeriesAction() {
  updateTag("series");
}

export async function refreshSeriesByIdAction(seriesId: string) {
  updateTag(`series:${seriesId}`);
}

export async function refreshAfterJobAction() {
  updateTag("books");
  updateTag("series");
  updateTag("stats");
}
