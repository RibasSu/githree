import { PUBLIC_API_URL } from '$env/static/public';
import { writable } from 'svelte/store';
import type {
  BlobResponse,
  CommitDetail,
  CommitInfo,
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

const apiBase = PUBLIC_API_URL || 'http://localhost:3001';

export const apiLoading = writable(false);
export const toasts = writable<ToastMessage[]>([]);

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
  } = {}
): Promise<T> {
  const method = options.method ?? 'GET';
  apiLoading.set(true);
  try {
    const response = await fetch(`${apiBase}/api${path}`, {
      method,
      headers: options.body ? { 'Content-Type': 'application/json' } : undefined,
      body: options.body ? JSON.stringify(options.body) : undefined
    });

    if (response.ok === false) {
      const payload = (await response.json().catch(() => null)) as { error?: string; code?: string } | null;
      const message = payload?.error || `Request failed with status ${response.status}`;
      if (options.notifyOnError ?? true) {
        toast(message, 'error');
      }
      throw new Error(message);
    }

    if (response.status === 204) {
      return undefined as T;
    }

    return (await response.json()) as T;
  } finally {
    apiLoading.set(false);
  }
}

export const api = {
  notify: toast,

  listRepos() {
    return request<RepoInfo[]>('/repos');
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

  getTree(name: string, refName: string, path = '') {
    const query = new URLSearchParams({ ref: refName, path });
    return request<TreeEntry[]>(`/repos/${encodeURIComponent(name)}/tree?${query.toString()}`);
  },

  getBlob(name: string, refName: string, path: string) {
    const query = new URLSearchParams({ ref: refName, path });
    return request<BlobResponse>(`/repos/${encodeURIComponent(name)}/blob?${query.toString()}`);
  },

  getReadme(name: string, refName: string) {
    const query = new URLSearchParams({ ref: refName });
    return request<ReadmeResponse>(`/repos/${encodeURIComponent(name)}/readme?${query.toString()}`);
  },

  getCommits(name: string, refName: string, options?: { path?: string; skip?: number; limit?: number }) {
    const query = new URLSearchParams({
      ref: refName,
      path: options?.path || '',
      skip: String(options?.skip ?? 0),
      limit: String(options?.limit ?? 30)
    });
    return request<CommitInfo[]>(`/repos/${encodeURIComponent(name)}/commits?${query.toString()}`);
  },

  getCommit(name: string, hash: string) {
    return request<CommitDetail>(`/repos/${encodeURIComponent(name)}/commit/${encodeURIComponent(hash)}`);
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
