"use client";

import { useState } from "react";
import Link from "next/link";
import Image from "next/image";
import type { CurrentlyReadingItem, RecentlyReadItem } from "@/lib/api";
import { getBookCoverUrl } from "@/lib/api";

function FilterPills({ usernames, selected, allLabel, onSelect, minUsers = 2 }: {
  usernames: string[];
  selected: string | null;
  allLabel: string;
  onSelect: (u: string | null) => void;
  minUsers?: number;
}) {
  if (usernames.length < minUsers) return null;
  return (
    <div className="flex flex-wrap gap-1.5 mb-3">
      <button
        onClick={() => onSelect(null)}
        className={`px-2.5 py-0.5 rounded-full text-xs font-medium transition-colors ${
          selected === null
            ? "bg-primary text-primary-foreground"
            : "bg-muted text-muted-foreground hover:bg-muted/80"
        }`}
      >
        {allLabel}
      </button>
      {usernames.map((u) => (
        <button
          key={u}
          onClick={() => onSelect(u === selected ? null : u)}
          className={`px-2.5 py-0.5 rounded-full text-xs font-medium transition-colors ${
            selected === u
              ? "bg-primary text-primary-foreground"
              : "bg-muted text-muted-foreground hover:bg-muted/80"
          }`}
        >
          {u}
        </button>
      ))}
    </div>
  );
}

export function CurrentlyReadingList({
  items,
  allLabel,
  emptyLabel,
  pageProgressTemplate,
}: {
  items: CurrentlyReadingItem[];
  allLabel: string;
  emptyLabel: string;
  /** Template with {{current}} and {{total}} placeholders */
  pageProgressTemplate: string;
}) {
  const usernames = [...new Set(items.map((i) => i.username).filter((u): u is string => !!u))];
  const [selected, setSelected] = useState<string | null>(null);
  const filtered = selected ? items.filter((i) => i.username === selected) : items;

  return (
    <div>
      <FilterPills usernames={usernames} selected={selected} allLabel={allLabel} onSelect={setSelected} />
      {filtered.length === 0 ? (
        <p className="text-muted-foreground text-sm text-center py-4">{emptyLabel}</p>
      ) : (
        <div className="space-y-3 max-h-[216px] overflow-y-auto pr-1">
          {filtered.slice(0, 8).map((book) => {
            const pct = book.page_count > 0 ? Math.round((book.current_page / book.page_count) * 100) : 0;
            return (
              <Link key={`${book.book_id}-${book.username}`} href={`/books/${book.book_id}` as any} className="flex items-center gap-3 group">
                <Image
                  src={getBookCoverUrl(book.book_id)}
                  alt={book.title}
                  width={40}
                  height={56}
                  className="w-10 h-14 object-cover rounded shadow-sm shrink-0 bg-muted"
                />
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium text-foreground truncate group-hover:text-primary transition-colors">{book.title}</p>
                  {book.series && <p className="text-xs text-muted-foreground truncate">{book.series}</p>}
                  {book.username && usernames.length > 1 && (
                    <p className="text-[10px] text-primary/70 font-medium">{book.username}</p>
                  )}
                  <div className="mt-1.5 flex items-center gap-2">
                    <div className="h-1.5 flex-1 bg-muted rounded-full overflow-hidden">
                      <div className="h-full bg-warning rounded-full transition-all" style={{ width: `${pct}%` }} />
                    </div>
                    <span className="text-[10px] text-muted-foreground shrink-0">{pct}%</span>
                  </div>
                  <p className="text-[10px] text-muted-foreground mt-0.5">{pageProgressTemplate.replace("{{current}}", String(book.current_page)).replace("{{total}}", String(book.page_count))}</p>
                </div>
              </Link>
            );
          })}
        </div>
      )}
    </div>
  );
}

export function RecentlyReadList({
  items,
  allLabel,
  emptyLabel,
}: {
  items: RecentlyReadItem[];
  allLabel: string;
  emptyLabel: string;
}) {
  const usernames = [...new Set(items.map((i) => i.username).filter((u): u is string => !!u))];
  const [selected, setSelected] = useState<string | null>(null);
  const filtered = selected ? items.filter((i) => i.username === selected) : items;
  const multiUser = usernames.length > 0;

  return (
    <div>
      <FilterPills usernames={usernames} selected={selected} allLabel={allLabel} onSelect={setSelected} minUsers={1} />
      {filtered.length === 0 ? (
        <p className="text-muted-foreground text-sm text-center py-4">{emptyLabel}</p>
      ) : (
        <div className="space-y-3 max-h-[216px] overflow-y-auto pr-1">
          {filtered.map((book) => (
            <Link key={`${book.book_id}-${book.username}`} href={`/books/${book.book_id}` as any} className="flex items-center gap-3 group">
              <Image
                src={getBookCoverUrl(book.book_id)}
                alt={book.title}
                width={40}
                height={56}
                className="w-10 h-14 object-cover rounded shadow-sm shrink-0 bg-muted"
              />
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium text-foreground truncate group-hover:text-primary transition-colors">{book.title}</p>
                {book.series && <p className="text-xs text-muted-foreground truncate">{book.series}</p>}
                {book.username && multiUser && (
                  <p className="text-[10px] text-primary/70 font-medium">{book.username}</p>
                )}
              </div>
              <span className="text-xs text-muted-foreground shrink-0">{book.last_read_at}</span>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
