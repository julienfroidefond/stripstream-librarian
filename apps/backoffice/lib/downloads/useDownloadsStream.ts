"use client";

import { useState } from "react";

import { useEventSource } from "@/lib/hooks/useEventSource";
import {
  DOWNLOADS_STREAM_URL,
  type DownloadsStreamPayload,
} from "./stream";

const EMPTY_PAYLOAD: DownloadsStreamPayload = {
  torrent: [],
  telegram: [],
  ddl: [],
};

/**
 * Subscribe to the unified downloads SSE stream.
 *
 * The three source lists are kept separately so the page can refresh them
 * independently, while every consumer sees the same payload contract.
 */
export function useDownloadsStream(
  initial: DownloadsStreamPayload = EMPTY_PAYLOAD,
) {
  const [torrent, setTorrent] = useState(initial.torrent);
  const [telegram, setTelegram] = useState(initial.telegram);
  const [ddl, setDdl] = useState(initial.ddl);

  useEventSource<DownloadsStreamPayload>({
    url: DOWNLOADS_STREAM_URL,
    onMessage: payload => {
      setTorrent(payload.torrent);
      setTelegram(payload.telegram);
      setDdl(payload.ddl);
    },
  });

  return {
    torrent,
    telegram,
    ddl,
    setTorrent,
    setTelegram,
    setDdl,
  };
}
