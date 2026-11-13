import { redirect } from "next/navigation";
import { loadSettingsPage } from "@/lib/settings/page";
import SettingsPage from "./SettingsPage";
import { TokensTab } from "./components/TokensTab";

export const dynamic = "force-dynamic";

export default async function SettingsPageWrapper({ searchParams }: { searchParams: Promise<{ tab?: string; created?: string }> }) {
  const { tab, created: createdToken } = await searchParams;
  // The reading overview moved out of Settings into its own top-level page.
  if (tab === "readingOverview") redirect("/progress");

  // Every fetch and its degradation policy live in the loader.
  const model = await loadSettingsPage();

  return (
    <SettingsPage
      model={model}
      initialTab={tab}
      tokensContent={<TokensTab createdToken={createdToken} />}
    />
  );
}
