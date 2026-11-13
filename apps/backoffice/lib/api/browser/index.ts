/** Browser-side API surface: typed domain functions over one proxy transport. */

import { anilistApi } from "./anilist";
import { authApi } from "./auth";
import { booksApi } from "./books";
import { ddlApi } from "./ddl";
import { diagnosticsApi } from "./diagnostics";
import { discoveryApi } from "./discovery";
import { downloadsApi } from "./downloads";
import { genresApi } from "./genres";
import { jobsApi } from "./jobs";
import { komgaApi } from "./komga";
import { librariesApi } from "./libraries";
import { metadataApi } from "./metadata";
import { prowlarrApi } from "./prowlarr";
import { readingListsApi } from "./reading-lists";
import { seriesApi } from "./series";
import { settingsApi } from "./settings";
import { statsApi } from "./stats";
import { telegramApi } from "./telegram";

export {
  browserErrorMessage,
  browserFetch,
  browserFetchRaw,
  browserFetchText,
  browserFetchVoid,
  withQuery,
} from "./transport";

export type { TestConnectionResult, TestConnectionTarget } from "./settings";
export type { UpdateBookRequest } from "./books";
export type {
  GenreBrowsePage,
  GenreBrowseQuery,
  GenreCover,
  GenreDto,
  GenreFacet,
  GenreSuggestion,
} from "./genres";
export type {
  RenameBookEntry,
  RenameBookOverride,
  RenameBooksRequest,
  RenameBooksResponse,
  SeriesCreateRequest,
  SeriesListQuery,
  SeriesUpdateRequest,
} from "./series";
export type {
  MetadataApproveInput,
  MetadataApproveResult,
  MetadataLinkUpdate,
  MetadataMatchInput,
  MetadataSearchInput,
} from "./metadata";
export type { DdlSearchSettings } from "./ddl";
export type { TelegramChannelSuggestion, TelegramMonitorSettings } from "./telegram";
export type {
  DiscoveryAddInput,
  DiscoveryHideInput,
  DiscoverySuggestionDto,
  HiddenDiscoveryItem,
  ProwlarrDiscoveryItem,
  ProwlarrDiscoveryResponse,
} from "./discovery";
export type { ProwlarrSearchInput } from "./prowlarr";
export type { ReleaseBlacklistItem, TorrentAddInput } from "./downloads";
export type { LibraryMonitoringUpdate } from "./libraries";

export const browserApi = {
  anilist: anilistApi,
  auth: authApi,
  books: booksApi,
  ddl: ddlApi,
  diagnostics: diagnosticsApi,
  discovery: discoveryApi,
  downloads: downloadsApi,
  genres: genresApi,
  jobs: jobsApi,
  komga: komgaApi,
  libraries: librariesApi,
  metadata: metadataApi,
  prowlarr: prowlarrApi,
  readingLists: readingListsApi,
  series: seriesApi,
  settings: settingsApi,
  stats: statsApi,
  telegram: telegramApi,
};
