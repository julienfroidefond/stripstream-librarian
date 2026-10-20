import { fetchReadingLists } from "@/lib/api";
import { getActiveUser } from "@/lib/activeUser";
import { ReadingListsClient } from "./ReadingListsClient";

export const dynamic = "force-dynamic";

export default async function ReadingListsPage() {
  const { hasActiveUser } = await getActiveUser();
  const lists = await fetchReadingLists().catch(() => []);
  return <ReadingListsClient initialLists={lists} hasActiveUser={hasActiveUser} />;
}
