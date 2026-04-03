<script lang="ts">
  import { goto } from '$app/navigation';
  import BlobViewer from '$lib/components/BlobViewer.svelte';
  import BranchSelector from '$lib/components/BranchSelector.svelte';
  import Breadcrumb from '$lib/components/Breadcrumb.svelte';
  import { api } from '$lib/api';
  import { formatDateTime } from '$lib/time';
  import type { BlobResponse, CommitInfo, RefsResponse } from '$lib/types';
  import { onMount } from 'svelte';
  import ShimmerRows from '$lib/components/ShimmerRows.svelte';

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
  let selectedRef = $state('');
  let loading = $state(true);
  let loadError = $state('');

  const rawUrl = $derived(api.rawUrl(data.repo, selectedRef || 'main', data.path));

  onMount(() => {
    void bootstrap();
  });

  $effect(() => {
    if (selectedRef.length === 0) return;
    void loadBlob();
  });

  async function bootstrap() {
    loading = true;
    loadError = '';
    try {
      refs = await api.getRefs(data.repo);
      if (selectedRef.length === 0) {
        selectedRef = data.refName || refs.default_branch || 'main';
      }
      await loadBlob();
    } catch (err) {
      loadError = extractErrorMessage(err, 'Failed to load file metadata.');
      blob = null;
      latestCommit = null;
    } finally {
      loading = false;
    }
  }

  async function loadBlob() {
    loading = true;
    loadError = '';
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
    } catch (err) {
      blob = null;
      latestCommit = null;
      loadError = extractErrorMessage(err, 'File not found for the selected ref.');
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

  function extractErrorMessage(value: unknown, fallback: string): string {
    if (value instanceof Error && value.message.length > 0) {
      return value.message;
    }
    return fallback;
  }
</script>

<section class="space-y-4">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <Breadcrumb mode="blob" path={data.path} refName={selectedRef || 'main'} repo={data.repo} />
    <BranchSelector onSelect={changeRef} refs={refs} selected={selectedRef || 'main'} />
  </div>

  {#if latestCommit}
    <div class="card-surface flex flex-wrap items-center gap-3 px-3 py-2 text-xs gh-muted">
      <span>Last commit:</span>
      <code class="rounded-sm border gh-divider bg-[#0d1117] px-1.5 py-0.5 font-mono text-[#2f81f7]">
        {latestCommit.short_hash}
      </code>
      <span>{latestCommit.message_short}</span>
      <span>by {latestCommit.author_name}</span>
      <time>{formatDateTime(latestCommit.authored_at)}</time>
    </div>
  {/if}

  {#if loading && blob === null && loadError.length === 0}
    <ShimmerRows rows={10} lineHeightClass="h-5" />
  {:else if loadError.length > 0}
    <div class="card-surface p-6 text-sm text-[#ffdcd7]">{loadError}</div>
  {:else if blob}
    <BlobViewer {blob} filePath={data.path} {rawUrl} repo={data.repo} refName={selectedRef || 'main'} />
  {:else}
    <div class="card-surface p-6 text-sm gh-muted">File not found.</div>
  {/if}
</section>
