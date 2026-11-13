/**
 * Series edit form: the initial field values derived from the series detail
 * view model. Pure (no React) so the mapping is tested on its own.
 */

import type { SeriesActionsModel } from "./detail";

/** What `EditSeriesForm` reads from the series detail model. */
export type EditSeriesModel = Pick<
  SeriesActionsModel,
  "seriesId" | "seriesName" | "metadata" | "editBookAuthor" | "editBookLanguage"
>;

/** Form-ready values (strings for text/number inputs, `""` for "unset"). */
export type SeriesEditDefaults = {
  name: string;
  authors: string[];
  genres: string[];
  publishers: string[];
  description: string;
  startYear: string;
  totalVolumes: string;
  status: string;
  lockedFields: Record<string, boolean>;
  bookAuthor: string;
  bookLanguage: string;
};

export function seriesEditDefaults(model: EditSeriesModel): SeriesEditDefaults {
  const metadata = model.metadata;
  return {
    name: model.seriesName === "unclassified" ? "" : model.seriesName,
    authors: metadata?.authors ?? [],
    genres: metadata?.genres ?? [],
    publishers: metadata?.publishers ?? [],
    description: metadata?.description ?? "",
    startYear: metadata?.start_year?.toString() ?? "",
    totalVolumes: metadata?.total_volumes?.toString() ?? "",
    status: metadata?.status ?? "",
    lockedFields: metadata?.locked_fields ?? {},
    bookAuthor: model.editBookAuthor ?? "",
    bookLanguage: model.editBookLanguage ?? "",
  };
}
