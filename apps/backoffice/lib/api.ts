export type LibraryDto = {
  id: string;
  name: string;
  root_path: string;
  enabled: boolean;
};

export type IndexJobDto = {
  id: string;
  type: string;
  status: string;
  created_at: string;
};

export type TokenDto = {
  id: string;
  name: string;
  scope: string;
  prefix: string;
  revoked_at: string | null;
};

function config() {
  const baseUrl = process.env.API_BASE_URL || "http://api:8080";
  const token = process.env.API_BOOTSTRAP_TOKEN;
  if (!token) {
    throw new Error("API_BOOTSTRAP_TOKEN is required for backoffice");
  }
  return { baseUrl: baseUrl.replace(/\/$/, ""), token };
}

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const { baseUrl, token } = config();
  const headers = new Headers(init?.headers || {});
  headers.set("Authorization", `Bearer ${token}`);
  if (init?.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const res = await fetch(`${baseUrl}${path}`, {
    ...init,
    headers,
    cache: "no-store"
  });

  if (!res.ok) {
    const text = await res.text();
    throw new Error(`API ${path} failed (${res.status}): ${text}`);
  }

  if (res.status === 204) {
    return null as T;
  }
  return (await res.json()) as T;
}

export async function listLibraries() {
  return apiFetch<LibraryDto[]>("/libraries");
}

export async function listJobs() {
  return apiFetch<IndexJobDto[]>("/index/status");
}

export async function listTokens() {
  return apiFetch<TokenDto[]>("/admin/tokens");
}
