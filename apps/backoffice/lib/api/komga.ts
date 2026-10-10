/**
 * Synchronisation Komga.
 */

import { apiFetch } from "./client";

export type KomgaSyncRequest = {
  url: string;
  username: string;
  password: string;
  user_id: string;
};

export type KomgaSyncResponse = {
  id: string;
  komga_url: string;
  user_id?: string;
  total_komga_read: number;
  matched: number;
  already_read: number;
  newly_marked: number;
  matched_books: string[];
  newly_marked_books: string[];
  unmatched: string[];
  created_at: string;
};

export type KomgaSyncReportSummary = {
  id: string;
  komga_url: string;
  user_id?: string;
  total_komga_read: number;
  matched: number;
  already_read: number;
  newly_marked: number;
  unmatched_count: number;
  created_at: string;
};

export async function syncKomga(req: KomgaSyncRequest) {
  return apiFetch<KomgaSyncResponse>("/komga/sync", {
    method: "POST",
    body: JSON.stringify(req),
  });
}

export async function listKomgaReports() {
  return apiFetch<KomgaSyncReportSummary[]>("/komga/reports");
}

export async function getKomgaReport(id: string) {
  return apiFetch<KomgaSyncResponse>(`/komga/reports/${id}`);
}
