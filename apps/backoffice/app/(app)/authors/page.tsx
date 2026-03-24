import { fetchAuthors, AuthorsPageDto } from "@/lib/api";
import { getServerTranslations } from "@/lib/i18n/server";
import { LiveSearchForm } from "@/app/components/LiveSearchForm";
import { Card, CardContent, OffsetPagination } from "@/app/components/ui";
import Link from "next/link";

export const dynamic = "force-dynamic";

export default async function AuthorsPage({
  searchParams,
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const { t } = await getServerTranslations();
  const searchParamsAwaited = await searchParams;
  const searchQuery = typeof searchParamsAwaited.q === "string" ? searchParamsAwaited.q : "";
  const sort = typeof searchParamsAwaited.sort === "string" ? searchParamsAwaited.sort : undefined;
  const page = typeof searchParamsAwaited.page === "string" ? parseInt(searchParamsAwaited.page) : 1;
  const limit = typeof searchParamsAwaited.limit === "string" ? parseInt(searchParamsAwaited.limit) : 20;

  const authorsPage = await fetchAuthors(
    searchQuery || undefined,
    page,
    limit,
    sort,
  ).catch(() => ({ items: [], total: 0, page: 1, limit }) as AuthorsPageDto);

  const totalPages = Math.ceil(authorsPage.total / limit);
  const hasFilters = searchQuery || sort;

  const sortOptions = [
    { value: "", label: t("authors.sortName") },
    { value: "books", label: t("authors.sortBooks") },
  ];

  return (
    <>
      <div className="mb-6">
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <svg className="w-8 h-8 text-violet-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
          </svg>
          {t("authors.title")}
        </h1>
      </div>

      <Card className="mb-6">
        <CardContent className="pt-6">
          <LiveSearchForm
            basePath="/authors"
            fields={[
              { name: "q", type: "text", label: t("common.search"), placeholder: t("authors.searchPlaceholder") },
              { name: "sort", type: "select", label: t("books.sort"), options: sortOptions },
            ]}
          />
        </CardContent>
      </Card>

      {/* Results count */}
      <p className="text-sm text-muted-foreground mb-4">
        {authorsPage.total} {t("authors.title").toLowerCase()}
        {searchQuery && <> {t("authors.matchingQuery")} &quot;{searchQuery}&quot;</>}
      </p>

      {/* Authors List */}
      {authorsPage.items.length > 0 ? (
        <>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
            {authorsPage.items.map((author) => (
              <Link
                key={author.name}
                href={`/authors/${encodeURIComponent(author.name)}`}
                className="group"
              >
                <div className="bg-card rounded-xl shadow-sm border border-border/60 overflow-hidden hover:shadow-md hover:-translate-y-1 transition-all duration-200 p-4">
                  <div className="flex items-center gap-3">
                    <div className="w-10 h-10 rounded-full bg-accent/50 flex items-center justify-center flex-shrink-0">
                      <span className="text-lg font-semibold text-violet-500">
                        {author.name.charAt(0).toUpperCase()}
                      </span>
                    </div>
                    <div className="min-w-0">
                      <h3 className="font-medium text-foreground truncate text-sm group-hover:text-violet-500 transition-colors" title={author.name}>
                        {author.name}
                      </h3>
                      <div className="flex items-center gap-3 mt-0.5">
                        <span className="text-xs text-muted-foreground">
                          {t("authors.bookCount", { count: String(author.book_count), plural: author.book_count !== 1 ? "s" : "" })}
                        </span>
                        <span className="text-xs text-muted-foreground">
                          {t("authors.seriesCount", { count: String(author.series_count), plural: author.series_count !== 1 ? "s" : "" })}
                        </span>
                      </div>
                    </div>
                  </div>
                </div>
              </Link>
            ))}
          </div>

          <OffsetPagination
            currentPage={page}
            totalPages={totalPages}
            pageSize={limit}
            totalItems={authorsPage.total}
          />
        </>
      ) : (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <div className="w-16 h-16 mb-4 text-muted-foreground/30">
            <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
            </svg>
          </div>
          <p className="text-muted-foreground text-lg">
            {hasFilters ? t("authors.noResults") : t("authors.noAuthors")}
          </p>
        </div>
      )}
    </>
  );
}
