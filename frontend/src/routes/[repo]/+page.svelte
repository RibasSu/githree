<script lang="ts">
  import { goto } from '$app/navigation';
  import BranchSelector from '$lib/components/BranchSelector.svelte';
  import FileTree from '$lib/components/FileTree.svelte';
  import SourceLogo from '$lib/components/SourceLogo.svelte';
  import { api } from '$lib/api';
  import { highlightMarkdownCodeBlocks } from '$lib/markdown';
  import { formatDateTime } from '$lib/time';
  import type {
    CommitInfo,
    LanguageStat,
    ReadmeResponse,
    RefsResponse,
    RepoInfo,
    TreeEntry
  } from '$lib/types';
  import { BookOpen, ChevronDown, Code2, Copy, FileText, Folder, GitBranch, Scale, Search, Shield, Tag, Users } from 'lucide-svelte';
  import DOMPurify from 'dompurify';
  import { marked } from 'marked';
  import { onMount } from 'svelte';
  import ShimmerRows from '$lib/components/ShimmerRows.svelte';

  interface PageData {
    repo: string;
    refName: string;
  }

  interface Props {
    data: PageData;
  }

  interface FileSearchEntry {
    path: string;
    entryType: 'blob' | 'tree';
  }

  let { data }: Props = $props();
  let repo = $state<RepoInfo | null>(null);
  let refs = $state<RefsResponse | null>(null);
  let tree = $state<TreeEntry[]>([]);
  let readme = $state<ReadmeResponse | null>(null);
  let readmeHtml = $state('');
  let recentCommits = $state<CommitInfo[]>([]);
  let languageStats = $state<LanguageStat[]>([]);
  let totalCommitCount = $state<number | null>(null);
  let selectedRef = $state('');
  let goToFilePath = $state('');
  let goToFileFocused = $state(false);
  let fileSearchEntries = $state<FileSearchEntry[]>([]);
  let fileSearchLoading = $state(false);
  let fileSearchActiveIndex = $state(0);
  let fileSearchBuildToken = 0;
  let codeMenuOpen = $state(false);
  let cloneTab = $state<'https' | 'ssh' | 'cli'>('https');
  let loading = $state(true);
  let codeMenuRoot = $state<HTMLElement | null>(null);

  onMount(() => {
    void bootstrap();
    const handlePointerDown = (event: PointerEvent) => {
      if (!codeMenuOpen) return;
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (codeMenuRoot?.contains(target)) return;
      codeMenuOpen = false;
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      codeMenuOpen = false;
    };

    document.addEventListener('pointerdown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);

    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  });

  $effect(() => {
    if (!isGithubRepo && cloneTab === 'cli') {
      cloneTab = 'https';
    }
  });

  $effect(() => {
    goToFilePath;
    selectedRef;
    fileSearchActiveIndex = 0;
  });

  async function bootstrap() {
    loading = true;
    try {
      const all = await api.listRepos();
      repo = all.find((item) => item.name === data.repo) ?? null;
      if (repo === null) {
        refs = null;
        tree = [];
        readme = null;
        readmeHtml = '';
        recentCommits = [];
        languageStats = [];
        totalCommitCount = null;
        return;
      }
      if (selectedRef.length === 0) {
        selectedRef = data.refName || repo?.default_branch || 'main';
      }
      await loadForRef();
    } finally {
      loading = false;
    }
  }

  async function loadForRef() {
    if (selectedRef.length === 0) return;
    const requestedRef = selectedRef;
    try {
      const nextRefs = await api.getRefs(data.repo);
      refs = nextRefs;

      let effectiveRef = requestedRef;
      const hasEffectiveRef =
        nextRefs.branches.includes(effectiveRef) || nextRefs.tags.includes(effectiveRef);
      if (!hasEffectiveRef) {
        effectiveRef =
          nextRefs.default_branch || nextRefs.branches[0] || nextRefs.tags[0] || requestedRef;
      }

      if (effectiveRef !== selectedRef) {
        selectedRef = effectiveRef;
        await goto(`/${data.repo}?ref=${encodeURIComponent(effectiveRef)}`, {
          replaceState: true,
          noScroll: true
        });
      }

      const [nextTree, nextReadme, commits, languages, commitCount, refreshedRepos] = await Promise.all([
        api.getTree(data.repo, effectiveRef, ''),
        api.getReadme(data.repo, effectiveRef).catch(() => null),
        api.getCommits(data.repo, effectiveRef, { limit: 30 }),
        api.getLanguages(data.repo, effectiveRef).catch(() => []),
        api.getCommitCount(data.repo, effectiveRef).catch(() => null),
        api.listRepos(true).catch(() => null)
      ]);
      if (selectedRef !== effectiveRef) return;
      tree = nextTree;
      readme = nextReadme;
      recentCommits = commits;
      languageStats = languages;
      totalCommitCount = commitCount?.count ?? null;
      if (Array.isArray(refreshedRepos)) {
        const refreshed = refreshedRepos.find((item) => item.name === data.repo);
        if (refreshed) {
          repo = refreshed;
        }
      }
      fileSearchEntries = [];
      fileSearchActiveIndex = 0;
      await renderReadme();
      void buildFileSearchIndex(effectiveRef);
    } catch {
      // toast already emitted
    }
  }

  async function renderReadme() {
    if (!readme) {
      readmeHtml = '';
      return;
    }
    const rendered = await marked.parse(readme.content);
    const rewritten = rewriteReadmeLinks(rendered, data.repo, selectedRef || 'main', readme.path);
    const highlighted = await highlightMarkdownCodeBlocks(rewritten);
    readmeHtml = DOMPurify.sanitize(highlighted);
  }

  async function changeRef(value: string) {
    if (value === selectedRef) return;
    selectedRef = value;
    goToFilePath = '';
    goToFileFocused = false;
    await goto(`/${data.repo}?ref=${encodeURIComponent(value)}`, { replaceState: true, noScroll: true });
    await loadForRef();
  }

  function sshCloneCommand(url: string): string {
    if (url.startsWith('git@')) return url;
    try {
      const parsed = new URL(url);
      const project = parsed.pathname.replace(/^\//, '').replace(/\.git$/, '');
      return `git@${parsed.host}:${project}.git`;
    } catch {
      return url;
    }
  }

  function githubCliCloneCommand(url: string): string {
    if (url.startsWith('git@')) {
      const match = /^git@([^:]+):(.+?)(?:\.git)?$/i.exec(url.trim());
      if (match && match[1].toLowerCase() === 'github.com') {
        return `gh repo clone ${match[2]}`;
      }
      return `git clone ${url}`;
    }

    try {
      const parsed = new URL(url);
      if (parsed.host.toLowerCase() !== 'github.com') return `git clone ${url}`;
      const project = parsed.pathname.replace(/^\/+/, '').replace(/\.git$/i, '');
      return project.length > 0 ? `gh repo clone ${project}` : `git clone ${url}`;
    } catch {
      return `git clone ${url}`;
    }
  }

  async function copyToClipboard(text: string, successMessage: string) {
    try {
      await navigator.clipboard.writeText(text);
      api.notify(successMessage, 'success');
    } catch {
      api.notify('Could not copy to clipboard.', 'error');
    }
  }

  async function submitGoToFile(event: SubmitEvent) {
    event.preventDefault();
    const candidate = goToFilePath.trim().replace(/^\/+/, '');
    const exact = fileSearchEntries.find(
      (entry) => entry.path.toLowerCase() === candidate.toLowerCase()
    );
    const fallback = fileSearchResults[fileSearchActiveIndex] || fileSearchResults[0];
    const target = exact || fallback;
    if (!target) return;

    await navigateToSearchEntry(target);
  }

  async function handleGoToFileKeydown(event: KeyboardEvent) {
    if (!showFileSearchDropdown) return;
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      fileSearchActiveIndex = Math.min(fileSearchActiveIndex + 1, fileSearchResults.length - 1);
      return;
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      fileSearchActiveIndex = Math.max(fileSearchActiveIndex - 1, 0);
      return;
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      goToFileFocused = false;
    }
  }

  async function navigateToSearchEntry(entry: FileSearchEntry) {
    const targetMode = entry.entryType === 'tree' ? 'tree' : 'blob';
    await goto(
      `/${data.repo}/${targetMode}/${encodeRepoPath(entry.path)}?ref=${encodeURIComponent(selectedRef)}`
    );
    goToFilePath = '';
    goToFileFocused = false;
  }

  async function buildFileSearchIndex(refName: string) {
    const buildToken = ++fileSearchBuildToken;
    fileSearchLoading = true;
    fileSearchEntries = [];
    fileSearchActiveIndex = 0;
    try {
      const queue: string[] = [''];
      const visited = new Set<string>();
      const nextEntries: FileSearchEntry[] = [];
      const MAX_ENTRIES = 6000;
      const MAX_DIRECTORIES = 1200;
      let traversedDirectories = 0;

      while (
        queue.length > 0 &&
        nextEntries.length < MAX_ENTRIES &&
        traversedDirectories < MAX_DIRECTORIES
      ) {
        const currentPath = queue.shift() ?? '';
        if (visited.has(currentPath)) continue;
        visited.add(currentPath);
        traversedDirectories += 1;

        const entries = await api.getTree(data.repo, refName, currentPath);
        if (buildToken !== fileSearchBuildToken) return;

        for (const entry of entries) {
          nextEntries.push({
            path: entry.path,
            entryType: entry.entry_type === 'tree' ? 'tree' : 'blob'
          });

          if (entry.entry_type === 'tree' && queue.length < MAX_ENTRIES) {
            queue.push(entry.path);
          }
        }
      }

      if (buildToken !== fileSearchBuildToken) return;

      nextEntries.sort((left, right) => left.path.localeCompare(right.path));
      fileSearchEntries = nextEntries;
    } catch {
      // toast already emitted by API client
    } finally {
      if (buildToken === fileSearchBuildToken) {
        fileSearchLoading = false;
      }
    }
  }

  function scoreSearchEntry(entry: FileSearchEntry, query: string): number {
    const target = entry.path.toLowerCase();
    if (target === query) return 0;
    if (target.startsWith(query)) return 1;
    if (target.includes(`/${query}`)) return 2;
    return 3;
  }

  const archiveTarUrl = $derived(api.archiveUrl(data.repo, selectedRef || 'main', 'tar.gz'));
  const archiveZipUrl = $derived(api.archiveUrl(data.repo, selectedRef || 'main', 'zip'));
  const isGithubRepo = $derived.by(() => {
    const source = repo?.source?.toString().toLowerCase();
    if (source === 'github') return true;
    return repo?.url.toLowerCase().includes('github.com') ?? false;
  });
  const isGitlabRepo = $derived.by(() => {
    const source = repo?.source?.toString().toLowerCase();
    if (source === 'gitlab') return true;
    return repo?.url.toLowerCase().includes('gitlab.') ?? false;
  });
  const remoteCoordinates = $derived.by(() => {
    if (!repo) {
      return { namespace: '', repositoryName: data.repo };
    }
    return extractRemoteCoordinates(repo.url, repo.name);
  });
  const remoteLinks = $derived.by(() => {
    if (!repo) {
      return { namespaceHref: '', repositoryHref: '' };
    }
    return buildRemoteLinks(repo.url, remoteCoordinates.namespace, remoteCoordinates.repositoryName);
  });
  const activeCloneCommand = $derived.by(() => {
    if (!repo) return '';
    if (cloneTab === 'ssh') return sshCloneCommand(repo.url);
    if (cloneTab === 'cli' && isGithubRepo) return githubCliCloneCommand(repo.url);
    return repo.url;
  });
  const filteredDocTabs = $derived.by(() =>
    highlightedDocs.filter((entry) => entry.name.toLowerCase() !== 'readme.md')
  );
  const highlightedDocs = $derived.by(() =>
    tree.filter((entry) =>
      ['readme.md', 'security.md', 'contributing.md', 'license', 'code_of_conduct.md'].includes(
        entry.name.toLowerCase()
      )
    )
  );
  const normalizedFileSearchQuery = $derived(goToFilePath.trim().toLowerCase());
  const fileSearchResults = $derived.by(() => {
    const query = normalizedFileSearchQuery;
    if (query.length === 0) return [] as FileSearchEntry[];

    const filtered = fileSearchEntries
      .filter((entry) => entry.path.toLowerCase().includes(query))
      .sort((left, right) => {
        const leftScore = scoreSearchEntry(left, query);
        const rightScore = scoreSearchEntry(right, query);
        if (leftScore !== rightScore) return leftScore - rightScore;
        return left.path.localeCompare(right.path);
      });
    return filtered.slice(0, 50);
  });
  const showFileSearchDropdown = $derived(
    goToFileFocused && (fileSearchLoading || fileSearchResults.length > 0 || normalizedFileSearchQuery.length > 0)
  );
  const visibleLanguageStats = $derived.by(() => {
    if (languageStats.length === 0) {
      return [] as LanguageStat[];
    }

    const mergedByLanguage = new Map<string, LanguageStat>();
    for (const stat of languageStats) {
      const key = stat.language.toLowerCase();
      const current = mergedByLanguage.get(key);
      if (current) {
        mergedByLanguage.set(key, {
          language: key,
          bytes: current.bytes + stat.bytes,
          percentage: current.percentage + stat.percentage
        });
      } else {
        mergedByLanguage.set(key, {
          language: key,
          bytes: stat.bytes,
          percentage: stat.percentage
        });
      }
    }

    const sorted = [...mergedByLanguage.values()].sort((left, right) => right.bytes - left.bytes);
    const topLimit = 6;
    const top = sorted.slice(0, topLimit);
    const remaining = sorted.slice(topLimit);

    if (remaining.length === 0) {
      return top;
    }

    const remainingBytes = remaining.reduce((total, item) => total + item.bytes, 0);
    const remainingPercentage = remaining.reduce((total, item) => total + item.percentage, 0);
    const otherIndex = top.findIndex((item) => item.language === 'other');
    if (otherIndex >= 0) {
      const updated = [...top];
      const current = updated[otherIndex];
      updated[otherIndex] = {
        language: 'other',
        bytes: current.bytes + remainingBytes,
        percentage: current.percentage + remainingPercentage
      };
      return updated;
    }

    return [...top, { language: 'other', bytes: remainingBytes, percentage: remainingPercentage }];
  });

  function languageColor(language: string): string {
    const key = language.toLowerCase();
    const colors: Record<string, string> = {
      rust: '#dea584',
      svelte: '#ff3e00',
      typescript: '#3178c6',
      javascript: '#f1e05a',
      tsx: '#3178c6',
      jsx: '#f1e05a',
      css: '#563d7c',
      html: '#e34c26',
      markdown: '#083fa1',
      python: '#3572a5',
      go: '#00add8',
      java: '#b07219',
      c: '#555555',
      cpp: '#f34b7d',
      bash: '#89e051',
      shell: '#89e051',
      docker: '#384d54',
      yaml: '#cb171e',
      json: '#292929',
      toml: '#9c4221',
      sql: '#e38c00',
      ruby: '#701516',
      php: '#4f5d95',
      swift: '#f05138',
      kotlin: '#a97bff',
      dart: '#00b4ab',
      xml: '#0060ac',
      other: '#8b949e',
      text: '#8b949e'
    };
    return colors[key] ?? '#8b949e';
  }

  function languageLabel(language: string): string {
    const key = language.toLowerCase();
    const labels: Record<string, string> = {
      rust: 'Rust',
      svelte: 'Svelte',
      typescript: 'TypeScript',
      javascript: 'JavaScript',
      tsx: 'TSX',
      jsx: 'JSX',
      css: 'CSS',
      html: 'HTML',
      markdown: 'Markdown',
      python: 'Python',
      go: 'Go',
      java: 'Java',
      c: 'C',
      cpp: 'C++',
      bash: 'Shell',
      shell: 'Shell',
      docker: 'Dockerfile',
      yaml: 'YAML',
      json: 'JSON',
      toml: 'TOML',
      sql: 'SQL',
      ruby: 'Ruby',
      php: 'PHP',
      swift: 'Swift',
      kotlin: 'Kotlin',
      dart: 'Dart',
      xml: 'XML',
      other: 'Other',
      text: 'Other'
    };
    return labels[key] ?? language.charAt(0).toUpperCase() + language.slice(1);
  }

  function languagePercent(percentage: number): string {
    return `${Math.max(0, percentage).toFixed(1)}%`;
  }

  function rewriteReadmeLinks(
    html: string,
    repoName: string,
    refName: string,
    readmePath: string
  ): string {
    if (typeof window === 'undefined') return html;

    const document = new DOMParser().parseFromString(html, 'text/html');
    const anchors = document.querySelectorAll('a[href]');

    for (const anchor of anchors) {
      const href = anchor.getAttribute('href')?.trim();
      if (!href) continue;
      if (href.startsWith('#')) continue;
      if (isExternalHref(href)) continue;

      const [hrefWithoutHash, hashFragment] = href.split('#', 2);
      if (hrefWithoutHash.length === 0) continue;
      if (hrefWithoutHash.startsWith('?')) continue;

      const repoPath = resolveRepoRelativePath(hrefWithoutHash, readmePath);
      if (repoPath.length === 0) continue;

      const mode = inferRepoLinkMode(hrefWithoutHash);
      const encodedPath = encodeRepoPath(repoPath);
      const suffix = hashFragment ? `#${encodeURIComponent(hashFragment)}` : '';
      anchor.setAttribute(
        'href',
        `/${repoName}/${mode}/${encodedPath}?ref=${encodeURIComponent(refName)}${suffix}`
      );
    }

    return document.body.innerHTML;
  }

  function isExternalHref(href: string): boolean {
    return /^(https?:|mailto:|tel:|data:)/i.test(href);
  }

  function resolveRepoRelativePath(target: string, readmePath: string): string {
    const readmeDirectory = readmePath.includes('/') ? readmePath.slice(0, readmePath.lastIndexOf('/')) : '';
    const baseUrl = new URL(`https://repo.local/${readmeDirectory.length > 0 ? `${readmeDirectory}/` : ''}`);
    const resolved = new URL(target, baseUrl);
    return resolved.pathname.replace(/^\/+/, '').replace(/\/+$/, '');
  }

  function inferRepoLinkMode(target: string): 'blob' | 'tree' {
    if (target.endsWith('/')) return 'tree';
    return 'blob';
  }

  function encodeRepoPath(path: string): string {
    return path
      .split('/')
      .filter((segment) => segment.length > 0)
      .map((segment) => encodeURIComponent(segment))
      .join('/');
  }

  function docTabLabel(name: string): string {
    const normalized = name.toLowerCase();
    if (normalized === 'code_of_conduct.md') return 'Code of conduct';
    if (normalized === 'contributing.md') return 'Contributing';
    if (normalized === 'security.md') return 'Security';
    if (normalized === 'license') return 'MIT license';
    return name;
  }

  function extractRemoteCoordinates(url: string, fallbackName: string): {
    namespace: string;
    repositoryName: string;
  } {
    const input = url.trim();
    if (input.length === 0) {
      return { namespace: '', repositoryName: fallbackName };
    }

    if (isScpLikeSshUrl(input)) {
      const rawPath = input.split(':').slice(1).join(':');
      return parseCoordinatesFromPath(rawPath, fallbackName);
    }

    try {
      const parsed = new URL(input);
      return parseCoordinatesFromPath(parsed.pathname, fallbackName);
    } catch {
      return parseCoordinatesFromPath(input, fallbackName);
    }
  }

  function parseCoordinatesFromPath(path: string, fallbackName: string): {
    namespace: string;
    repositoryName: string;
  } {
    const cleaned = path.replace(/^\/+/, '').replace(/\/+$/, '').replace(/\.git$/i, '');
    const segments = cleaned.split('/').filter((segment) => segment.length > 0);
    if (segments.length === 0) {
      return { namespace: '', repositoryName: fallbackName };
    }

    const repositoryName = segments[segments.length - 1] || fallbackName;
    const namespace = segments.slice(0, -1).join('/');
    return { namespace, repositoryName };
  }

  function isScpLikeSshUrl(url: string): boolean {
    return /^[a-z0-9._-]+@[a-z0-9.-]+:[^:\s]+$/i.test(url);
  }

  function buildRemoteLinks(
    sourceUrl: string,
    namespace: string,
    repositoryName: string
  ): { namespaceHref: string; repositoryHref: string } {
    const trimmed = sourceUrl.trim();
    if (trimmed.length === 0) {
      return { namespaceHref: '', repositoryHref: '' };
    }

    const namespacePath = namespace.trim().replace(/^\/+|\/+$/g, '');
    const repoPath = [namespacePath, repositoryName]
      .filter((segment) => segment.length > 0)
      .join('/')
      .replace(/\.git$/i, '');

    const sshMatch = /^([a-z0-9._-]+)@([^:]+):(.+)$/i.exec(trimmed);
    if (sshMatch) {
      const host = sshMatch[2];
      const sshPath = sshMatch[3].replace(/^\/+|\/+$/g, '').replace(/\.git$/i, '');
      const effectiveRepoPath = repoPath.length > 0 ? repoPath : sshPath;
      return {
        namespaceHref: namespacePath.length > 0 ? `https://${host}/${namespacePath}` : '',
        repositoryHref: effectiveRepoPath.length > 0 ? `https://${host}/${effectiveRepoPath}` : ''
      };
    }

    try {
      const parsed = new URL(trimmed);
      const cleanedPath = parsed.pathname.replace(/^\/+|\/+$/g, '').replace(/\.git$/i, '');
      const effectiveRepoPath = repoPath.length > 0 ? repoPath : cleanedPath;
      return {
        namespaceHref: namespacePath.length > 0 ? `${parsed.origin}/${namespacePath}` : '',
        repositoryHref:
          effectiveRepoPath.length > 0 ? `${parsed.origin}/${effectiveRepoPath}` : parsed.href
      };
    } catch {
      return { namespaceHref: '', repositoryHref: trimmed };
    }
  }
</script>

{#if loading}
  <div class="space-y-3">
    <ShimmerRows rows={3} />
    <ShimmerRows rows={7} />
    <ShimmerRows rows={6} />
  </div>
{:else if repo === null}
  <div class="card-surface p-6 text-sm gt-muted">Repository "{data.repo}" was not found.</div>
{:else}
  <section class="space-y-4">
    <header class="flex flex-wrap items-center justify-between gap-3 border-b gt-divider pb-3">
      <div>
        <h1 class="flex items-center gap-2 text-2xl font-semibold text-[#f0f6fc]">
          <SourceLogo
            size={20}
            source={isGithubRepo ? 'github' : isGitlabRepo ? 'gitlab' : 'generic'}
          />

          {#if remoteCoordinates.namespace.length > 0}
            {#if remoteLinks.namespaceHref.length > 0}
              <a
                class="text-[#8b949e] hover:underline"
                href={remoteLinks.namespaceHref}
                rel="noopener noreferrer"
                target="_blank"
              >
                {remoteCoordinates.namespace}
              </a>
            {:else}
              <span class="text-[#8b949e]">{remoteCoordinates.namespace}</span>
            {/if}
            <span class="gt-muted">/</span>
          {/if}
          {#if remoteLinks.repositoryHref.length > 0}
            <a
              class="text-[#f0f6fc] hover:underline"
              href={remoteLinks.repositoryHref}
              rel="noopener noreferrer"
              target="_blank"
            >
              {remoteCoordinates.repositoryName}
            </a>
          {:else}
            <span>{remoteCoordinates.repositoryName}</span>
          {/if}
        </h1>
      </div>
      <div class="flex flex-wrap gap-2">
        <a class="btn btn-primary" href={`/${data.repo}/commits?ref=${encodeURIComponent(selectedRef)}`}>
          Commits
        </a>
      </div>
    </header>

    <div class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_300px]">
      <div class="space-y-6">
        <div class="card-surface overflow-visible">
          <div class="flex flex-wrap items-center justify-between gap-2 border-b gt-divider px-3 py-2">
            <div class="flex flex-wrap items-center gap-3">
              <BranchSelector compact onSelect={changeRef} refs={refs} repoName={data.repo} selected={selectedRef} />
              <span class="gt-toolbar-stat">
                <GitBranch size={14} />
                {#if (refs?.branches.length ?? 0) === 1}
                  1 Branch
                {:else}
                  {(refs?.branches.length ?? 0).toLocaleString()} Branches
                {/if}
              </span>
              <span class="gt-toolbar-stat">
                <Tag size={14} />
                {#if (refs?.tags.length ?? 0) === 1}
                  1 Tag
                {:else}
                  {(refs?.tags.length ?? 0).toLocaleString()} Tags
                {/if}
              </span>
            </div>

            <div class="flex flex-wrap items-center gap-2">
              <div class="relative">
                <form class="gt-go-to-file" onsubmit={submitGoToFile}>
                  <Search class="gt-muted" size={15} />
                  <input
                    bind:value={goToFilePath}
                    autocomplete="off"
                    onblur={() => {
                      setTimeout(() => {
                        goToFileFocused = false;
                      }, 80);
                    }}
                    onfocus={() => {
                      goToFileFocused = true;
                    }}
                    onkeydown={handleGoToFileKeydown}
                    placeholder="Go to file"
                    spellcheck="false"
                    type="text"
                  />
                </form>

                {#if showFileSearchDropdown}
                  <div class="gt-go-to-file-menu">
                    {#if fileSearchLoading && fileSearchResults.length === 0}
                      <div class="gt-go-to-file-empty">Indexing repository files...</div>
                    {:else if fileSearchResults.length === 0}
                      <div class="gt-go-to-file-empty">No matching files or folders.</div>
                    {:else}
                      <ul class="gt-go-to-file-items">
                        {#each fileSearchResults as result, index (result.path)}
                          <li>
                            <button
                              class={`gt-go-to-file-item ${index === fileSearchActiveIndex ? 'active' : ''}`}
                              onclick={() => navigateToSearchEntry(result)}
                              type="button"
                            >
                              {#if result.entryType === 'tree'}
                                <Folder aria-hidden="true" class="gt-muted" size={14} />
                              {:else}
                                <FileText aria-hidden="true" class="gt-muted" size={14} />
                              {/if}
                              <span class="truncate">{result.path}</span>
                            </button>
                          </li>
                        {/each}
                      </ul>
                    {/if}
                  </div>
                {/if}
              </div>

              <div bind:this={codeMenuRoot} class="relative">
                <button
                  aria-expanded={codeMenuOpen}
                  class="btn btn-code"
                  onclick={() => {
                    codeMenuOpen = !codeMenuOpen;
                  }}
                  type="button"
                >
                  <Code2 size={14} />
                  Code
                  <ChevronDown size={14} />
                </button>

                {#if codeMenuOpen}
                  <div class="gt-code-menu">
                    <div class="flex items-center gap-1 border-b gt-divider px-2">
                      <button
                        class={`gt-code-menu-tab ${cloneTab === 'https' ? 'active' : ''}`}
                        onclick={() => {
                          cloneTab = 'https';
                        }}
                        type="button"
                      >
                        HTTPS
                      </button>
                      <button
                        class={`gt-code-menu-tab ${cloneTab === 'ssh' ? 'active' : ''}`}
                        onclick={() => {
                          cloneTab = 'ssh';
                        }}
                        type="button"
                      >
                        SSH
                      </button>
                      {#if isGithubRepo}
                        <button
                          class={`gt-code-menu-tab ${cloneTab === 'cli' ? 'active' : ''}`}
                          onclick={() => {
                            cloneTab = 'cli';
                          }}
                          type="button"
                        >
                          GitHub CLI
                        </button>
                      {/if}
                    </div>

                    <div class="space-y-2 p-3">
                      <p class="text-xs font-semibold uppercase tracking-wide gt-muted">Clone</p>
                      <div class="flex items-center gap-2 rounded-md border gt-divider bg-[#161b22] p-2">
                        <code class="gt-clone-command" title={activeCloneCommand}>{activeCloneCommand}</code>
                        <button
                          class="btn"
                          onclick={() => copyToClipboard(activeCloneCommand, 'Clone command copied.')}
                          type="button"
                        >
                          <Copy size={14} />
                        </button>
                      </div>
                    </div>

                    <div class="grid grid-cols-2 gap-2 border-t gt-divider p-3">
                      <a class="btn justify-center" href={archiveZipUrl}>Download ZIP</a>
                      <a class="btn justify-center" href={archiveTarUrl}>Download tar.gz</a>
                    </div>
                  </div>
                {/if}
              </div>
            </div>
          </div>

          {#if recentCommits.length > 0}
            <div class="border-t gt-divider bg-[#0d1117] px-4 py-2 text-sm">
              <span class="font-semibold text-[#f0f6fc]">{recentCommits[0].author_name}</span>
              <span class="mx-2 gt-muted">{recentCommits[0].message_short}</span>
              <span class="gt-muted">· {formatDateTime(recentCommits[0].authored_at)}</span>
            </div>
          {/if}
          <FileTree entries={tree} refName={selectedRef} repo={data.repo} />
        </div>

        <section class="card-surface overflow-hidden">
          <div class="gt-doc-tabs">
            <a
              class="gt-doc-tab active"
              href={`/${data.repo}?ref=${encodeURIComponent(selectedRef)}`}
            >
              <BookOpen size={15} />
              README
            </a>
            {#each filteredDocTabs as entry}
              <a
                class="gt-doc-tab"
                href={`/${data.repo}/blob/${encodeRepoPath(entry.path)}?ref=${encodeURIComponent(selectedRef)}`}
              >
                {#if entry.name.toLowerCase() === 'security.md'}
                  <Shield size={15} />
                {:else if entry.name.toLowerCase() === 'code_of_conduct.md'}
                  <Users size={15} />
                {:else if entry.name.toLowerCase() === 'license'}
                  <Scale size={15} />
                {:else}
                  <BookOpen size={15} />
                {/if}
                {docTabLabel(entry.name)}
              </a>
            {/each}
          </div>

          {#if readme}
            <article class="github-markdown px-8 py-6">
              {@html readmeHtml}
            </article>
          {:else}
            <div class="p-5 text-sm gt-muted">No README found for this ref.</div>
          {/if}
        </section>
      </div>

      <aside class="space-y-3">
        <div class="card-surface p-4">
          <h3 class="text-sm font-semibold text-[#f0f6fc]">About</h3>
          {#if repo.description}
            <p class="mt-2 text-sm text-[#c9d1d9]">{repo.description}</p>
          {:else}
            <p class="mt-2 text-sm gt-muted">No description provided.</p>
          {/if}
          <a class="mt-3 inline-flex text-sm link-accent hover:underline" href={repo.url} target="_blank">
            Open remote repository
          </a>
        </div>

        <div class="card-surface p-4">
          <h3 class="text-sm font-semibold text-[#f0f6fc]">Repository Stats</h3>
          <dl class="mt-3 grid grid-cols-2 gap-2 text-xs">
            <dt class="gt-muted">Branches</dt>
            <dd class="text-right">{refs?.branches.length ?? 0}</dd>
            <dt class="gt-muted">Tags</dt>
            <dd class="text-right">{refs?.tags.length ?? 0}</dd>
            <dt class="gt-muted">Size</dt>
            <dd class="text-right">{repo.size_kb} KB</dd>
            <dt class="gt-muted">Commits</dt>
            <dd class="text-right">{totalCommitCount ?? recentCommits.length}</dd>
          </dl>
          {#if repo.last_fetched}
            <p class="mt-3 text-xs gt-muted">Last fetched: {formatDateTime(repo.last_fetched)}</p>
          {/if}
        </div>

        <div class="card-surface p-4">
          <h3 class="text-sm font-semibold text-[#f0f6fc]">Languages</h3>
          {#if visibleLanguageStats.length > 0}
            <div
              aria-label="Repository language distribution"
              class="gt-language-bar mt-3"
              role="img"
            >
              {#each visibleLanguageStats as stat (stat.language)}
                <span
                  class="gt-language-segment"
                  style={`width: ${Math.max(stat.percentage, 0.8)}%; background-color: ${languageColor(stat.language)};`}
                  title={`${languageLabel(stat.language)} ${languagePercent(stat.percentage)}`}
                ></span>
              {/each}
            </div>
            <ul class="gt-language-list mt-3">
              {#each visibleLanguageStats as stat (stat.language)}
                <li class="gt-language-item">
                  <span
                    aria-hidden="true"
                    class="gt-language-dot"
                    style={`background-color: ${languageColor(stat.language)};`}
                  ></span>
                  <span class="text-xs font-semibold text-[#f0f6fc]">{languageLabel(stat.language)}</span>
                  <span class="text-xs gt-muted">{languagePercent(stat.percentage)}</span>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="mt-2 text-sm gt-muted">No language data available for this reference.</p>
          {/if}
        </div>
      </aside>
    </div>
  </section>
{/if}
