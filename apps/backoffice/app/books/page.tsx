import { fetchBooks, searchBooks, fetchLibraries, BookDto, LibraryDto, getBookCoverUrl } from "../../lib/api";
import { BooksGrid, EmptyState } from "../components/BookCard";
import { Card, CardContent, Button, FormField, FormInput, FormSelect, FormRow, OffsetPagination } from "../components/ui";
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
  const page = typeof searchParamsAwaited.page === "string" ? parseInt(searchParamsAwaited.page) : 1;
  const limit = typeof searchParamsAwaited.limit === "string" ? parseInt(searchParamsAwaited.limit) : 20;

  const [libraries] = await Promise.all([
    fetchLibraries().catch(() => [] as LibraryDto[])
  ]);

  let books: BookDto[] = [];
  let total = 0;
  let searchResults: BookDto[] | null = null;
  let totalHits: number | null = null;

  if (searchQuery) {
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
        updated_at: "",
        reading_status: "unread" as const,
        reading_current_page: null,
        reading_last_read_at: null,
      }));
      totalHits = searchResponse.estimated_total_hits;
    }
  } else {
    const booksPage = await fetchBooks(libraryId, undefined, page, limit).catch(() => ({
      items: [] as BookDto[],
      total: 0,
      page: 1,
      limit,
    }));
    books = booksPage.items;
    total = booksPage.total;
  }

  const displayBooks = (searchResults || books).map(book => ({
    ...book,
    coverUrl: getBookCoverUrl(book.id)
  }));

  const totalPages = Math.ceil(total / limit);

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-success" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
          </svg>
          Books
        </h1>
      </div>
      
      {/* Search Bar - Style compact et propre */}
      <Card className="mb-6">
        <CardContent className="pt-6">
          <form className="flex flex-col sm:flex-row gap-3 items-start sm:items-end">
            <FormField className="flex-1 w-full">
              <label className="block text-sm font-medium text-foreground mb-1.5">Search</label>
              <FormInput 
                name="q" 
                placeholder="Search by title, author, series..." 
                defaultValue={searchQuery}
                className="w-full"
              />
            </FormField>
            <FormField className="w-full sm:w-48">
              <label className="block text-sm font-medium text-foreground mb-1.5">Library</label>
              <FormSelect name="library" defaultValue={libraryId || ""}>
                <option value="">All libraries</option>
                {libraries.map((lib) => (
                  <option key={lib.id} value={lib.id}>
                    {lib.name}
                  </option>
                ))}
              </FormSelect>
            </FormField>
            <div className="flex gap-2 w-full sm:w-auto">
              <Button type="submit" className="flex-1 sm:flex-none">
                <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                </svg>
                Search
              </Button>
              {searchQuery && (
                <Link 
                  href="/books" 
                  className="
                    inline-flex items-center justify-center
                    h-10 px-4
                    border border-input
                    text-sm font-medium
                    text-muted-foreground
                    bg-background
                    rounded-md
                    hover:bg-accent hover:text-accent-foreground
                    transition-colors duration-200
                    flex-1 sm:flex-none
                  "
                >
                  Clear
                </Link>
              )}
            </div>
          </form>
        </CardContent>
      </Card>

      {/* Résultats */}
      {searchQuery && totalHits !== null && (
        <p className="text-sm text-muted-foreground mb-4">
          Found {totalHits} result{totalHits !== 1 ? 's' : ''} for &quot;{searchQuery}&quot;
        </p>
      )}

      {/* Grille de livres */}
      {displayBooks.length > 0 ? (
        <>
          <BooksGrid books={displayBooks} />
          
          {!searchQuery && (
            <OffsetPagination
              currentPage={page}
              totalPages={totalPages}
              pageSize={limit}
              totalItems={total}
            />
          )}
        </>
      ) : (
        <EmptyState message={searchQuery ? `No books found for "${searchQuery}"` : "No books available"} />
      )}
    </>
  );
}
