<script lang="ts">
  import { goto } from '$app/navigation';
  import BlobViewer from '$lib/components/BlobViewer.svelte';
  import BranchSelector from '$lib/components/BranchSelector.svelte';
  import Breadcrumb from '$lib/components/Breadcrumb.svelte';
  import { api } from '$lib/api';
  import { formatDateTime } from '$lib/time';
  import type { BlobResponse, CommitInfo, RefsResponse } from '$lib/types';
  import { onMount } from 'svelte';

  interface PageData {
    repo: string;
    path: string;
    refName: string;
  }

  interface Props {
    data: PageData;
  }

  let { data }: Props = $props();
  let refs = $state<RefsResponse | null>(null);
  let blob = $state<BlobResponse | null>(null);
  let latestCommit = $state<CommitInfo | null>(null);
  let selectedRef = $state(data.refName);
  let loading = $state(true);

  const rawUrl = $derived(api.rawUrl(data.repo, selectedRef || 'main', data.path));

  onMount(() => {
    void bootstrap();
  });

  $effect(() => {
    if (selectedRef.length === 0) return;
    void loadBlob();
  });

  async function bootstrap() {
    refs = await api.getRefs(data.repo);
    if (selectedRef.length === 0) {
      selectedRef = refs.default_branch || 'main';
    }
    await loadBlob();
  }

  async function loadBlob() {
    loading = true;
    try {
      const [nextBlob, commits] = await Promise.all([
        api.getBlob(data.repo, selectedRef, data.path),
        api.getCommits(data.repo, selectedRef, {
          path: data.path,
          limit: 1
        }).catch(() => [])
      ]);
      blob = nextBlob;
      latestCommit = commits[0] ?? null;
    } finally {
      loading = false;
    }
  }

  async function changeRef(value: string) {
    selectedRef = value;
    await goto(`/${data.repo}/blob/${data.path}?ref=${encodeURIComponent(value)}`, {
      replaceState: true,
      noScroll: true
    });
  }
</script>

<section class="space-y-4">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <Breadcrumb mode="blob" path={data.path} refName={selectedRef || 'main'} repo={data.repo} />
    <BranchSelector onSelect={changeRef} refs={refs} selected={selectedRef || 'main'} />
  </div>

  {#if latestCommit}
    <div class="card-surface flex flex-wrap items-center gap-3 p-3 text-xs text-white/70">
      <span>Last commit:</span>
      <code class="rounded bg-black/30 px-1.5 py-0.5 text-primary">{latestCommit.short_hash}</code>
      <span>{latestCommit.message_short}</span>
      <span>by {latestCommit.author_name}</span>
      <time>{formatDateTime(latestCommit.authored_at)}</time>
    </div>
  {/if}

  {#if loading}
    <p class="text-sm text-white/60">Loading file...</p>
  {:else if blob}
    <BlobViewer {blob} filePath={data.path} {rawUrl} />
  {:else}
    <div class="card-surface p-6 text-sm text-white/60">File not found.</div>
  {/if}
</section>
