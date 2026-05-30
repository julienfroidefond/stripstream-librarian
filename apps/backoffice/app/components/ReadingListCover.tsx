import { getBookCoverUrl } from "@/lib/api";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function coverSrc(value: string): string {
  return UUID_RE.test(value) ? getBookCoverUrl(value) : value;
}

export function ReadingListCover({ covers, name }: { covers: string[]; name: string }) {
  const main = covers[0] ?? null;
  const strip = covers.slice(1, 4); // jusqu'à 3 thumbs

  if (!main) {
    return (
      <div className="w-full aspect-[2/3] bg-gradient-to-br from-cyan-500/20 to-primary/20 flex items-center justify-center">
        <svg className="w-10 h-10 text-muted-foreground/40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
        </svg>
      </div>
    );
  }

  return (
    <div className="relative w-full aspect-[2/3] overflow-hidden">
      <img src={coverSrc(main)} alt={name} className="w-full h-full object-cover" />

      {strip.length > 0 && (
        <>
          <div className="absolute inset-x-0 bottom-0 h-1/2 bg-gradient-to-t from-black via-black/70 to-transparent" />
          <div className="absolute inset-x-0 bottom-3 flex gap-2 justify-center px-3">
            {strip.map((bookId) => (
              <div
                key={bookId}
                className="w-12 aspect-[2/3] rounded-md overflow-hidden border border-white/30 shadow-lg flex-shrink-0"
              >
                <img src={coverSrc(bookId)} alt="" className="w-full h-full object-cover" />
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
