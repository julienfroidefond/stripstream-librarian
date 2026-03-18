import Link from "next/link";
import { Card, Badge } from "./ui";
import { getServerTranslations } from "../../lib/i18n/server";

interface LibrarySubPageHeaderProps {
  library: {
    id: string;
    name: string;
    root_path: string;
    book_count: number;
    enabled: boolean;
  };
  title: string;
  icon: React.ReactNode;
  iconColor?: string;
  filterInfo?: {
    label: string;
    clearHref: string;
    clearLabel: string;
  };
}

export async function LibrarySubPageHeader({
  library,
  title,
  icon,
  iconColor = "text-primary",
  filterInfo
}: LibrarySubPageHeaderProps) {
  const { t } = await getServerTranslations();
  return (
    <div className="space-y-6">
      {/* Header avec breadcrumb intégré */}
      <div>
        <div className="flex items-center gap-2 mb-2">
          <Link 
            href="/libraries" 
            className="inline-flex items-center text-sm text-muted-foreground hover:text-primary transition-colors duration-200"
          >
            <svg className="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
            {t("libraryHeader.libraries")}
          </Link>
          <span className="text-muted-foreground">/</span>
          <span className="text-sm text-foreground font-medium">{library.name}</span>
        </div>
        
        <h1 className="text-3xl font-bold text-foreground flex items-center gap-3">
          <span className={iconColor}>{icon}</span>
          {title}
        </h1>
      </div>
      
      {/* Info Bar - Version améliorée */}
      <Card className="bg-muted/30 border-border/40">
        <div className="p-4">
          <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-sm">
            {/* Path */}
            <div className="flex items-center gap-2">
              <svg className="w-4 h-4 text-muted-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
              </svg>
              <code className="text-xs font-mono text-muted-foreground bg-background px-2 py-1 rounded border border-border/60">
                {library.root_path}
              </code>
            </div>
            
            {/* Divider */}
            <span className="hidden sm:block w-px h-4 bg-border" />
            
            {/* Book count */}
            <div className="flex items-center gap-2">
              <svg className="w-4 h-4 text-muted-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
              </svg>
              <span className="text-foreground">
                <span className="text-muted-foreground ml-1">{t("libraryHeader.bookCount", { count: library.book_count, plural: library.book_count !== 1 ? "s" : "" })}</span>
              </span>
            </div>
            
            {/* Divider */}
            <span className="hidden sm:block w-px h-4 bg-border" />
            
            {/* Status */}
            <Badge 
              variant={library.enabled ? "success" : "muted"}
              className="text-xs"
            >
              {library.enabled ? t("libraryHeader.enabled") : t("libraries.disabled")}
            </Badge>
          </div>
        </div>
      </Card>

      {/* Filter Info (optionnel) */}
      {filterInfo && (
        <div className="flex items-center justify-between py-2">
          <p className="text-muted-foreground text-sm">
            {filterInfo.label}
          </p>
          <Link 
            href={filterInfo.clearHref as `/libraries/${string}/books`}
            className="text-sm text-primary hover:text-primary/80 font-medium"
          >
            {filterInfo.clearLabel}
          </Link>
        </div>
      )}
    </div>
  );
}
