import { fetchLibraries, apiFetch } from "@/lib/api";
import { getServerTranslations } from "@/lib/i18n/server";
import { Icon } from "@/app/components/ui";
import { DiscoveryGrid } from "./components/DiscoveryGrid";

export const dynamic = "force-dynamic";

export interface DiscoverySuggestion {
  provider: string;
  external_id: string;
  title: string;
  authors: string[];
  description: string | null;
  genres: string[];
  cover_url: string | null;
  external_url: string | null;
  start_year: number | null;
  total_volumes: number | null;
  status: string | null;
}

export default async function DiscoveryPage({
  searchParams,
}: {
  searchParams: Promise<{ [key: string]: string | string[] | undefined }>;
}) {
  const { t } = await getServerTranslations();
  const params = await searchParams;
  const provider = (typeof params.provider === "string" ? params.provider : "sc_trending_bd");

  const [libraries, trending, prowlarrConfigured] = await Promise.all([
    fetchLibraries().catch(() => []),
    apiFetch<DiscoverySuggestion[]>(`/discovery/trending?provider=${provider}&limit=100`).catch(() => []),
    apiFetch<{ api_key?: string }>("/settings/prowlarr")
      .then(d => !!(d?.api_key?.trim()))
      .catch(() => false),
  ]);

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <Icon name="search" size="xl" className="text-rose-500" />
        <h1 className="text-3xl font-bold text-foreground">{t("discovery.title")}</h1>
      </div>

      <DiscoveryGrid
        initialSuggestions={trending}
        libraries={libraries}
        provider={provider}
        prowlarrConfigured={prowlarrConfigured}
      />
    </div>
  );
}
