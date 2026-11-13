/** Browser-side reading lists. */

import type {
  AiReadingListSuggestions,
  ReadingListDetailDto,
  ReadingListDto,
} from "../reading-lists";
import { browserFetch, browserFetchVoid, withQuery } from "./transport";

export const readingListsApi = {
  list(seriesId?: string): Promise<ReadingListDto[]> {
    return browserFetch<ReadingListDto[]>(
      withQuery("/api/reading-lists", { series_id: seriesId })
    );
  },

  detail(listId: string): Promise<ReadingListDetailDto> {
    return browserFetch<ReadingListDetailDto>(
      `/api/reading-lists/${encodeURIComponent(listId)}`
    );
  },

  create(name: string, description?: string | null): Promise<ReadingListDto> {
    return browserFetch<ReadingListDto>("/api/reading-lists", {
      method: "POST",
      body: { name, description },
    });
  },

  update(
    listId: string,
    data: { name?: string; description?: string | null }
  ): Promise<ReadingListDto> {
    return browserFetch<ReadingListDto>(
      `/api/reading-lists/${encodeURIComponent(listId)}`,
      { method: "PATCH", body: data }
    );
  },

  remove(listId: string): Promise<void> {
    return browserFetchVoid(`/api/reading-lists/${encodeURIComponent(listId)}`, {
      method: "DELETE",
    });
  },

  /** The API answers `204 No Content`: there is no created item to return. */
  addSeries(listId: string, seriesId: string): Promise<void> {
    return browserFetchVoid(
      `/api/reading-lists/${encodeURIComponent(listId)}/series`,
      { method: "POST", body: { series_id: seriesId } }
    );
  },

  removeSeries(listId: string, seriesId: string): Promise<void> {
    return browserFetchVoid(
      `/api/reading-lists/${encodeURIComponent(listId)}/series/${encodeURIComponent(seriesId)}`,
      { method: "DELETE" }
    );
  },

  moveSeries(fromListId: string, seriesId: string, toListId: string): Promise<void> {
    return browserFetchVoid(
      `/api/reading-lists/${encodeURIComponent(fromListId)}/series/${encodeURIComponent(seriesId)}/move`,
      { method: "POST", body: { to_list_id: toListId } }
    );
  },

  reorder(listId: string, seriesIds: string[]): Promise<void> {
    return browserFetchVoid(
      `/api/reading-lists/${encodeURIComponent(listId)}/series/reorder`,
      { method: "PUT", body: { series_ids: seriesIds } }
    );
  },

  aiSuggest(limit?: number): Promise<AiReadingListSuggestions> {
    return browserFetch<AiReadingListSuggestions>("/api/reading-lists/ai-suggest", {
      method: "POST",
      body: limit == null ? {} : { limit },
    });
  },
};
