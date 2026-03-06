import { revalidatePath } from "next/cache";
import Link from "next/link";
import { listFolders, createLibrary, deleteLibrary, fetchLibraries, fetchSeries, scanLibrary, LibraryDto, FolderItem } from "../../lib/api";
import { LibraryActions } from "../components/LibraryActions";
import { Card, CardHeader, Button, Badge, FormField, FormInput, FormSelect, FormRow } from "../components/ui";

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
        const series = await fetchSeries(lib.id);
        return { id: lib.id, count: series.length };
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
      <h1 className="text-3xl font-bold text-foreground mb-6 flex items-center gap-3">
        <svg className="w-8 h-8 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" /></svg>
        Libraries
      </h1>
      
      {/* Add Library Form */}
      <Card className="mb-6">
        <CardHeader title="Add New Library" />
        <form action={addLibrary}>
          <FormRow>
            <FormField>
              <FormInput name="name" placeholder="Library name" required />
            </FormField>
            <FormField>
              <FormSelect name="root_path" required defaultValue="">
                <option value="" disabled>Select folder...</option>
                {folders.map((folder) => (
                  <option key={folder.path} value={folder.path}>
                    {folder.name}
                  </option>
                ))}
              </FormSelect>
            </FormField>
            <Button type="submit">➕ Add Library</Button>
          </FormRow>
        </form>
      </Card>

      {/* Libraries Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {libraries.map((lib) => {
          const seriesCount = seriesCountMap.get(lib.id) || 0;
          return (
            <Card key={lib.id} className="flex flex-col">
              {/* Header with settings */}
              <div className="flex items-start justify-between mb-3">
                <div>
                  <h3 className="text-lg font-semibold text-foreground">{lib.name}</h3>
                  {!lib.enabled && <Badge variant="muted" className="mt-1">Disabled</Badge>}
                </div>
                <LibraryActions
                  libraryId={lib.id}
                  monitorEnabled={lib.monitor_enabled}
                  scanMode={lib.scan_mode}
                  watcherEnabled={lib.watcher_enabled}
                />
              </div>

              {/* Path */}
              <code className="text-xs font-mono text-muted mb-4 break-all">{lib.root_path}</code>

              {/* Stats */}
              <div className="grid grid-cols-2 gap-3 mb-4">
                <Link href={`/libraries/${lib.id}/books`} className="text-center p-3 bg-muted/5 rounded-lg hover:bg-muted/10 transition-colors">
                  <span className="block text-2xl font-bold text-primary">{lib.book_count}</span>
                  <span className="text-xs text-muted">Books</span>
                </Link>
                <Link href={`/libraries/${lib.id}/series`} className="text-center p-3 bg-muted/5 rounded-lg hover:bg-muted/10 transition-colors">
                  <span className="block text-2xl font-bold text-foreground">{seriesCount}</span>
                  <span className="text-xs text-muted">Series</span>
                </Link>
              </div>

              {/* Status */}
              <div className="flex items-center gap-3 mb-4 text-sm">
                <span className={`flex items-center gap-1 ${lib.monitor_enabled ? 'text-success' : 'text-muted'}`}>
                  {lib.monitor_enabled ? '●' : '○'} {lib.monitor_enabled ? 'Auto' : 'Manual'}
                </span>
                {lib.watcher_enabled && (
                  <span className="text-warning" title="File watcher active">⚡</span>
                )}
                {lib.monitor_enabled && lib.next_scan_at && (
                  <span className="text-xs text-muted ml-auto">
                    Next: {formatNextScan(lib.next_scan_at)}
                  </span>
                )}
              </div>

              {/* Actions */}
              <div className="flex items-center gap-2 mt-auto">
                <form className="flex-1">
                  <input type="hidden" name="id" value={lib.id} />
                  <Button type="submit" variant="primary" size="sm" className="w-full" formAction={scanLibraryAction}>
                    🔄 Index
                  </Button>
                </form>
                <form className="flex-1">
                  <input type="hidden" name="id" value={lib.id} />
                  <Button type="submit" variant="secondary" size="sm" className="w-full" formAction={scanLibraryFullAction}>
                    🔁 Full
                  </Button>
                </form>
                <form>
                  <input type="hidden" name="id" value={lib.id} />
                  <Button type="submit" variant="danger" size="sm" formAction={removeLibrary}>
                    🗑
                  </Button>
                </form>
              </div>
            </Card>
          );
        })}
      </div>
    </>
  );
}
