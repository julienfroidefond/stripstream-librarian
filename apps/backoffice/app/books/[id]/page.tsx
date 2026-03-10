import { fetchLibraries, getBookCoverUrl, BookDto, apiFetch, ReadingStatus } from "../../../lib/api";
import { BookPreview } from "../../components/BookPreview";
import { ConvertButton } from "../../components/ConvertButton";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";

export const dynamic = "force-dynamic";

const readingStatusConfig: Record<ReadingStatus, { label: string; className: string }> = {
  unread: { label: "Non lu", className: "bg-muted/60 text-muted-foreground border border-border" },
  reading: { label: "En cours", className: "bg-amber-500/15 text-amber-600 dark:text-amber-400 border border-amber-500/30" },
  read: { label: "Lu", className: "bg-green-500/15 text-green-600 dark:text-green-400 border border-green-500/30" },
};

function ReadingStatusBadge({
  status,
  currentPage,
  lastReadAt,
}: {
  status: ReadingStatus;
  currentPage: number | null;
  lastReadAt: string | null;
}) {
  const { label, className } = readingStatusConfig[status];
  return (
    <div className="flex items-center gap-2">
      <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold ${className}`}>
        {label}
        {status === "reading" && currentPage != null && ` · p. ${currentPage}`}
      </span>
      {lastReadAt && (
        <span className="text-xs text-muted-foreground">
          {new Date(lastReadAt).toLocaleDateString()}
        </span>
      )}
    </div>
  );
}

async function fetchBook(bookId: string): Promise<BookDto | null> {
  try {
    return await apiFetch<BookDto>(`/books/${bookId}`);
  } catch {
    return null;
  }
}

export default async function BookDetailPage({
  params
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const [book, libraries] = await Promise.all([
    fetchBook(id),
    fetchLibraries().catch(() => [] as { id: string; name: string }[])
  ]);

  if (!book) {
    notFound();
  }

  const library = libraries.find(l => l.id === book.library_id);

  return (
    <>
      <div className="mb-6">
        <Link href="/books" className="inline-flex items-center text-sm text-muted-foreground hover:text-primary transition-colors">
          ← Back to books
        </Link>
      </div>

      <div className="flex flex-col lg:flex-row gap-8">
        <div className="flex-shrink-0">
          <div className="bg-card rounded-xl shadow-card border border-border p-4 inline-block">
            <Image
              src={getBookCoverUrl(book.id)}
              alt={`Cover of ${book.title}`}
              width={300}
              height={440}
              className="w-auto h-auto max-w-[300px] rounded-lg"
              unoptimized
              loading="lazy"
            />
          </div>
        </div>

        <div className="flex-1">
          <div className="bg-card rounded-xl shadow-sm border border-border p-6">
            <h1 className="text-3xl font-bold text-foreground mb-2">{book.title}</h1>
            
            {book.author && (
              <p className="text-lg text-muted-foreground mb-4">by {book.author}</p>
            )}

            {book.series && (
              <p className="text-sm text-muted-foreground mb-6">
                {book.series}
                {book.volume && <span className="ml-2 px-2 py-1 bg-primary/10 text-primary rounded text-xs">Volume {book.volume}</span>}
              </p>
            )}

            <div className="space-y-3">
              {book.reading_status && (
                <div className="flex items-center justify-between py-2 border-b border-border">
                  <span className="text-sm text-muted-foreground">Lecture :</span>
                  <ReadingStatusBadge
                    status={book.reading_status}
                    currentPage={book.reading_current_page ?? null}
                    lastReadAt={book.reading_last_read_at ?? null}
                  />
                </div>
              )}

              <div className="flex items-center justify-between py-2 border-b border-border">
                <span className="text-sm text-muted-foreground">Format:</span>
                <span className={`inline-flex px-2.5 py-1 rounded-full text-xs font-semibold ${
                  book.kind === 'epub' ? 'bg-primary/10 text-primary' : 'bg-muted/50 text-muted-foreground'
                }`}>
                  {book.kind.toUpperCase()}
                </span>
              </div>
              
              {book.volume && (
                <div className="flex items-center justify-between py-2 border-b border-border">
                  <span className="text-sm text-muted-foreground">Volume:</span>
                  <span className="text-sm text-foreground">{book.volume}</span>
                </div>
              )}
              
              {book.language && (
                <div className="flex items-center justify-between py-2 border-b border-border">
                  <span className="text-sm text-muted-foreground">Language:</span>
                  <span className="text-sm text-foreground">{book.language.toUpperCase()}</span>
                </div>
              )}
              
              {book.page_count && (
                <div className="flex items-center justify-between py-2 border-b border-border">
                  <span className="text-sm text-muted-foreground">Pages:</span>
                  <span className="text-sm text-foreground">{book.page_count}</span>
                </div>
              )}

              <div className="flex items-center justify-between py-2 border-b border-border">
                <span className="text-sm text-muted-foreground">Library:</span>
                <span className="text-sm text-foreground">{library?.name || book.library_id}</span>
              </div>

              {book.series && (
                <div className="flex items-center justify-between py-2 border-b border-border">
                  <span className="text-sm text-muted-foreground">Series:</span>
                  <span className="text-sm text-foreground">{book.series}</span>
                </div>
              )}

              {book.file_format && (
                <div className="flex items-center justify-between py-2 border-b border-border">
                  <span className="text-sm text-muted-foreground">File Format:</span>
                  <div className="flex items-center gap-3">
                    <span className="text-sm text-foreground">{book.file_format.toUpperCase()}</span>
                    {book.file_format === "cbr" && <ConvertButton bookId={book.id} />}
                  </div>
                </div>
              )}

              {book.file_parse_status && (
                <div className="flex items-center justify-between py-2 border-b border-border">
                  <span className="text-sm text-muted-foreground">Parse Status:</span>
                  <span className={`inline-flex px-2.5 py-1 rounded-full text-xs font-semibold ${
                    book.file_parse_status === 'success' ? 'bg-success/10 text-success' : 
                    book.file_parse_status === 'failed' ? 'bg-destructive/10 text-error' : 'bg-muted/50 text-muted-foreground'
                  }`}>
                    {book.file_parse_status}
                  </span>
                </div>
              )}

              {book.file_path && (
                <div className="flex flex-col py-2 border-b border-border">
                  <span className="text-sm text-muted-foreground mb-1">File Path:</span>
                  <code className="text-xs font-mono text-foreground break-all">{book.file_path}</code>
                </div>
              )}

              <div className="flex flex-col py-2 border-b border-border">
                <span className="text-sm text-muted-foreground mb-1">Book ID:</span>
                <code className="text-xs font-mono text-foreground break-all">{book.id}</code>
              </div>

              <div className="flex flex-col py-2 border-b border-border">
                <span className="text-sm text-muted-foreground mb-1">Library ID:</span>
                <code className="text-xs font-mono text-foreground break-all">{book.library_id}</code>
              </div>

              {book.updated_at && (
                <div className="flex items-center justify-between py-2">
                  <span className="text-sm text-muted-foreground">Updated:</span>
                  <span className="text-sm text-foreground">{new Date(book.updated_at).toLocaleString()}</span>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      {book.page_count && book.page_count > 0 && (
        <div className="mt-8">
          <BookPreview bookId={book.id} pageCount={book.page_count} />
        </div>
      )}
    </>
  );
}
