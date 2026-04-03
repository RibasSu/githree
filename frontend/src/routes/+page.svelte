<script lang="ts">
  import { onMount } from 'svelte';
  import RepoCard from '$lib/components/RepoCard.svelte';
  import { api } from '$lib/api';
  import type { RepoInfo } from '$lib/types';

  let repoUrl = $state('');
  let repoAlias = $state('');
  let search = $state('');
  let repos = $state<RepoInfo[]>([]);
  let loading = $state(false);

  const filteredRepos = $derived(
    repos.filter((repo) => fuzzyMatch(repo.name, search) || fuzzyMatch(repo.url, search))
  );

  onMount(() => {
    void loadRepos();
  });

  async function loadRepos() {
    loading = true;
    try {
      repos = await api.listRepos();
    } catch {
      // toast already emitted in api client
    } finally {
      loading = false;
    }
  }

  async function submitRepo(event: SubmitEvent) {
    event.preventDefault();
    if (repoUrl.trim().length === 0) return;

    try {
      await api.addRepo(repoUrl.trim(), repoAlias.trim() || undefined);
      repoUrl = '';
      repoAlias = '';
      api.notify('Repository added.', 'success');
      await loadRepos();
    } catch {
      // toast already emitted in api client
    }
  }

  async function fetchRepo(name: string) {
    try {
      await api.fetchRepo(name);
      api.notify(`Fetched ${name}.`, 'success');
      await loadRepos();
    } catch {
      // toast already emitted in api client
    }
  }

  async function removeRepo(name: string) {
    const approved = window.confirm(`Remove repository "${name}" from Githree?`);
    if (!approved) return;
    try {
      await api.deleteRepo(name);
      api.notify(`Removed ${name}.`, 'success');
      await loadRepos();
    } catch {
      // toast already emitted in api client
    }
  }

  function fuzzyMatch(value: string, needle: string): boolean {
    const term = needle.trim().toLowerCase();
    if (term.length === 0) return true;
    const source = value.toLowerCase();
    if (source.includes(term)) return true;

    let index = 0;
    for (const ch of source) {
      if (ch === term[index]) index += 1;
      if (index === term.length) return true;
    }
    return false;
  }
</script>

<section class="space-y-6">
  <div class="card-surface p-5">
    <h1 class="text-xl font-semibold">Add a Repository</h1>
    <p class="mt-1 text-sm text-white/70">
      Paste any GitHub, GitLab, or self-hosted git URL (SSH or HTTPS).
    </p>
    <form class="mt-4 grid gap-3 md:grid-cols-[1fr_240px_auto]" onsubmit={submitRepo}>
      <input
        bind:value={repoUrl}
        class="input"
        placeholder="https://github.com/user/repo.git or git@github.com:user/repo.git"
        required
        type="url"
      />
      <input bind:value={repoAlias} class="input" placeholder="Optional alias (e.g. my-repo)" type="text" />
      <button class="btn btn-primary justify-center" type="submit">Add Repo</button>
    </form>
  </div>

  <div class="flex flex-wrap items-center justify-between gap-3">
    <h2 class="text-lg font-semibold">Registered Repositories</h2>
    <input bind:value={search} class="input max-w-xs" placeholder="Search repositories" type="text" />
  </div>

  {#if loading}
    <p class="text-sm text-white/60">Loading repositories...</p>
  {:else if filteredRepos.length === 0}
    <div class="card-surface p-6 text-sm text-white/60">
      No repositories found. Add your first repository URL above.
    </div>
  {:else}
    <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
      {#each filteredRepos as repo (repo.name)}
        <RepoCard {repo} onFetch={fetchRepo} onRemove={removeRepo} />
      {/each}
    </div>
  {/if}
</section>
