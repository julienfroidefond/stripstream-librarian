import { revalidatePath } from "next/cache";
import Link from "next/link";
import { listFolders, createLibrary, deleteLibrary, fetchLibraries, fetchSeries, scanLibrary, LibraryDto, FolderItem } from "../../lib/api";
import { LibraryActions } from "../components/LibraryActions";
import { LibraryForm } from "../components/LibraryForm";
import { 
  Card, CardHeader, CardTitle, CardDescription, CardContent,
  Button, Badge
} from "../components/ui";

export const dynamic = "force-dynamic";

function formatNextScan(nextScanAt: string | null): string {
  if (!nextScanAt) return "-";
  const date = new Date(nextScanAt);
  const now = new Date();
  const diff = date.getTime() - now.getTime();
  
  if (diff < 0) return "Due now";
  if (diff < 60000) return "< 1 min";
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m`;
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}h`;
  return `${Math.floor(diff / 86400000)}d`;
}

export default async function LibrariesPage() {
  const [libraries, folders] = await Promise.all([
    fetchLibraries().catch(() => [] as LibraryDto[]),
    listFolders().catch(() => [] as FolderItem[])
  ]);

  const seriesCounts = await Promise.all(
    libraries.map(async (lib) => {
      try {
        const seriesPage = await fetchSeries(lib.id);
        return { id: lib.id, count: seriesPage.items.length };
      } catch {
        return { id: lib.id, count: 0 };
      }
    })
  );
  
  const seriesCountMap = new Map(seriesCounts.map(s => [s.id, s.count]));

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

  async function scanLibraryAction(formData: FormData) {
    "use server";
    const id = formData.get("id") as string;
    await scanLibrary(id);
    revalidatePath("/libraries");
    revalidatePath("/jobs");
  }

  async function scanLibraryFullAction(formData: FormData) {
    "use server";
    const id = formData.get("id") as string;
    await scanLibrary(id, true);
    revalidatePath("/libraries");
    revalidatePath("/jobs");
  }

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
          </svg>
          Libraries
        </h1>
      </div>
      
      {/* Add Library Form */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle>Add New Library</CardTitle>
          <CardDescription>Create a new library from an existing folder</CardDescription>
        </CardHeader>
        <CardContent>
          <LibraryForm initialFolders={folders} action={addLibrary} />
        </CardContent>
      </Card>

      {/* Libraries Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {libraries.map((lib) => {
          const seriesCount = seriesCountMap.get(lib.id) || 0;
          return (
            <Card key={lib.id} className="flex flex-col">
              <CardHeader className="pb-2">
                <div className="flex items-start justify-between">
                  <div>
                    <CardTitle className="text-lg">{lib.name}</CardTitle>
                    {!lib.enabled && <Badge variant="muted" className="mt-1">Disabled</Badge>}
                  </div>
                  <LibraryActions
                    libraryId={lib.id}
                    monitorEnabled={lib.monitor_enabled}
                    scanMode={lib.scan_mode}
                    watcherEnabled={lib.watcher_enabled}
                    metadataProvider={lib.metadata_provider}
                  />
                </div>
              </CardHeader>
              <CardContent className="flex-1 pt-0">
                {/* Path */}
                <code className="text-xs font-mono text-muted-foreground mb-4 break-all block">{lib.root_path}</code>

                {/* Stats */}
                <div className="grid grid-cols-2 gap-3 mb-4">
                  <Link 
                    href={`/libraries/${lib.id}/books`} 
                    className="text-center p-3 bg-muted/50 rounded-lg hover:bg-accent transition-colors duration-200"
                  >
                    <span className="block text-2xl font-bold text-primary">{lib.book_count}</span>
                    <span className="text-xs text-muted-foreground">Books</span>
                  </Link>
                  <Link 
                    href={`/libraries/${lib.id}/series`} 
                    className="text-center p-3 bg-muted/50 rounded-lg hover:bg-accent transition-colors duration-200"
                  >
                    <span className="block text-2xl font-bold text-foreground">{seriesCount}</span>
                    <span className="text-xs text-muted-foreground">Series</span>
                  </Link>
                </div>

                {/* Status */}
                <div className="flex items-center gap-3 mb-4 text-sm">
                  <span className={`flex items-center gap-1 ${lib.monitor_enabled ? 'text-success' : 'text-muted-foreground'}`}>
                    {lib.monitor_enabled ? '●' : '○'} {lib.monitor_enabled ? 'Auto' : 'Manual'}
                  </span>
                  {lib.watcher_enabled && (
                    <span className="text-warning" title="File watcher active">⚡</span>
                  )}
                  {lib.monitor_enabled && lib.next_scan_at && (
                    <span className="text-xs text-muted-foreground ml-auto">
                      Next: {formatNextScan(lib.next_scan_at)}
                    </span>
                  )}
                </div>

                {/* Actions */}
                <div className="flex items-center gap-2">
                  <form className="flex-1">
                    <input type="hidden" name="id" value={lib.id} />
                    <Button type="submit" variant="default" size="sm" className="w-full" formAction={scanLibraryAction}>
                      <svg className="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                      </svg>
                      Index
                    </Button>
                  </form>
                  <form className="flex-1">
                    <input type="hidden" name="id" value={lib.id} />
                    <Button type="submit" variant="secondary" size="sm" className="w-full" formAction={scanLibraryFullAction}>
                      <svg className="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                      </svg>
                      Full
                    </Button>
                  </form>
                  <form>
                    <input type="hidden" name="id" value={lib.id} />
                    <Button type="submit" variant="destructive" size="sm" formAction={removeLibrary}>
                      <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                      </svg>
                    </Button>
                  </form>
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
    </>
  );
}
