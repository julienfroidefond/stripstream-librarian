import { fetchLibraries, fetchBooks, getBookCoverUrl, LibraryDto, BookDto } from "../../../../lib/api";
import { BooksGrid, EmptyState } from "../../../components/BookCard";
import { LibrarySubPageHeader } from "../../../components/LibrarySubPageHeader";
import { CursorPagination } from "../../../components/ui";
import { notFound } from "next/navigation";

export const dynamic = "force-dynamic";

export default async function LibraryBooksPage({
  params,
  searchParams
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const { id } = await params;
  const searchParamsAwaited = await searchParams;
  const cursor = typeof searchParamsAwaited.cursor === "string" ? searchParamsAwaited.cursor : undefined;
  const series = typeof searchParamsAwaited.series === "string" ? searchParamsAwaited.series : undefined;
  const limit = typeof searchParamsAwaited.limit === "string" ? parseInt(searchParamsAwaited.limit) : 20;
  
  const [library, booksPage] = await Promise.all([
    fetchLibraries().then(libs => libs.find(l => l.id === id)),
    fetchBooks(id, series, cursor, limit).catch(() => ({ 
      items: [] as BookDto[], 
      next_cursor: null 
    }))
  ]);

  if (!library) {
    notFound();
  }

  const books = booksPage.items.map(book => ({
    ...book,
    coverUrl: getBookCoverUrl(book.id)
  }));
  const nextCursor = booksPage.next_cursor;
  
  const seriesDisplayName = series === "unclassified" ? "Unclassified" : series;
  const hasNextPage = !!nextCursor;
  const hasPrevPage = !!cursor;

  return (
    <div className="space-y-6">
      <LibrarySubPageHeader
        library={library}
        title={series ? `Books in "${seriesDisplayName}"` : "All Books"}
        icon={
          <svg className="w-8 h-8" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
          </svg>
        }
        iconColor="text-success"
        filterInfo={series ? {
          label: `Showing books from series "${seriesDisplayName}"`,
          clearHref: `/libraries/${id}/books`,
          clearLabel: "View all books"
        } : undefined}
      />

      {books.length > 0 ? (
        <>
          <BooksGrid books={books} />
          
          <CursorPagination
            hasNextPage={hasNextPage}
            hasPrevPage={hasPrevPage}
            pageSize={limit}
            currentCount={books.length}
            nextCursor={nextCursor}
          />
        </>
      ) : (
        <EmptyState message={series ? `No books in series "${seriesDisplayName}"` : "No books in this library yet"} />
      )}
    </div>
  );
}
