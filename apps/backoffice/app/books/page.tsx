import { fetchBooks, searchBooks, fetchLibraries, BookDto, LibraryDto, getBookCoverUrl } from "../../lib/api";
import { BooksGrid, EmptyState } from "../components/BookCard";
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
  
  const [libraries] = await Promise.all([
    fetchLibraries().catch(() => [] as LibraryDto[])
  ]);

  let books: BookDto[] = [];
  let nextCursor: string | null = null;
  let searchResults: BookDto[] | null = null;
  let totalHits: number | null = null;

  if (searchQuery) {
    // Mode recherche
    const searchResponse = await searchBooks(searchQuery, libraryId).catch(() => null);
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
    // Mode liste
    const booksPage = await fetchBooks(libraryId).catch(() => ({ items: [] as BookDto[], next_cursor: null }));
    books = booksPage.items;
    nextCursor = booksPage.next_cursor;
  }

  const displayBooks = searchResults || books;

  return (
    <>
      <h1>Books</h1>
      
      {/* Filtres et recherche */}
      <div className="card">
        <form className="search-form">
          <input 
            name="q" 
            placeholder="Search books..." 
            defaultValue={searchQuery}
            className="search-input"
          />
          <select name="library" defaultValue={libraryId || ""}>
            <option value="">All libraries</option>
            {libraries.map((lib) => (
              <option key={lib.id} value={lib.id}>
                {lib.name}
              </option>
            ))}
          </select>
          <button type="submit">Search</button>
          {searchQuery && (
            <Link href="/books" className="button secondary">Clear</Link>
          )}
        </form>
      </div>

      {/* Résultats de recherche */}
      {searchQuery && totalHits !== null && (
        <p className="results-info">
          Found {totalHits} result{totalHits !== 1 ? 's' : ''} for &quot;{searchQuery}&quot;
        </p>
      )}

      {/* Grille de livres */}
      {displayBooks.length > 0 ? (
        <>
          <BooksGrid books={displayBooks} getBookCoverUrl={getBookCoverUrl} />
          
          {/* Pagination */}
          {!searchQuery && nextCursor && (
            <div className="pagination">
              <form>
                <input type="hidden" name="library" value={libraryId || ""} />
                <input type="hidden" name="cursor" value={nextCursor} />
                <button type="submit">Load more</button>
              </form>
            </div>
          )}
        </>
      ) : (
        <EmptyState message={searchQuery ? `No books found for "${searchQuery}"` : "No books available"} />
      )}
    </>
  );
}
