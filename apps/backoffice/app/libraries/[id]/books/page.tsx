import { fetchLibraries, fetchBooks, getBookCoverUrl, LibraryDto, BookDto } from "../../../../lib/api";
import { BooksGrid, EmptyState } from "../../../components/BookCard";
import { Card, Badge, Button, CursorPagination } from "../../../components/ui";
import Link from "next/link";
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
    <>
      <div className="mb-6">
        <Link href="/libraries" className="text-sm text-muted hover:text-primary transition-colors">← Back to libraries</Link>
      </div>

      <h1 className="text-3xl font-bold text-foreground flex items-center gap-3 mb-6">
        <svg className="w-8 h-8 text-success" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" /></svg>
        {library.name}
      </h1>
      
      <Card className="mb-6">
        <div className="flex flex-wrap items-center gap-3 text-sm">
          <code className="text-xs font-mono text-muted bg-muted/10 px-2 py-1 rounded">{library.root_path}</code>
          <span className="text-muted">|</span>
          <span className="text-foreground">{library.book_count} book{library.book_count !== 1 ? 's' : ''}</span>
          <span className="text-muted">|</span>
          <Badge variant={library.enabled ? "success" : "muted"}>
            {library.enabled ? "Enabled" : "Disabled"}
          </Badge>
        </div>
      </Card>

      <div className="flex items-center gap-4 mb-6">
        <h2 className="text-xl font-semibold text-foreground">
          {series ? `Books in "${seriesDisplayName}"` : "All Books"}
        </h2>
        {series && (
          <Link href={`/libraries/${id}/books`} className="text-sm text-primary hover:text-primary/80">
            View all
          </Link>
        )}
      </div>

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
    </>
  );
}
