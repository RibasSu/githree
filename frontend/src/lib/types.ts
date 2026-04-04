export type RepoSource = 'github' | 'gitlab' | 'generic';

export interface RepoInfo {
  name: string;
  url: string;
  description: string | null;
  default_branch: string;
  last_fetched: string | null;
  size_kb: number;
  source: RepoSource | string;
}

export interface RefsResponse {
  branches: string[];
  tags: string[];
  default_branch: string;
}

export interface LanguageStat {
  language: string;
  bytes: number;
  percentage: number;
}

export interface TreeEntry {
  name: string;
  path: string;
  entry_type: 'blob' | 'tree';
  oid: string;
  size: number | null;
  mode: number;
  last_commit: CommitInfo | null;
}

export interface BlobResponse {
  content: string;
  encoding: 'utf8' | 'base64' | string;
  size: number;
  language: string;
  is_binary: boolean;
  mime?: string | null;
  is_truncated: boolean;
  truncated_reason?: string | null;
}

export interface ReadmeResponse {
  content: string;
  filename: string;
  path: string;
}

export interface CommitInfo {
  hash: string;
  short_hash: string;
  author_name: string;
  author_email: string;
  authored_at: string;
  message: string;
  message_short: string;
}

export interface CommitCountResponse {
  count: number;
}

export interface DiffStats {
  files_changed: number;
  insertions: number;
  deletions: number;
}

export interface DiffLine {
  old_lineno: number | null;
  new_lineno: number | null;
  content: string;
  line_type: 'add' | 'delete' | 'context' | 'meta' | string;
}

export interface DiffHunk {
  header: string;
  lines: DiffLine[];
}

export interface FileDiff {
  old_path: string | null;
  new_path: string | null;
  status: 'added' | 'deleted' | 'modified' | 'renamed' | string;
  hunks: DiffHunk[];
  is_binary: boolean;
}

export interface CommitDetail {
  commit: CommitInfo;
  parents: string[];
  stats: DiffStats;
  diffs: FileDiff[];
  is_truncated: boolean;
  truncated_reason?: string | null;
  displayed_file_count: number;
  displayed_line_count: number;
}

export interface ApiError {
  error: string;
  code: string;
}

export interface AppSettings {
  web_repo_management: boolean;
  show_repo_controls?: boolean;
  repos_dir: string;
  registry_file: string;
  app_name?: string;
  logo_url?: string;
  site_url?: string;
  domain?: string;
  caddy_enabled?: boolean;
}
