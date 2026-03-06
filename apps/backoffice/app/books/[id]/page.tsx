import { fetchLibraries, getBookCoverUrl, BookDto, apiFetch } from "../../../lib/api";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";

export const dynamic = "force-dynamic";

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
        <Link href="/books" className="inline-flex items-center text-sm text-muted hover:text-primary transition-colors">
          ← Back to books
        </Link>
      </div>

      <div className="flex flex-col lg:flex-row gap-8">
        <div className="flex-shrink-0">
          <div className="bg-card rounded-xl shadow-card border border-line p-4 inline-block">
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
          <div className="bg-card rounded-xl shadow-soft border border-line p-6">
            <h1 className="text-3xl font-bold text-foreground mb-2">{book.title}</h1>
            
            {book.author && (
              <p className="text-lg text-muted mb-4">by {book.author}</p>
            )}

            {book.series && (
              <p className="text-sm text-muted mb-6">
                {book.series}
                {book.volume && <span className="ml-2 px-2 py-1 bg-primary-soft text-primary rounded text-xs">Volume {book.volume}</span>}
              </p>
            )}

            <div className="space-y-3">
              <div className="flex items-center justify-between py-2 border-b border-line">
                <span className="text-sm text-muted">Format:</span>
                <span className={`inline-flex px-2.5 py-1 rounded-full text-xs font-semibold ${
                  book.kind === 'epub' ? 'bg-primary-soft text-primary' : 'bg-muted/20 text-muted'
                }`}>
                  {book.kind.toUpperCase()}
                </span>
              </div>
              
              {book.volume && (
                <div className="flex items-center justify-between py-2 border-b border-line">
                  <span className="text-sm text-muted">Volume:</span>
                  <span className="text-sm text-foreground">{book.volume}</span>
                </div>
              )}
              
              {book.language && (
                <div className="flex items-center justify-between py-2 border-b border-line">
                  <span className="text-sm text-muted">Language:</span>
                  <span className="text-sm text-foreground">{book.language.toUpperCase()}</span>
                </div>
              )}
              
              {book.page_count && (
                <div className="flex items-center justify-between py-2 border-b border-line">
                  <span className="text-sm text-muted">Pages:</span>
                  <span className="text-sm text-foreground">{book.page_count}</span>
                </div>
              )}

              <div className="flex items-center justify-between py-2 border-b border-line">
                <span className="text-sm text-muted">Library:</span>
                <span className="text-sm text-foreground">{library?.name || book.library_id}</span>
              </div>

              {book.series && (
                <div className="flex items-center justify-between py-2 border-b border-line">
                  <span className="text-sm text-muted">Series:</span>
                  <span className="text-sm text-foreground">{book.series}</span>
                </div>
              )}

              {book.file_format && (
                <div className="flex items-center justify-between py-2 border-b border-line">
                  <span className="text-sm text-muted">File Format:</span>
                  <span className="text-sm text-foreground">{book.file_format.toUpperCase()}</span>
                </div>
              )}

              {book.file_parse_status && (
                <div className="flex items-center justify-between py-2 border-b border-line">
                  <span className="text-sm text-muted">Parse Status:</span>
                  <span className={`inline-flex px-2.5 py-1 rounded-full text-xs font-semibold ${
                    book.file_parse_status === 'success' ? 'bg-success-soft text-success' : 
                    book.file_parse_status === 'failed' ? 'bg-error-soft text-error' : 'bg-muted/20 text-muted'
                  }`}>
                    {book.file_parse_status}
                  </span>
                </div>
              )}

              {book.file_path && (
                <div className="flex flex-col py-2 border-b border-line">
                  <span className="text-sm text-muted mb-1">File Path:</span>
                  <code className="text-xs font-mono text-foreground break-all">{book.file_path}</code>
                </div>
              )}

              <div className="flex flex-col py-2 border-b border-line">
                <span className="text-sm text-muted mb-1">Book ID:</span>
                <code className="text-xs font-mono text-foreground break-all">{book.id}</code>
              </div>

              <div className="flex flex-col py-2 border-b border-line">
                <span className="text-sm text-muted mb-1">Library ID:</span>
                <code className="text-xs font-mono text-foreground break-all">{book.library_id}</code>
              </div>

              {book.updated_at && (
                <div className="flex items-center justify-between py-2">
                  <span className="text-sm text-muted">Updated:</span>
                  <span className="text-sm text-foreground">{new Date(book.updated_at).toLocaleString()}</span>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
