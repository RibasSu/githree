<script lang="ts">
  import { goto } from '$app/navigation';
  import BranchSelector from '$lib/components/BranchSelector.svelte';
  import FileTree from '$lib/components/FileTree.svelte';
  import { api } from '$lib/api';
  import { highlightMarkdownCodeBlocks } from '$lib/markdown';
  import { formatDateTime } from '$lib/time';
  import type { CommitInfo, ReadmeResponse, RefsResponse, RepoInfo, TreeEntry } from '$lib/types';
  import { BookOpen, ChevronDown, Code2, Copy, FileText, Folder, Github, Gitlab, GitBranch, Scale, Search, Shield, Tag, Users } from 'lucide-svelte';
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

  onMount(() => {
    void bootstrap();
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

      const [nextTree, nextReadme, commits] = await Promise.all([
        api.getTree(data.repo, effectiveRef, ''),
        api.getReadme(data.repo, effectiveRef).catch(() => null),
        api.getCommits(data.repo, effectiveRef, { limit: 30 })
      ]);
      if (selectedRef !== effectiveRef) return;
      tree = nextTree;
      readme = nextReadme;
      recentCommits = commits;
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
</script>

{#if loading}
  <div class="space-y-3">
    <ShimmerRows rows={3} />
    <ShimmerRows rows={7} />
    <ShimmerRows rows={6} />
  </div>
{:else if repo === null}
  <div class="card-surface p-6 text-sm gh-muted">Repository "{data.repo}" was not found.</div>
{:else}
  <section class="space-y-4">
    <header class="flex flex-wrap items-center justify-between gap-3 border-b gh-divider pb-3">
      <div>
        <h1 class="flex items-center gap-2 text-2xl font-semibold text-[#f0f6fc]">
          {#if isGithubRepo}
            <Github class="text-[#8b949e]" size={20} />
          {:else if isGitlabRepo}
            <Gitlab class="text-[#fc6d26]" size={20} />
          {:else}
            <img alt="Git" class="h-5 w-5" height="20" src="/git-logo.svg" width="20" />
          {/if}

          {#if remoteCoordinates.namespace.length > 0}
            <span class="text-[#8b949e]">{remoteCoordinates.namespace}</span>
            <span class="gh-muted">/</span>
          {/if}
          <span>{remoteCoordinates.repositoryName}</span>
        </h1>
        <a class="mt-1 inline-flex items-center text-sm link-accent hover:underline" href={repo.url} target="_blank">
          {repo.url}
        </a>
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
          <div class="flex flex-wrap items-center justify-between gap-2 border-b gh-divider px-3 py-2">
            <div class="flex flex-wrap items-center gap-3">
              <BranchSelector compact onSelect={changeRef} refs={refs} repoName={data.repo} selected={selectedRef} />
              <span class="gh-toolbar-stat">
                <GitBranch size={14} />
                {#if (refs?.branches.length ?? 0) === 1}
                  1 Branch
                {:else}
                  {(refs?.branches.length ?? 0).toLocaleString()} Branches
                {/if}
              </span>
              <span class="gh-toolbar-stat">
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
                <form class="gh-go-to-file" onsubmit={submitGoToFile}>
                  <Search class="gh-muted" size={15} />
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
                  <div class="gh-go-to-file-menu">
                    {#if fileSearchLoading && fileSearchResults.length === 0}
                      <div class="gh-go-to-file-empty">Indexing repository files...</div>
                    {:else if fileSearchResults.length === 0}
                      <div class="gh-go-to-file-empty">No matching files or folders.</div>
                    {:else}
                      <ul class="gh-go-to-file-items">
                        {#each fileSearchResults as result, index (result.path)}
                          <li>
                            <button
                              class={`gh-go-to-file-item ${index === fileSearchActiveIndex ? 'active' : ''}`}
                              onclick={() => navigateToSearchEntry(result)}
                              type="button"
                            >
                              {#if result.entryType === 'tree'}
                                <Folder aria-hidden="true" class="gh-muted" size={14} />
                              {:else}
                                <FileText aria-hidden="true" class="gh-muted" size={14} />
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

              <div class="relative">
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
                  <div class="gh-code-menu">
                    <div class="flex items-center gap-1 border-b gh-divider px-2">
                      <button
                        class={`gh-code-menu-tab ${cloneTab === 'https' ? 'active' : ''}`}
                        onclick={() => {
                          cloneTab = 'https';
                        }}
                        type="button"
                      >
                        HTTPS
                      </button>
                      <button
                        class={`gh-code-menu-tab ${cloneTab === 'ssh' ? 'active' : ''}`}
                        onclick={() => {
                          cloneTab = 'ssh';
                        }}
                        type="button"
                      >
                        SSH
                      </button>
                      {#if isGithubRepo}
                        <button
                          class={`gh-code-menu-tab ${cloneTab === 'cli' ? 'active' : ''}`}
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
                      <p class="text-xs font-semibold uppercase tracking-wide gh-muted">Clone</p>
                      <div class="flex items-center gap-2 rounded-md border gh-divider bg-[#161b22] p-2">
                        <code class="gh-clone-command" title={activeCloneCommand}>{activeCloneCommand}</code>
                        <button
                          class="btn"
                          onclick={() => copyToClipboard(activeCloneCommand, 'Clone command copied.')}
                          type="button"
                        >
                          <Copy size={14} />
                        </button>
                      </div>
                    </div>

                    <div class="grid grid-cols-2 gap-2 border-t gh-divider p-3">
                      <a class="btn justify-center" href={archiveZipUrl}>Download ZIP</a>
                      <a class="btn justify-center" href={archiveTarUrl}>Download tar.gz</a>
                    </div>
                  </div>
                {/if}
              </div>
            </div>
          </div>

          {#if recentCommits.length > 0}
            <div class="border-t gh-divider bg-[#0d1117] px-4 py-2 text-sm">
              <span class="font-semibold text-[#f0f6fc]">{recentCommits[0].author_name}</span>
              <span class="mx-2 gh-muted">{recentCommits[0].message_short}</span>
              <span class="gh-muted">· {formatDateTime(recentCommits[0].authored_at)}</span>
            </div>
          {/if}
          <FileTree entries={tree} refName={selectedRef} repo={data.repo} />
        </div>

        <section class="card-surface overflow-hidden">
          <div class="gh-doc-tabs">
            <a
              class="gh-doc-tab active"
              href={`/${data.repo}?ref=${encodeURIComponent(selectedRef)}`}
            >
              <BookOpen size={15} />
              README
            </a>
            {#each filteredDocTabs as entry}
              <a
                class="gh-doc-tab"
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
            <div class="p-5 text-sm gh-muted">No README found for this ref.</div>
          {/if}
        </section>
      </div>

      <aside class="space-y-3">
        <div class="card-surface p-4">
          <h3 class="text-sm font-semibold text-[#f0f6fc]">About</h3>
          {#if repo.description}
            <p class="mt-2 text-sm text-[#c9d1d9]">{repo.description}</p>
          {:else}
            <p class="mt-2 text-sm gh-muted">No description provided.</p>
          {/if}
          <a class="mt-3 inline-flex text-sm link-accent hover:underline" href={repo.url} target="_blank">
            Open remote repository
          </a>
        </div>

        <div class="card-surface p-4">
          <h3 class="text-sm font-semibold text-[#f0f6fc]">Repository Stats</h3>
          <dl class="mt-3 grid grid-cols-2 gap-2 text-xs">
            <dt class="gh-muted">Branches</dt>
            <dd class="text-right">{refs?.branches.length ?? 0}</dd>
            <dt class="gh-muted">Tags</dt>
            <dd class="text-right">{refs?.tags.length ?? 0}</dd>
            <dt class="gh-muted">Size</dt>
            <dd class="text-right">{repo.size_kb} KB</dd>
            <dt class="gh-muted">Loaded commits</dt>
            <dd class="text-right">{recentCommits.length}</dd>
          </dl>
          {#if repo.last_fetched}
            <p class="mt-3 text-xs gh-muted">Last fetched: {formatDateTime(repo.last_fetched)}</p>
          {/if}
        </div>
      </aside>
    </div>
  </section>
{/if}
