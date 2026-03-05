import Image from "next/image";
import Link from "next/link";
import { BookDto } from "../../lib/api";

interface BookCardProps {
  book: BookDto;
  getBookCoverUrl: (bookId: string) => string;
}

export function BookCard({ book, getBookCoverUrl }: BookCardProps) {
  return (
    <Link href={`/books/${book.id}`} className="book-card">
      <div className="book-cover">
        <Image
          src={getBookCoverUrl(book.id)}
          alt={`Cover of ${book.title}`}
          width={150}
          height={220}
          className="cover-image"
          unoptimized
        />
      </div>
      <div className="book-info">
        <h3 className="book-title" title={book.title}>
          {book.title}
        </h3>
        {book.author && (
          <p className="book-author">{book.author}</p>
        )}
        {book.series && (
          <p className="book-series">
            {book.series}
            {book.volume && ` #${book.volume}`}
          </p>
        )}
        <div className="book-meta">
          <span className={`book-kind ${book.kind}`}>{book.kind.toUpperCase()}</span>
          {book.language && <span className="book-lang">{book.language.toUpperCase()}</span>}
        </div>
      </div>
    </Link>
  );
}

interface BooksGridProps {
  books: BookDto[];
  getBookCoverUrl: (bookId: string) => string;
}

export function BooksGrid({ books, getBookCoverUrl }: BooksGridProps) {
  return (
    <div className="books-grid">
      {books.map((book) => (
        <BookCard key={book.id} book={book} getBookCoverUrl={getBookCoverUrl} />
      ))}
    </div>
  );
}

interface EmptyStateProps {
  message: string;
}

export function EmptyState({ message }: EmptyStateProps) {
  return (
    <div className="empty-state">
      <p>{message}</p>
    </div>
  );
}
