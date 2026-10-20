import { revalidateTag } from "next/cache";

import { READING_LISTS_TAG } from "./api/reading-lists";

/** Expire every cached fetch that reads reading-list data. */
export function revalidateReadingLists(): void {
  revalidateTag(READING_LISTS_TAG, { expire: 0 });
}
