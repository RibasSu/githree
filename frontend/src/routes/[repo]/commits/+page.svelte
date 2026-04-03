<script lang="ts">
  import { goto } from '$app/navigation';
  import BranchSelector from '$lib/components/BranchSelector.svelte';
  import CommitLog from '$lib/components/CommitLog.svelte';
  import ShimmerRows from '$lib/components/ShimmerRows.svelte';
  import { api } from '$lib/api';
  import type { CommitInfo, RefsResponse } from '$lib/types';
  import { onMount } from 'svelte';

  interface PageData {
    repo: string;
    refName: string;
  }

  interface Props {
    data: PageData;
  }

  let { data }: Props = $props();
  let refs = $state<RefsResponse | null>(null);
  let commits = $state<CommitInfo[]>([]);
  let selectedRef = $state('');
  let skip = $state(0);
  let limit = 30;
  let loading = $state(true);

  onMount(() => {
    void bootstrap();
  });

  async function bootstrap() {
    refs = await api.getRefs(data.repo);
    if (selectedRef.length === 0) {
      selectedRef = data.refName || refs.default_branch || 'main';
    }
    await loadCommits();
  }

  async function loadCommits() {
    loading = true;
    try {
      commits = await api.getCommits(data.repo, selectedRef || 'main', { skip, limit });
    } finally {
      loading = false;
    }
  }

  async function changeRef(value: string) {
    selectedRef = value;
    skip = 0;
    await goto(`/${data.repo}/commits?ref=${encodeURIComponent(value)}`, {
      replaceState: true,
      noScroll: true
    });
    await loadCommits();
  }

  async function nextPage() {
    skip += limit;
    await loadCommits();
  }

  async function previousPage() {
    skip = Math.max(0, skip - limit);
    await loadCommits();
  }
</script>

<section class="space-y-4">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <h1 class="text-xl font-semibold text-[#f0f6fc]">Commit History</h1>
    <BranchSelector onSelect={changeRef} refs={refs} selected={selectedRef || 'main'} />
  </div>

  <div class="flex flex-wrap items-center gap-2">
    <a class="btn" href={`/${data.repo}?ref=${encodeURIComponent(selectedRef)}`}>Back to repository</a>
    <span class="text-xs gh-muted">Showing {skip + 1} - {skip + commits.length}</span>
  </div>

  {#if loading && commits.length === 0}
    <ShimmerRows rows={10} />
  {:else}
    <CommitLog commits={commits} repo={data.repo} />
    <div class="flex items-center gap-2">
      <button class="btn" disabled={skip === 0} onclick={previousPage} type="button">Previous</button>
      <button class="btn btn-primary" disabled={commits.length < limit} onclick={nextPage} type="button">Next</button>
    </div>
  {/if}
</section>
