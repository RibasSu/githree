<script lang="ts">
  import { goto } from '$app/navigation';
  import BranchSelector from '$lib/components/BranchSelector.svelte';
  import ShimmerRows from '$lib/components/ShimmerRows.svelte';
  import { api } from '$lib/api';
  import { formatRelativeTime } from '$lib/time';
  import type { CommitInfo, RefsResponse } from '$lib/types';
  import { GitBranch, Search } from 'lucide-svelte';
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
  let selectedRef = $state('');
  let search = $state('');
  let loading = $state(true);
  let latestCommitByBranch = $state<Record<string, CommitInfo | null>>({});
  let commitLookupToken = 0;

  const branches = $derived.by(() => refs?.branches ?? []);
  const normalizedSearch = $derived(search.trim().toLowerCase());
  const filteredBranches = $derived.by(() => {
    if (normalizedSearch.length === 0) return branches;
    return branches.filter((branch) => branch.toLowerCase().includes(normalizedSearch));
  });

  onMount(() => {
    void bootstrap();
  });

  async function bootstrap() {
    loading = true;
    try {
      const nextRefs = await api.getRefs(data.repo);
      refs = nextRefs;
      selectedRef =
        resolveSelectedRef(data.refName, nextRefs) || nextRefs.default_branch || nextRefs.branches[0] || 'main';
      await loadLatestCommits(nextRefs.branches);
    } finally {
      loading = false;
    }
  }

  function resolveSelectedRef(requestedRef: string, nextRefs: RefsResponse): string {
    if (requestedRef.length === 0) return '';
    if (nextRefs.branches.includes(requestedRef)) return requestedRef;
    if (nextRefs.tags.includes(requestedRef)) return requestedRef;
    return '';
  }

  async function loadLatestCommits(branchNames: string[]) {
    const token = ++commitLookupToken;
    if (branchNames.length === 0) {
      latestCommitByBranch = {};
      return;
    }

    const lookupMap: Record<string, CommitInfo | null> = {};
    const queue = [...branchNames];
    const MAX_CONCURRENCY = 6;
    const workers = Array.from({ length: Math.min(MAX_CONCURRENCY, queue.length) }, async () => {
      while (queue.length > 0) {
        const branch = queue.shift();
        if (!branch) continue;

        try {
          const commits = await api.getCommits(data.repo, branch, { limit: 1 });
          if (token !== commitLookupToken) return;
          lookupMap[branch] = commits[0] ?? null;
        } catch {
          if (token !== commitLookupToken) return;
          lookupMap[branch] = null;
        }
      }
    });

    await Promise.all(workers);
    if (token !== commitLookupToken) return;
    latestCommitByBranch = lookupMap;
  }

  async function changeRef(value: string) {
    selectedRef = value;
    await goto(`/${data.repo}/branches?ref=${encodeURIComponent(value)}`, {
      replaceState: true,
      noScroll: true
    });
  }

  function branchHref(branch: string): string {
    return `/${data.repo}?ref=${encodeURIComponent(branch)}`;
  }
</script>

<section class="space-y-4">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div>
      <h1 class="text-2xl font-semibold text-[#f0f6fc]">Branches</h1>
      <p class="text-sm gh-muted">Browse and switch branches for this repository.</p>
    </div>
    <div class="flex flex-wrap items-center gap-2">
      <a class="btn" href={`/${data.repo}?ref=${encodeURIComponent(selectedRef || 'main')}`}>Back to repository</a>
      <BranchSelector onSelect={changeRef} refs={refs} repoName={data.repo} selected={selectedRef || 'main'} />
    </div>
  </div>

  <section class="card-surface overflow-hidden">
    <div class="border-b gh-divider bg-[#0d1117] px-4 py-3">
      <div class="flex items-center gap-4 text-sm">
        <span class="gh-branch-tab-active pb-2">All</span>
      </div>

      <label class="gh-go-to-file mt-3 w-full" for="branch-search-input">
        <Search aria-hidden="true" class="gh-muted" size={15} />
        <input
          bind:value={search}
          autocomplete="off"
          id="branch-search-input"
          placeholder="Search branches..."
          spellcheck="false"
          type="text"
        />
      </label>
    </div>

    {#if loading}
      <div class="p-4">
        <ShimmerRows rows={8} />
      </div>
    {:else if filteredBranches.length === 0}
      <div class="p-4 text-sm gh-muted">No branches match your search.</div>
    {:else}
      <div class="overflow-x-auto">
        <table class="w-full min-w-[780px] text-sm">
          <thead class="bg-[#161b22] text-[#8b949e]">
            <tr class="border-b gh-divider">
              <th class="px-4 py-2 text-left font-medium">Branch</th>
              <th class="px-4 py-2 text-left font-medium">Updated</th>
              <th class="px-4 py-2 text-left font-medium">Check status</th>
              <th class="px-4 py-2 text-left font-medium">Behind / Ahead</th>
              <th class="px-4 py-2 text-left font-medium">Pull request</th>
            </tr>
          </thead>
          <tbody>
            {#each filteredBranches as branch}
              <tr class="border-b gh-divider hover:bg-[#161b22]">
                <td class="px-4 py-3">
                  <div class="flex items-center gap-2">
                    <GitBranch aria-hidden="true" class="gh-muted" size={14} />
                    <a class="link-accent font-medium hover:underline" href={branchHref(branch)}>
                      {branch}
                    </a>
                    {#if branch === refs?.default_branch}
                      <span class="gh-ref-default">default</span>
                    {/if}
                  </div>
                </td>
                <td class="px-4 py-3">
                  {#if latestCommitByBranch[branch]}
                    <a
                      class="link-muted"
                      href={`/${data.repo}/commit/${latestCommitByBranch[branch]?.hash}?ref=${encodeURIComponent(branch)}`}
                    >
                      {formatRelativeTime(latestCommitByBranch[branch]?.authored_at || '')}
                    </a>
                  {:else}
                    <span class="gh-muted">-</span>
                  {/if}
                </td>
                <td class="px-4 py-3"><span class="gh-muted">-</span></td>
                <td class="px-4 py-3"><span class="gh-muted">- / -</span></td>
                <td class="px-4 py-3"><span class="gh-muted">-</span></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </section>
</section>
