import type { MetadataFieldCatalogDto } from "@/lib/api";

/**
 * Shape returned by `GET /metadata/fields`, used by component tests that render
 * a lock form inside `MetadataFieldsProvider`.
 */
export const METADATA_FIELDS_CATALOG: MetadataFieldCatalogDto = {
  series: [
    { key: "description", lockable: true, clearable: true },
    { key: "authors", lockable: true, clearable: true },
    { key: "publishers", lockable: true, clearable: true },
    { key: "start_year", lockable: true, clearable: true },
    { key: "total_volumes", lockable: true, clearable: true },
    { key: "status", lockable: true, clearable: true },
    { key: "genres", lockable: false, clearable: false },
    { key: "cover_url", lockable: true, clearable: true },
  ],
  books: [
    { key: "summary", lockable: true, clearable: false },
    { key: "isbn", lockable: true, clearable: false },
    { key: "publish_date", lockable: true, clearable: false },
    { key: "language", lockable: true, clearable: false },
    { key: "authors", lockable: true, clearable: false },
  ],
};
