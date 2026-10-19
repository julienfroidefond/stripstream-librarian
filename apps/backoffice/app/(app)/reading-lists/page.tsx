import { cookies } from "next/headers";
import { fetchReadingLists } from "@/lib/api";
import { ReadingListsClient } from "./ReadingListsClient";

export const dynamic = "force-dynamic";

export default async function ReadingListsPage() {
  const cookieStore = await cookies();
  const hasActiveUser = !!cookieStore.get("as_user_id")?.value;
  const lists = await fetchReadingLists().catch(() => []);
  return <ReadingListsClient initialLists={lists} hasActiveUser={hasActiveUser} />;
}
