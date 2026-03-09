"use client";

import { useState } from "react";
import Image from "next/image";

const PAGE_SIZE = 5;

export function BookPreview({ bookId, pageCount }: { bookId: string; pageCount: number }) {
  const [offset, setOffset] = useState(0);

  const pages = Array.from({ length: PAGE_SIZE }, (_, i) => offset + i + 1).filter(
    (p) => p <= pageCount
  );

  return (
    <div className="bg-card rounded-xl border border-border p-6">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold text-foreground">
          Preview
          <span className="ml-2 text-sm font-normal text-muted-foreground">
            pages {offset + 1}–{Math.min(offset + PAGE_SIZE, pageCount)} / {pageCount}
          </span>
        </h2>
        <div className="flex gap-2">
          <button
            onClick={() => setOffset((o) => Math.max(0, o - PAGE_SIZE))}
            disabled={offset === 0}
            className="px-3 py-1.5 text-sm rounded-lg border border-border bg-muted/50 text-foreground hover:bg-muted disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
          >
            ← Prev
          </button>
          <button
            onClick={() => setOffset((o) => Math.min(o + PAGE_SIZE, pageCount - 1))}
            disabled={offset + PAGE_SIZE >= pageCount}
            className="px-3 py-1.5 text-sm rounded-lg border border-border bg-muted/50 text-foreground hover:bg-muted disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
          >
            Next →
          </button>
        </div>
      </div>

      <div className="grid grid-cols-5 gap-3">
        {pages.map((pageNum) => (
          <div key={pageNum} className="flex flex-col items-center gap-1.5">
            <div className="relative w-full aspect-[2/3] bg-muted rounded-lg overflow-hidden border border-border">
              <Image
                src={`/api/books/${bookId}/pages/${pageNum}?format=webp&width=600&quality=80`}
                alt={`Page ${pageNum}`}
                fill
                className="object-contain"
                unoptimized
              />
            </div>
            <span className="text-xs text-muted-foreground">{pageNum}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
