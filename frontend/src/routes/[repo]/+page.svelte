<script lang="ts">
  import { goto } from '$app/navigation';
  import BranchSelector from '$lib/components/BranchSelector.svelte';
  import FileTree from '$lib/components/FileTree.svelte';
  import { api } from '$lib/api';
  import { formatDateTime } from '$lib/time';
  import type { CommitInfo, ReadmeResponse, RefsResponse, RepoInfo, TreeEntry } from '$lib/types';
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

  let { data }: Props = $props();
  let repo = $state<RepoInfo | null>(null);
  let refs = $state<RefsResponse | null>(null);
  let tree = $state<TreeEntry[]>([]);
  let readme = $state<ReadmeResponse | null>(null);
  let readmeHtml = $state('');
  let recentCommits = $state<CommitInfo[]>([]);
  let selectedRef = $state('');
  let loading = $state(true);

  onMount(() => {
    void bootstrap();
  });

  $effect(() => {
    if (selectedRef.length === 0) return;
    void loadForRef();
  });

  async function bootstrap() {
    loading = true;
    try {
      const all = await api.listRepos();
      repo = all.find((item) => item.name === data.repo) ?? null;
      refs = await api.getRefs(data.repo);
      if (selectedRef.length === 0) {
        selectedRef = data.refName || refs.default_branch || repo?.default_branch || 'main';
      }
      await loadForRef();
    } finally {
      loading = false;
    }
  }

  async function loadForRef() {
    if (selectedRef.length === 0) return;
    try {
      const [nextTree, nextReadme, commits] = await Promise.all([
        api.getTree(data.repo, selectedRef, ''),
        api.getReadme(data.repo, selectedRef).catch(() => null),
        api.getCommits(data.repo, selectedRef, { limit: 30 })
      ]);
      tree = nextTree;
      readme = nextReadme;
      recentCommits = commits;
      await renderReadme();
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
    readmeHtml = DOMPurify.sanitize(rewritten);
  }

  async function changeRef(value: string) {
    selectedRef = value;
    await goto(`/${data.repo}?ref=${encodeURIComponent(value)}`, { replaceState: true, noScroll: true });
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

  const archiveTarUrl = $derived(api.archiveUrl(data.repo, selectedRef || 'main', 'tar.gz'));
  const archiveZipUrl = $derived(api.archiveUrl(data.repo, selectedRef || 'main', 'zip'));
  const highlightedDocs = $derived.by(() =>
    tree.filter((entry) =>
      ['readme.md', 'security.md', 'contributing.md', 'license', 'code_of_conduct.md'].includes(
        entry.name.toLowerCase()
      )
    )
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
        <h1 class="text-2xl font-semibold text-[#f0f6fc]">
          <span class="gh-muted">{repo.source === 'github' ? 'github' : repo.source}</span>
          <span class="mx-2 gh-muted">/</span>
          {repo.name}
        </h1>
        <a class="mt-1 inline-flex items-center text-sm link-accent hover:underline" href={repo.url} target="_blank">
          {repo.url}
        </a>
      </div>
      <div class="flex flex-wrap gap-2">
        <a class="btn" href={archiveTarUrl}>Download tar.gz</a>
        <a class="btn" href={archiveZipUrl}>Download zip</a>
        <a class="btn btn-primary" href={`/${data.repo}/commits?ref=${encodeURIComponent(selectedRef)}`}>
          Commits
        </a>
      </div>
    </header>

    <div class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_300px]">
    <div class="space-y-6">
      <div class="card-surface overflow-hidden">
        <div class="flex flex-wrap items-center justify-between gap-3 border-b gh-divider px-4 py-3">
          <BranchSelector onSelect={changeRef} refs={refs} selected={selectedRef} />
          <div class="flex flex-wrap items-center gap-2 text-xs gh-muted">
            <span>{refs?.branches.length ?? 0} branches</span>
            <span>·</span>
            <span>{refs?.tags.length ?? 0} tags</span>
            <span>·</span>
            <span>{repo.size_kb} KB</span>
          </div>
        </div>
        <div class="grid gap-3 px-4 py-3 text-xs md:grid-cols-2">
          <div class="rounded-md border gh-divider bg-[#0d1117] p-3">
            <p class="gh-muted">HTTPS clone</p>
            <code class="mt-1 block break-all text-[#c9d1d9]">{repo.url}</code>
          </div>
          <div class="rounded-md border gh-divider bg-[#0d1117] p-3">
            <p class="gh-muted">SSH clone</p>
            <code class="mt-1 block break-all text-[#c9d1d9]">
              {sshCloneCommand(repo.url)}
            </code>
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

      <section class="space-y-3">
        <h2 class="text-lg font-semibold text-[#f0f6fc]">README</h2>
        {#if readme}
          <article class="card-surface p-5 github-markdown">
            {@html readmeHtml}
          </article>
        {:else}
          <div class="card-surface p-5 text-sm gh-muted">No README found for this ref.</div>
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
        <h3 class="text-sm font-semibold text-[#f0f6fc]">Project Files</h3>
        {#if highlightedDocs.length > 0}
          <ul class="mt-2 space-y-1.5 text-sm">
            {#each highlightedDocs as entry}
              <li>
                <a class="link-accent hover:underline" href={`/${data.repo}/blob/${encodeRepoPath(entry.path)}?ref=${encodeURIComponent(selectedRef)}`}>
                  {entry.name}
                </a>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="mt-2 text-sm gh-muted">No highlighted docs in repository root.</p>
        {/if}
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
