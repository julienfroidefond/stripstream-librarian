import { fetchLibraries, fetchSeries, getBookCoverUrl, LibraryDto, SeriesDto } from "../../../../lib/api";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
import { LibrarySubPageHeader } from "../../../components/LibrarySubPageHeader";

export const dynamic = "force-dynamic";

export default async function LibrarySeriesPage({
  params
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  
  const [library, series] = await Promise.all([
    fetchLibraries().then(libs => libs.find(l => l.id === id)),
    fetchSeries(id).catch(() => [] as SeriesDto[])
  ]);

  if (!library) {
    notFound();
  }

  return (
    <div className="space-y-6">
      <LibrarySubPageHeader
        library={library}
        title={`Series (${series.length})`}
        icon={
          <svg className="w-8 h-8" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
        }
        iconColor="text-primary"
      />

      {series.length > 0 ? (
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-6">
          {series.map((s) => (
            <Link 
              key={s.name} 
              href={`/libraries/${id}/books?series=${encodeURIComponent(s.name)}`}
              className="group"
            >
              <div className="bg-card rounded-xl shadow-sm border border-border/60 overflow-hidden hover:shadow-md transition-shadow duration-200">
                <div className="aspect-[2/3] relative bg-muted/50">
                  <Image
                    src={getBookCoverUrl(s.first_book_id)}
                    alt={`Cover of ${s.name}`}
                    fill
                    className="object-cover"
                    unoptimized
                  />
                </div>
                <div className="p-3">
                  <h3 className="font-medium text-foreground truncate text-sm" title={s.name}>
                    {s.name === "unclassified" ? "Unclassified" : s.name}
                  </h3>
                  <p className="text-xs text-muted-foreground mt-1">
                    {s.book_count} book{s.book_count !== 1 ? 's' : ''}
                  </p>
                </div>
              </div>
            </Link>
          ))}
        </div>
      ) : (
        <div className="text-center py-12 text-muted-foreground">
          <p>No series found in this library</p>
        </div>
      )}
    </div>
  );
}
