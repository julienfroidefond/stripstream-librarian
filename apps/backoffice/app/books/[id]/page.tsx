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
      <div className="breadcrumb">
        <Link href="/books">← Back to books</Link>
      </div>

      <div className="book-detail">
        <div className="book-detail-cover">
          <Image
            src={getBookCoverUrl(book.id)}
            alt={`Cover of ${book.title}`}
            width={300}
            height={440}
            className="detail-cover-image"
            unoptimized
            loading="lazy"
          />
        </div>

        <div className="book-detail-info">
          <h1>{book.title}</h1>
          
          {book.author && (
            <p className="detail-author">by {book.author}</p>
          )}

          {book.series && (
            <p className="detail-series">
              {book.series}
              {book.volume && <span className="volume">Volume {book.volume}</span>}
            </p>
          )}

          <div className="detail-meta">
            <div className="meta-row">
              <span className="meta-label">Format:</span>
              <span className={`book-kind ${book.kind}`}>{book.kind.toUpperCase()}</span>
            </div>
            
            {book.volume && (
              <div className="meta-row">
                <span className="meta-label">Volume:</span>
                <span>{book.volume}</span>
              </div>
            )}
            
            {book.language && (
              <div className="meta-row">
                <span className="meta-label">Language:</span>
                <span>{book.language.toUpperCase()}</span>
              </div>
            )}
            
            {book.page_count && (
              <div className="meta-row">
                <span className="meta-label">Pages:</span>
                <span>{book.page_count}</span>
              </div>
            )}

            <div className="meta-row">
              <span className="meta-label">Library:</span>
              <span>{library?.name || book.library_id}</span>
            </div>

            {book.series && (
              <div className="meta-row">
                <span className="meta-label">Series:</span>
                <span>{book.series}</span>
              </div>
            )}

            {book.file_format && (
              <div className="meta-row">
                <span className="meta-label">File Format:</span>
                <span>{book.file_format.toUpperCase()}</span>
              </div>
            )}

            {book.file_parse_status && (
              <div className="meta-row">
                <span className="meta-label">Parse Status:</span>
                <span className={`status-${book.file_parse_status}`}>{book.file_parse_status}</span>
              </div>
            )}

            {book.file_path && (
              <div className="meta-row">
                <span className="meta-label">File Path:</span>
                <code className="file-path">{book.file_path}</code>
              </div>
            )}

            <div className="meta-row">
              <span className="meta-label">Book ID:</span>
              <code className="book-id">{book.id}</code>
            </div>

            <div className="meta-row">
              <span className="meta-label">Library ID:</span>
              <code className="book-id">{book.library_id}</code>
            </div>

            {book.updated_at && (
              <div className="meta-row">
                <span className="meta-label">Updated:</span>
                <span>{new Date(book.updated_at).toLocaleString()}</span>
              </div>
            )}
          </div>
        </div>
      </div>
    </>
  );
}
