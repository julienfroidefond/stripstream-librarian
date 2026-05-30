import { fetchReadingLists } from "@/lib/api";
import { ReadingListsClient } from "./ReadingListsClient";

export const dynamic = "force-dynamic";

export default async function ReadingListsPage() {
  const lists = await fetchReadingLists().catch(() => []);
  return <ReadingListsClient initialLists={lists} />;
}
