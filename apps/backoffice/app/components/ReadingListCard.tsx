import type { ReactNode } from "react";
import Link from "next/link";
import type { ReadingListDto } from "@/lib/api";
import { ReadingListCover } from "./ReadingListCover";
import { Card, CardContent, CardHeader, CardTitle } from "./ui";

type ReadingListCardProps = {
  list: ReadingListDto;
  seriesCountLabel: string;
  booksLabel: string;
  readLabel: string;
  action?: ReactNode;
};

/** Shared reading-list card for the Lists page and the Series list view. */
export function ReadingListCard({ list, seriesCountLabel, booksLabel, readLabel, action }: ReadingListCardProps) {
  const href = `/reading-lists/${list.id}` as any;
  const progress = list.book_count > 0 ? Math.round((list.books_read_count / list.book_count) * 100) : 0;

  return (
    <Card className="flex flex-col overflow-hidden">
      <Link href={href} className="group block h-48 bg-muted/10">
        <ReadingListCover covers={list.preview_covers} name={list.name} className="h-full aspect-auto transition-transform duration-200 hover:scale-[1.02]" />
      </Link>
      <CardHeader className="pb-2">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <Link href={href} className="block hover:text-primary transition-colors">
              <CardTitle className="truncate text-lg"><span title={list.name}>{list.name}</span></CardTitle>
            </Link>
            {list.description && <p className="mt-1 line-clamp-2 text-sm text-muted-foreground">{list.description}</p>}
          </div>
          {action}
        </div>
      </CardHeader>
      <CardContent className="flex-1 pt-0">
        <Link href={href} className="block rounded-lg bg-muted/50 p-3 transition-colors hover:bg-accent">
          <div className="grid grid-cols-2 gap-3">
            <div>
              <span className="block text-2xl font-bold text-primary">{list.series_count}</span>
              <span className="text-xs text-muted-foreground">{seriesCountLabel}</span>
            </div>
            <div className="border-l border-border/70 pl-3">
              <span className="block text-2xl font-bold text-foreground">{list.book_count}</span>
              <span className="text-xs text-muted-foreground">{booksLabel}</span>
            </div>
          </div>
          {list.book_count > 0 && (
            <div className="mt-3">
              <div className="mb-1 flex items-center justify-between text-[11px] text-muted-foreground">
                <span>{readLabel}</span>
                <span>{list.books_read_count} / {list.book_count} · {progress}%</span>
              </div>
              <div className="h-1.5 overflow-hidden rounded-full bg-background/80">
                <div className="h-full rounded-full bg-primary transition-all" style={{ width: `${progress}%` }} />
              </div>
            </div>
          )}
        </Link>
      </CardContent>
    </Card>
  );
}
