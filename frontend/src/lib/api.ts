import { writable } from 'svelte/store';
import type {
  BlobResponse,
  AppSettings,
  CommitDetail,
  CommitInfo,
  LanguageStat,
  ReadmeResponse,
  RefsResponse,
  RepoInfo,
  TreeEntry
} from '$lib/types';

type HttpMethod = 'GET' | 'POST' | 'DELETE';
type ToastType = 'error' | 'success' | 'info';

export interface ToastMessage {
  id: string;
  message: string;
  type: ToastType;
}

const envApiBase =
  typeof import.meta !== 'undefined' && import.meta.env
    ? (import.meta.env.PUBLIC_API_URL as string | undefined)
    : undefined;
const apiBase = envApiBase || 'http://localhost:3001';

export const apiLoading = writable(false);
export const toasts = writable<ToastMessage[]>([]);
const responseCache = new Map<string, { expiresAt: number; value: unknown }>();
const inFlightGetRequests = new Map<string, Promise<unknown>>();

function toast(message: string, type: ToastType = 'info') {
  const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  toasts.update((current) => [...current, { id, message, type }]);
  setTimeout(() => {
    toasts.update((current) => current.filter((item) => item.id !== id));
  }, 3500);
}

async function request<T>(
  path: string,
  options: {
    method?: HttpMethod;
    body?: unknown;
    notifyOnError?: boolean;
    cacheTtlMs?: number;
    forceRefresh?: boolean;
    timeoutMs?: number;
  } = {}
): Promise<T> {
  const method = options.method ?? 'GET';
  const timeoutMs = options.timeoutMs ?? 30_000;
  const cacheTtlMs = options.cacheTtlMs ?? 0;
  const shouldCache = method === 'GET' && cacheTtlMs > 0;
  const cacheKey = shouldCache ? `${method}:${path}` : '';

  if (shouldCache && !options.forceRefresh) {
    const cached = responseCache.get(cacheKey);
    if (cached && cached.expiresAt > Date.now()) {
      return cached.value as T;
    }
    if (cached) {
      responseCache.delete(cacheKey);
    }

    const inFlight = inFlightGetRequests.get(cacheKey);
    if (inFlight) {
      return (await inFlight) as T;
    }
  }

  const execute = async (): Promise<T> => {
    apiLoading.set(true);
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), timeoutMs);
    try {
      let response: Response;
      try {
        response = await fetch(`${apiBase}/api${path}`, {
          method,
          headers: options.body ? { 'Content-Type': 'application/json' } : undefined,
          body: options.body ? JSON.stringify(options.body) : undefined,
          signal: controller.signal
        });
      } catch (error) {
        const isTimeout =
          error instanceof DOMException && error.name === 'AbortError';
        const message = isTimeout
          ? `Request timed out after ${Math.round(timeoutMs / 1000)}s.`
          : error instanceof Error
            ? error.message
            : 'Network request failed.';
        if (options.notifyOnError ?? true) {
          toast(message, 'error');
        }
        throw new Error(message);
      }

      if (response.ok === false) {
        const payload = (await response.json().catch(() => null)) as
          | { error?: string; code?: string }
          | null;
        const message = payload?.error || `Request failed with status ${response.status}`;
        if (options.notifyOnError ?? true) {
          toast(message, 'error');
        }
        throw new Error(message);
      }

      if (response.status === 204) {
        if (method !== 'GET') {
          responseCache.clear();
        }
        return undefined as T;
      }

      const payload = (await response.json()) as T;
      if (method !== 'GET') {
        responseCache.clear();
      }
      return payload;
    } finally {
      clearTimeout(timeoutId);
      apiLoading.set(false);
    }
  };

  const requestPromise = execute();
  if (shouldCache) {
    inFlightGetRequests.set(cacheKey, requestPromise as Promise<unknown>);
  }

  try {
    const payload = await requestPromise;
    if (shouldCache) {
      responseCache.set(cacheKey, {
        expiresAt: Date.now() + cacheTtlMs,
        value: payload
      });
    }
    return payload;
  } finally {
    if (shouldCache) {
      inFlightGetRequests.delete(cacheKey);
    }
  }
}

export const api = {
  notify: toast,

  getSettings() {
    return request<AppSettings>('/settings', { cacheTtlMs: 60_000 });
  },

  listRepos() {
    return request<RepoInfo[]>('/repos', { cacheTtlMs: 15_000 });
  },

  addRepo(url: string, name?: string) {
    return request<RepoInfo>('/repos', {
      method: 'POST',
      body: { url, name }
    });
  },

  deleteRepo(name: string) {
    return request<void>(`/repos/${encodeURIComponent(name)}`, {
      method: 'DELETE'
    });
  },

  fetchRepo(name: string) {
    return request<RepoInfo>(`/repos/${encodeURIComponent(name)}/fetch`, {
      method: 'POST'
    });
  },

  getRefs(name: string) {
    return request<RefsResponse>(`/repos/${encodeURIComponent(name)}/refs`);
  },

  getLanguages(name: string, refName: string) {
    const query = new URLSearchParams({ ref: refName });
    return request<LanguageStat[]>(
      `/repos/${encodeURIComponent(name)}/languages?${query.toString()}`,
      {
        cacheTtlMs: 60_000
      }
    );
  },

  getTree(name: string, refName: string, path = '') {
    const query = new URLSearchParams({ ref: refName, path });
    return request<TreeEntry[]>(`/repos/${encodeURIComponent(name)}/tree?${query.toString()}`, {
      cacheTtlMs: 60_000
    });
  },

  getBlob(name: string, refName: string, path: string) {
    const query = new URLSearchParams({ ref: refName, path });
    return request<BlobResponse>(`/repos/${encodeURIComponent(name)}/blob?${query.toString()}`, {
      cacheTtlMs: 60_000
    });
  },

  getReadme(name: string, refName: string) {
    const query = new URLSearchParams({ ref: refName });
    return request<ReadmeResponse>(`/repos/${encodeURIComponent(name)}/readme?${query.toString()}`, {
      cacheTtlMs: 120_000
    });
  },

  getCommits(name: string, refName: string, options?: { path?: string; skip?: number; limit?: number }) {
    const query = new URLSearchParams({
      ref: refName,
      path: options?.path || '',
      skip: String(options?.skip ?? 0),
      limit: String(options?.limit ?? 30)
    });
    return request<CommitInfo[]>(`/repos/${encodeURIComponent(name)}/commits?${query.toString()}`, {
      cacheTtlMs: 30_000
    });
  },

  getCommit(name: string, hash: string) {
    return request<CommitDetail>(`/repos/${encodeURIComponent(name)}/commit/${encodeURIComponent(hash)}`, {
      cacheTtlMs: 120_000
    });
  },

  archiveUrl(name: string, refName: string, format: 'tar.gz' | 'zip') {
    const query = new URLSearchParams({ ref: refName, format });
    return `${apiBase}/api/repos/${encodeURIComponent(name)}/archive?${query.toString()}`;
  },

  rawUrl(name: string, refName: string, path: string) {
    const query = new URLSearchParams({ ref: refName, path });
    return `${apiBase}/api/repos/${encodeURIComponent(name)}/raw?${query.toString()}`;
  }
};
