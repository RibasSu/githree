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
  let selectedRef = $state(data.refName);
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
        selectedRef = refs.default_branch || repo?.default_branch || 'main';
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
    readmeHtml = DOMPurify.sanitize(rendered);
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
</script>

{#if loading}
  <p class="text-sm text-white/60">Loading repository...</p>
{:else if repo === null}
  <div class="card-surface p-6 text-sm text-white/60">Repository "{data.repo}" was not found.</div>
{:else}
  <section class="grid gap-6 xl:grid-cols-[1fr_260px]">
    <div class="space-y-6">
      <div class="card-surface p-5">
        <div class="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h1 class="text-2xl font-semibold">{repo.name}</h1>
            <a class="mt-2 inline-block text-sm text-primary hover:text-primary/80" href={repo.url} target="_blank">
              {repo.url}
            </a>
          </div>
          <BranchSelector onSelect={changeRef} refs={refs} selected={selectedRef} />
        </div>

        <div class="mt-4 grid gap-3 text-xs text-white/70 md:grid-cols-2">
          <div class="rounded-lg border border-white/10 bg-black/20 p-3">
            <p class="uppercase tracking-wide text-white/50">HTTPS clone</p>
            <code class="mt-1 block break-all text-white">{repo.url}</code>
          </div>
          <div class="rounded-lg border border-white/10 bg-black/20 p-3">
            <p class="uppercase tracking-wide text-white/50">SSH clone</p>
            <code class="mt-1 block break-all text-white">
              {sshCloneCommand(repo.url)}
            </code>
          </div>
        </div>

        <div class="mt-4 flex flex-wrap gap-2">
          <a class="btn btn-primary" href={archiveTarUrl}>Download .tar.gz</a>
          <a class="btn" href={archiveZipUrl}>Download .zip</a>
          <a class="btn" href={`/${data.repo}/commits?ref=${encodeURIComponent(selectedRef)}`}>View commits</a>
        </div>
      </div>

      <section class="space-y-3">
        <h2 class="text-lg font-semibold">Repository Tree</h2>
        <FileTree entries={tree} refName={selectedRef} repo={data.repo} />
      </section>

      <section class="space-y-3">
        <h2 class="text-lg font-semibold">README</h2>
        {#if readme}
          <article class="card-surface p-5 github-markdown">
            {@html readmeHtml}
          </article>
        {:else}
          <div class="card-surface p-5 text-sm text-white/60">No README found for this ref.</div>
        {/if}
      </section>
    </div>

    <aside class="space-y-3">
      <div class="card-surface p-4">
        <h3 class="text-sm font-semibold">Repository Stats</h3>
        <dl class="mt-3 grid grid-cols-2 gap-2 text-xs">
          <dt class="text-white/60">Branches</dt>
          <dd class="text-right">{refs?.branches.length ?? 0}</dd>
          <dt class="text-white/60">Tags</dt>
          <dd class="text-right">{refs?.tags.length ?? 0}</dd>
          <dt class="text-white/60">Size</dt>
          <dd class="text-right">{repo.size_kb} KB</dd>
          <dt class="text-white/60">Loaded commits</dt>
          <dd class="text-right">{recentCommits.length}</dd>
        </dl>
        {#if repo.last_fetched}
          <p class="mt-3 text-xs text-white/50">Last fetched: {formatDateTime(repo.last_fetched)}</p>
        {/if}
      </div>
    </aside>
  </section>
{/if}
