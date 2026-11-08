import { revalidatePath, revalidateTag } from "next/cache";

import { READING_LISTS_TAG } from "./api/reading-lists";

/** Expire every cached fetch that reads reading-list data. */
export function revalidateReadingLists(): void {
  revalidateTag(READING_LISTS_TAG, { expire: 0 });
  // `revalidateTag` alone can leave the already-rendered grouped Series page
  // serving its stale fetch payload on the next request in dev.
  revalidatePath("/series");
  revalidatePath("/reading-lists");
}
