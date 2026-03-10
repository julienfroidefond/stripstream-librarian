"use client";

import { useState } from "react";
import Image from "next/image";
import Link from "next/link";
import { BookDto, ReadingStatus } from "../../lib/api";

const readingStatusOverlay: Record<ReadingStatus, { label: string; className: string } | null> = {
  unread: null,
  reading: { label: "En cours", className: "bg-amber-500/90 text-white" },
  read: { label: "Lu", className: "bg-green-600/90 text-white" },
};

interface BookCardProps {
  book: BookDto & { coverUrl?: string };
  readingStatus?: ReadingStatus;
}

function BookImage({ src, alt }: { src: string; alt: string }) {
  const [isLoaded, setIsLoaded] = useState(false);

  return (
    <div className="relative aspect-[2/3] overflow-hidden bg-muted">
      {/* Skeleton */}
      <div 
        className={`absolute inset-0 bg-muted/50 animate-pulse transition-opacity duration-300 ${
          isLoaded ? 'opacity-0 pointer-events-none' : 'opacity-100'
        }`}
      />
      
      {/* Image */}
      <Image
        src={src}
        alt={alt}
        fill
        className={`object-cover group-hover:scale-105 transition-transform duration-300 ${
          isLoaded ? 'opacity-100' : 'opacity-0'
        }`}
        sizes="(max-width: 640px) 50vw, (max-width: 768px) 33vw, (max-width: 1024px) 25vw, 16vw"
        onLoad={() => setIsLoaded(true)}
        unoptimized
      />
    </div>
  );
}

export function BookCard({ book, readingStatus }: BookCardProps) {
  const coverUrl = book.coverUrl || `/api/books/${book.id}/thumbnail`;
  const status = readingStatus ?? book.reading_status;
  const overlay = status ? readingStatusOverlay[status] : null;

  return (
    <Link
      href={`/books/${book.id}`}
      className="group block bg-card rounded-xl border border-border/60 shadow-sm hover:shadow-md hover:-translate-y-1 transition-all duration-200 overflow-hidden"
    >
      <div className="relative">
        <BookImage
          src={coverUrl}
          alt={`Cover of ${book.title}`}
        />
        {overlay && (
          <span className={`absolute bottom-2 left-2 px-2 py-0.5 rounded-full text-[10px] font-bold tracking-wide ${overlay.className}`}>
            {overlay.label}
          </span>
        )}
      </div>
      
      {/* Book Info */}
      <div className="p-4">
        <h3 
          className="font-semibold text-foreground mb-1 line-clamp-2 min-h-[2.5rem]" 
          title={book.title}
        >
          {book.title}
        </h3>
        
        {book.author && (
          <p className="text-sm text-muted-foreground mb-1 truncate">{book.author}</p>
        )}
        
        {book.series && (
          <p className="text-xs text-muted-foreground/80 truncate mb-2">
            {book.series}
            {book.volume && <span className="text-primary font-medium"> #{book.volume}</span>}
          </p>
        )}
        
        {/* Meta Tags */}
        <div className="flex items-center gap-2 mt-2">
          <span className={`
            px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider rounded-full
            ${book.kind === 'cbz' ? 'bg-success/10 text-success' : ''}
            ${book.kind === 'cbr' ? 'bg-warning/10 text-warning' : ''}
            ${book.kind === 'pdf' ? 'bg-destructive/10 text-destructive' : ''}
          `}>
            {book.kind}
          </span>
          {book.language && (
            <span className="px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider rounded-full bg-primary/10 text-primary">
              {book.language}
            </span>
          )}
        </div>
      </div>
    </Link>
  );
}

interface BooksGridProps {
  books: (BookDto & { coverUrl?: string })[];
}

export function BooksGrid({ books }: BooksGridProps) {
  return (
    <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
      {books.map((book) => (
        <BookCard key={book.id} book={book} />
      ))}
    </div>
  );
}

interface EmptyStateProps {
  message: string;
}

export function EmptyState({ message }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center">
      <div className="w-16 h-16 mb-4 text-muted-foreground/30">
        <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
        </svg>
      </div>
      <p className="text-muted-foreground text-lg">{message}</p>
    </div>
  );
}
