<script lang="ts">
  import { goto } from '$app/navigation';
  import Breadcrumb from '$lib/components/Breadcrumb.svelte';
  import BranchSelector from '$lib/components/BranchSelector.svelte';
  import FileTree from '$lib/components/FileTree.svelte';
  import ShimmerRows from '$lib/components/ShimmerRows.svelte';
  import { api } from '$lib/api';
  import type { RefsResponse, TreeEntry } from '$lib/types';
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
  let entries = $state<TreeEntry[]>([]);
  let selectedRef = $state('');
  let loading = $state(true);

  const parentPath = $derived.by(() => {
    const segments = data.path.split('/').filter((segment) => segment.length > 0);
    if (segments.length <= 1) return '';
    return segments.slice(0, -1).join('/');
  });

  onMount(() => {
    void bootstrap();
  });

  $effect(() => {
    if (selectedRef.length === 0) return;
    void loadTree();
  });

  async function bootstrap() {
    refs = await api.getRefs(data.repo);
    if (selectedRef.length === 0) {
      selectedRef = data.refName || refs.default_branch || 'main';
    }
    await loadTree();
  }

  async function loadTree() {
    loading = true;
    try {
      entries = await api.getTree(data.repo, selectedRef, data.path);
    } finally {
      loading = false;
    }
  }

  async function changeRef(value: string) {
    selectedRef = value;
    await goto(`/${data.repo}/tree/${data.path}?ref=${encodeURIComponent(value)}`, {
      replaceState: true,
      noScroll: true
    });
  }
</script>

<section class="space-y-4">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <Breadcrumb mode="tree" path={data.path} refName={selectedRef || 'main'} repo={data.repo} />
    <BranchSelector onSelect={changeRef} refs={refs} selected={selectedRef || 'main'} />
  </div>

  <div class="flex flex-wrap gap-2">
    {#if parentPath.length > 0}
      <a class="btn" href={`/${data.repo}/tree/${parentPath}?ref=${encodeURIComponent(selectedRef)}`}>Back to parent</a>
    {:else}
      <a class="btn" href={`/${data.repo}?ref=${encodeURIComponent(selectedRef)}`}>Back to root</a>
    {/if}
    <a class="btn btn-primary" href={api.archiveUrl(data.repo, selectedRef || 'main', 'tar.gz')}>
      Download this folder as archive
    </a>
  </div>

  {#if loading && entries.length === 0}
    <ShimmerRows rows={8} />
  {:else}
    <FileTree {entries} refName={selectedRef || 'main'} repo={data.repo} />
  {/if}
</section>
