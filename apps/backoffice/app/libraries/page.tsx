import { revalidatePath } from "next/cache";
import Link from "next/link";
import { listFolders, createLibrary, deleteLibrary, fetchLibraries, getBookCoverUrl, LibraryDto, FolderItem } from "../../lib/api";
import type { TranslationKey } from "../../lib/i18n/fr";
import { getServerTranslations } from "../../lib/i18n/server";
import { LibraryActions } from "../components/LibraryActions";
import { LibraryForm } from "../components/LibraryForm";
import { ProviderIcon } from "../components/ProviderIcon";
import {
  Card, CardHeader, CardTitle, CardDescription, CardContent,
  Button, Badge
} from "../components/ui";

export const dynamic = "force-dynamic";

function formatNextScan(nextScanAt: string | null, imminentLabel: string): string {
  if (!nextScanAt) return "-";
  const date = new Date(nextScanAt);
  const now = new Date();
  const diff = date.getTime() - now.getTime();

  if (diff < 0) return imminentLabel;
  if (diff < 60000) return "< 1 min";
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m`;
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}h`;
  return `${Math.floor(diff / 86400000)}d`;
}

export default async function LibrariesPage() {
  const { t } = await getServerTranslations();
  const [libraries, folders] = await Promise.all([
    fetchLibraries().catch(() => [] as LibraryDto[]),
    listFolders().catch(() => [] as FolderItem[])
  ]);

  const thumbnailMap = new Map(
    libraries.map(lib => [
      lib.id,
      (lib.thumbnail_book_ids || []).map(bookId => getBookCoverUrl(bookId)),
    ])
  );

  async function addLibrary(formData: FormData) {
    "use server";
    const name = formData.get("name") as string;
    const rootPath = formData.get("root_path") as string;
    if (name && rootPath) {
      await createLibrary(name, rootPath);
      revalidatePath("/libraries");
    }
  }

  async function removeLibrary(formData: FormData) {
    "use server";
    const id = formData.get("id") as string;
    await deleteLibrary(id);
    revalidatePath("/libraries");
  }

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
          </svg>
          {t("libraries.title")}
        </h1>
      </div>

      {/* Add Library Form */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle>{t("libraries.addLibrary")}</CardTitle>
          <CardDescription>{t("libraries.addLibraryDescription")}</CardDescription>
        </CardHeader>
        <CardContent>
          <LibraryForm initialFolders={folders} action={addLibrary} />
        </CardContent>
      </Card>

      {/* Libraries Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {libraries.map((lib) => {
          const thumbnails = thumbnailMap.get(lib.id) || [];
          return (
            <Card key={lib.id} className="flex flex-col overflow-hidden">
              {/* Thumbnail fan */}
              {thumbnails.length > 0 ? (
                <Link href={`/libraries/${lib.id}/series`} className="block relative h-48 overflow-hidden bg-muted/10">
                  <img
                    src={thumbnails[0]}
                    alt=""
                    className="absolute inset-0 w-full h-full object-cover blur-xl scale-110 opacity-40"
                    loading="lazy"
                  />
                  <div className="absolute inset-0 flex items-end justify-center">
                    {thumbnails.map((url, i) => {
                      const count = thumbnails.length;
                      const mid = (count - 1) / 2;
                      const angle = (i - mid) * 12;
                      const radius = 220;
                      const rad = ((angle - 90) * Math.PI) / 180;
                      const cx = Math.cos(rad) * radius;
                      const cy = Math.sin(rad) * radius;
                      return (
                        <img
                          key={i}
                          src={url}
                          alt=""
                          className="absolute w-24 h-36 object-cover shadow-lg"
                          style={{
                            transform: `translate(${cx}px, ${cy}px) rotate(${angle}deg)`,
                            transformOrigin: 'bottom center',
                            zIndex: count - Math.abs(Math.round(i - mid)),
                            bottom: '-185px',
                          }}
                          loading="lazy"
                        />
                      );
                    })}
                  </div>
                </Link>
              ) : (
                <div className="h-8 bg-muted/10" />
              )}

              <CardHeader className="pb-2">
                <div className="flex items-start justify-between">
                  <div>
                    <CardTitle className="text-lg">{lib.name}</CardTitle>
                    {!lib.enabled && <Badge variant="muted" className="mt-1">{t("libraries.disabled")}</Badge>}
                  </div>
                  <div className="flex items-center gap-1">
                    <LibraryActions
                      libraryId={lib.id}
                      monitorEnabled={lib.monitor_enabled}
                      scanMode={lib.scan_mode}
                      watcherEnabled={lib.watcher_enabled}
                      metadataProvider={lib.metadata_provider}
                      fallbackMetadataProvider={lib.fallback_metadata_provider}
                      metadataRefreshMode={lib.metadata_refresh_mode}
                    />
                    <form>
                      <input type="hidden" name="id" value={lib.id} />
                      <Button type="submit" variant="ghost" size="sm" formAction={removeLibrary} className="text-muted-foreground hover:text-destructive">
                        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                        </svg>
                      </Button>
                    </form>
                  </div>
                </div>
                <code className="text-xs font-mono text-muted-foreground break-all">{lib.root_path}</code>
              </CardHeader>
              <CardContent className="flex-1 pt-0">
                {/* Stats */}
                <div className="grid grid-cols-2 gap-3 mb-3">
                  <Link
                    href={`/libraries/${lib.id}/books`}
                    className="text-center p-2.5 bg-muted/50 rounded-lg hover:bg-accent transition-colors duration-200"
                  >
                    <span className="block text-2xl font-bold text-primary">{lib.book_count}</span>
                    <span className="text-xs text-muted-foreground">{t("libraries.books")}</span>
                  </Link>
                  <Link
                    href={`/libraries/${lib.id}/series`}
                    className="text-center p-2.5 bg-muted/50 rounded-lg hover:bg-accent transition-colors duration-200"
                  >
                    <span className="block text-2xl font-bold text-foreground">{lib.series_count}</span>
                    <span className="text-xs text-muted-foreground">{t("libraries.series")}</span>
                  </Link>
                </div>

                {/* Configuration tags */}
                <div className="flex flex-wrap gap-1.5">
                  <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium ${
                    lib.monitor_enabled
                      ? 'bg-success/10 text-success'
                      : 'bg-muted/50 text-muted-foreground'
                  }`}>
                    <span className="text-[9px]">{lib.monitor_enabled ? '●' : '○'}</span>
                    {t("libraries.scanLabel", { mode: t(`monitoring.${lib.scan_mode}` as TranslationKey) })}
                  </span>

                  <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium ${
                    lib.watcher_enabled
                      ? 'bg-warning/10 text-warning'
                      : 'bg-muted/50 text-muted-foreground'
                  }`}>
                    <span>{lib.watcher_enabled ? '⚡' : '○'}</span>
                    <span>{t("libraries.watcherLabel")}</span>
                  </span>

                  {lib.metadata_provider && lib.metadata_provider !== "none" && (
                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-primary/10 text-primary">
                      <ProviderIcon provider={lib.metadata_provider} size={11} />
                      {lib.metadata_provider.replace('_', ' ')}
                    </span>
                  )}

                  {lib.metadata_refresh_mode !== "manual" && (
                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-muted/50 text-muted-foreground">
                      {t("libraries.metaRefreshLabel", { mode: t(`monitoring.${lib.metadata_refresh_mode}` as TranslationKey) })}
                    </span>
                  )}

                  {lib.monitor_enabled && lib.next_scan_at && (
                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-muted/50 text-muted-foreground">
                      {t("libraries.nextScan", { time: formatNextScan(lib.next_scan_at, t("libraries.imminent")) })}
                    </span>
                  )}
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
    </>
  );
}
