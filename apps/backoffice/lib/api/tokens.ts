/**
 * Tokens API.
 */

import { apiFetch } from "./client";

export type TokenDto = {
  id: string;
  name: string;
  scope: string;
  prefix: string;
  revoked_at: string | null;
  user_id?: string;
  username?: string;
};

export async function listTokens() {
  return apiFetch<TokenDto[]>("/admin/tokens");
}

export async function createToken(name: string, scope: string, userId?: string) {
  return apiFetch<{ token: string }>("/admin/tokens", {
    method: "POST",
    body: JSON.stringify({ name, scope, ...(userId ? { user_id: userId } : {}) }),
  });
}

export async function revokeToken(id: string) {
  return apiFetch<void>(`/admin/tokens/${id}`, { method: "DELETE" });
}

export async function deleteToken(id: string) {
  return apiFetch<void>(`/admin/tokens/${id}/delete`, { method: "POST" });
}

export async function updateToken(id: string, userId: string | null) {
  return apiFetch<void>(`/admin/tokens/${id}`, {
    method: "PATCH",
    body: JSON.stringify({ user_id: userId || null }),
  });
}
