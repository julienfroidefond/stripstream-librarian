import { fetchBooks, searchBooks, fetchLibraries, BookDto, LibraryDto, getBookCoverUrl } from "../../lib/api";
import { BooksGrid, EmptyState } from "../components/BookCard";
import { Card, Button, FormField, FormInput, FormSelect, FormRow, CursorPagination } from "../components/ui";
import Link from "next/link";

export const dynamic = "force-dynamic";

export default async function BooksPage({
  searchParams
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const searchParamsAwaited = await searchParams;
  const libraryId = typeof searchParamsAwaited.library === "string" ? searchParamsAwaited.library : undefined;
  const searchQuery = typeof searchParamsAwaited.q === "string" ? searchParamsAwaited.q : "";
  const cursor = typeof searchParamsAwaited.cursor === "string" ? searchParamsAwaited.cursor : undefined;
  const limit = typeof searchParamsAwaited.limit === "string" ? parseInt(searchParamsAwaited.limit) : 20;
  
  const [libraries] = await Promise.all([
    fetchLibraries().catch(() => [] as LibraryDto[])
  ]);

  let books: BookDto[] = [];
  let nextCursor: string | null = null;
  let searchResults: BookDto[] | null = null;
  let totalHits: number | null = null;

  if (searchQuery) {
    // Mode recherche
    const searchResponse = await searchBooks(searchQuery, libraryId, limit).catch(() => null);
    if (searchResponse) {
      searchResults = searchResponse.hits.map(hit => ({
        id: hit.id,
        library_id: hit.library_id,
        kind: hit.kind,
        title: hit.title,
        author: hit.author,
        series: hit.series,
        volume: hit.volume,
        language: hit.language,
        page_count: null,
        file_path: null,
        file_format: null,
        file_parse_status: null,
        updated_at: ""
      }));
      totalHits = searchResponse.estimated_total_hits;
    }
  } else {
    // Mode liste avec pagination
    const booksPage = await fetchBooks(libraryId, undefined, cursor, limit).catch(() => ({ 
      items: [] as BookDto[], 
      next_cursor: null,
      prev_cursor: null 
    }));
    books = booksPage.items;
    nextCursor = booksPage.next_cursor;
    // Note: L'API ne supporte pas encore prev_cursor, on gère ça côté UI
  }

  const displayBooks = (searchResults || books).map(book => ({
    ...book,
    coverUrl: getBookCoverUrl(book.id)
  }));

  const hasNextPage = !!nextCursor;
  const hasPrevPage = !!cursor; // Si on a un cursor, on peut revenir en arrière (simplifié)

  return (
    <>
      <h1 className="text-3xl font-bold text-foreground mb-6 flex items-center gap-3">
        <svg className="w-8 h-8 text-success" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" /></svg>
        Books
      </h1>
      
      {/* Filtres et recherche */}
      <Card className="mb-6">
        <form>
          <FormRow>
            <FormField>
              <FormInput 
                name="q" 
                placeholder="Search books..." 
                defaultValue={searchQuery}
              />
            </FormField>
            <FormField>
              <FormSelect name="library" defaultValue={libraryId || ""}>
                <option value="">All libraries</option>
                {libraries.map((lib) => (
                  <option key={lib.id} value={lib.id}>
                    {lib.name}
                  </option>
                ))}
              </FormSelect>
            </FormField>
            <Button type="submit">🔍 Search</Button>
            {searchQuery && (
              <Link 
                href="/books" 
                className="px-4 py-2.5 border border-line text-muted font-medium rounded-lg hover:bg-muted/5 transition-colors"
              >
                ✕ Clear
              </Link>
            )}
          </FormRow>
        </form>
      </Card>

      {/* Résultats de recherche */}
      {searchQuery && totalHits !== null && (
        <p className="text-sm text-muted mb-4">
          Found {totalHits} result{totalHits !== 1 ? 's' : ''} for &quot;{searchQuery}&quot;
        </p>
      )}

      {/* Grille de livres */}
      {displayBooks.length > 0 ? (
        <>
          <BooksGrid books={displayBooks} />
          
          {/* Pagination */}
          {!searchQuery && (
            <CursorPagination
              hasNextPage={hasNextPage}
              hasPrevPage={hasPrevPage}
              pageSize={limit}
              currentCount={displayBooks.length}
              nextCursor={nextCursor}
            />
          )}
        </>
      ) : (
        <EmptyState message={searchQuery ? `No books found for "${searchQuery}"` : "No books available"} />
      )}
    </>
  );
}
