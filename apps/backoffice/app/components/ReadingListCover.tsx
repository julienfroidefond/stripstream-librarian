import { getBookCoverUrl } from "@/lib/api";
import { CoverFan } from "@/app/components/ui";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function coverSrc(value: string): string {
  return UUID_RE.test(value) ? getBookCoverUrl(value) : value;
}

export function ReadingListCover({ covers, name, className = "" }: { covers: string[]; name: string; className?: string }) {
  const main = covers[0] ?? null;
  const fan = covers.slice(0, 5);
  const sizeClass = className || "aspect-[2/3]";

  if (!main) {
    return (
      <div className={`w-full bg-gradient-to-br from-cyan-500/20 to-primary/20 flex items-center justify-center ${sizeClass}`}>
        <svg className="w-10 h-10 text-muted-foreground/40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
        </svg>
      </div>
    );
  }

  return (
    <CoverFan
      className={sizeClass}
      background={<img src={coverSrc(main)} alt="" className="h-full w-full scale-110 object-cover blur-xl opacity-35" />}
      covers={fan.map((bookId, index) => (
        <img key={`${bookId}-${index}`} src={coverSrc(bookId)} alt={index === Math.round((fan.length - 1) / 2) ? name : ""} className="h-full w-full object-cover" />
      ))}
    />
  );
}
