"use client";

import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { useRouter } from "next/navigation";

import { toast } from "@/app/components/ui/Toast";
import type { ReadingListDetailDto, ReadingListDto } from "@/lib/api";
import { browserApi, browserErrorMessage } from "@/lib/api/browser";
import { useTranslation } from "@/lib/i18n/context";
import {
  createReadingListsStore,
  type ReadingListOp,
  type ReadingListOutcome,
  type ReadingListsSnapshot,
  type ReadingListsStore,
} from "@/lib/readingLists/actions";

/** Per-call presentation of a failure; the policy itself is the store's. */
export interface ReadingListRunOptions {
  /** Toast copy for a failure (defaults to the upstream message). */
  failureMessage?: (outcome: ReadingListOutcome) => string;
}

export interface ReadingListsHandle {
  readonly lists: ReadingListsSnapshot["lists"];
  detail(listId: string): ReadingListDetailDto | null;
  pending(key: string): boolean;
  hydrate(data: { lists?: readonly ReadingListDto[]; detail?: ReadingListDetailDto }): void;
  run(op: ReadingListOp, options?: ReadingListRunOptions): Promise<ReadingListOutcome>;
  runAll(ops: readonly ReadingListOp[], options?: ReadingListRunOptions): Promise<ReadingListOutcome>;
}

type Store = ReadingListsStore<ReadingListRunOptions>;

const ReadingListsContext = createContext<Store | null>(null);

/**
 * One store wired to the toast (failures) and `router.refresh()` (successes:
 * server-rendered sections such as the series page's lists). The store's
 * state is the source of truth for the surface; the refresh never feeds it.
 */
function useCreateStore(initial: {
  lists?: readonly ReadingListDto[] | null;
  detail?: ReadingListDetailDto;
}): Store {
  const { t } = useTranslation();
  const router = useRouter();
  const wiring = useRef({ t, router });
  useEffect(() => {
    wiring.current = { t, router };
  }, [t, router]);

  const [store] = useState<Store>(() =>
    createReadingListsStore<ReadingListRunOptions>({
      api: browserApi.readingLists,
      lists: initial.lists ?? null,
      details: initial.detail ? [initial.detail] : [],
      onError: (error, outcome, meta) => {
        const message =
          meta?.failureMessage?.(outcome) ??
          browserErrorMessage(error, wiring.current.t("readingLists.writeError"));
        toast(message, "error");
      },
      onSuccess: () => wiring.current.router.refresh(),
    }),
  );
  // No dispose on unmount: the store holds no timer, React drops its
  // subscription, and disposing would break the StrictMode remount.
  return store;
}

/** Shares one reading-lists store with every surface below it. */
export function ReadingListsProvider({
  lists,
  detail,
  children,
}: {
  lists?: readonly ReadingListDto[];
  detail?: ReadingListDetailDto;
  children: ReactNode;
}) {
  const store = useCreateStore({ lists, detail });
  return <ReadingListsContext.Provider value={store}>{children}</ReadingListsContext.Provider>;
}

/**
 * The reading-lists write seam for a component. Inside a
 * `ReadingListsProvider` it shares the page's store; elsewhere (the series
 * "add to list" modal) it owns a store of its own, hydrated by the caller.
 */
export function useReadingLists(): ReadingListsHandle {
  const shared = useContext(ReadingListsContext);
  const local = useCreateStore({});
  const store = shared ?? local;
  const state = useSyncExternalStore(store.subscribe, store.getState, store.getState);

  return {
    lists: state.lists,
    detail: (listId) => state.details[listId] ?? null,
    pending: (key) => state.pending.has(key),
    hydrate: store.hydrate,
    run: (op, options) => store.run(op, options),
    runAll: (ops, options) => store.runAll(ops, options),
  };
}
