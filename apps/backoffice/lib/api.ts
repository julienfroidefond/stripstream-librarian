export type LibraryDto = {
  id: string;
  name: string;
  root_path: string;
  enabled: boolean;
  book_count: number;
};

export type IndexJobDto = {
  id: string;
  library_id: string | null;
  type: string;
  status: string;
  started_at: string | null;
  finished_at: string | null;
  error_opt: string | null;
  created_at: string;
};

export type TokenDto = {
  id: string;
  name: string;
  scope: string;
  prefix: string;
  revoked_at: string | null;
};

export type FolderItem = {
  name: string;
  path: string;
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

export async function fetchLibraries() {
  return apiFetch<LibraryDto[]>("/libraries");
}

export async function createLibrary(name: string, rootPath: string) {
  return apiFetch<LibraryDto>("/libraries", {
    method: "POST",
    body: JSON.stringify({ name, root_path: rootPath })
  });
}

export async function deleteLibrary(id: string) {
  return apiFetch<void>(`/libraries/${id}`, { method: "DELETE" });
}

export async function listJobs() {
  return apiFetch<IndexJobDto[]>("/index/status");
}

export async function rebuildIndex(libraryId?: string) {
  const body = libraryId ? { library_id: libraryId } : {};
  return apiFetch<IndexJobDto>("/index/rebuild", {
    method: "POST",
    body: JSON.stringify(body)
  });
}

export async function cancelJob(id: string) {
  return apiFetch<IndexJobDto>(`/index/cancel/${id}`, { method: "POST" });
}

export async function listFolders() {
  return apiFetch<FolderItem[]>("/folders");
}

export async function listTokens() {
  return apiFetch<TokenDto[]>("/admin/tokens");
}

export async function createToken(name: string, scope: string) {
  return apiFetch<{ token: string }>("/admin/tokens", {
    method: "POST",
    body: JSON.stringify({ name, scope })
  });
}

export async function revokeToken(id: string) {
  return apiFetch<void>(`/admin/tokens/${id}`, { method: "DELETE" });
}
