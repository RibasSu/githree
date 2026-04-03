<script lang="ts">
  import { ExternalLink, GitBranch, Github, Gitlab, RefreshCcw, Trash2 } from 'lucide-svelte';
  import type { RepoInfo } from '$lib/types';
  import { formatRelativeTime } from '$lib/time';

  interface Props {
    repo: RepoInfo;
    onFetch: (name: string) => Promise<void> | void;
    onRemove: (name: string) => Promise<void> | void;
  }

  let { repo, onFetch, onRemove }: Props = $props();
  const source = $derived(repo.source || 'generic');

  function sourceLabel(): string {
    if (source === 'github') return 'GitHub';
    if (source === 'gitlab') return 'GitLab';
    return 'Git';
  }
</script>

<article class="card-surface p-4">
  <div class="flex items-start justify-between gap-3">
    <div class="min-w-0">
      <h3 class="truncate text-base font-semibold text-white">
        <a class="hover:text-primary" href={`/${repo.name}`}>{repo.name}</a>
      </h3>
      <p class="mt-1 flex items-center gap-2 truncate text-xs text-white/60">
        {#if source === 'github'}
          <Github size={14} />
        {:else if source === 'gitlab'}
          <Gitlab size={14} />
        {:else}
          <GitBranch size={14} />
        {/if}
        <span>{sourceLabel()}</span>
      </p>
      <a
        class="mt-2 inline-flex items-center gap-1 text-xs text-primary hover:text-primary/80"
        href={repo.url}
        rel="noreferrer"
        target="_blank"
      >
        <span class="max-w-[14rem] truncate">{repo.url}</span>
        <ExternalLink size={12} />
      </a>
    </div>
    <span class="rounded-md border border-white/10 bg-black/30 px-2 py-1 text-xs text-white/70">
      {repo.default_branch}
    </span>
  </div>

  <div class="mt-4 flex items-center justify-between text-xs text-white/60">
    <span>
      {#if repo.last_fetched}
        updated {formatRelativeTime(repo.last_fetched)}
      {:else}
        never fetched
      {/if}
    </span>
    <span>{repo.size_kb} KB</span>
  </div>

  <div class="mt-4 flex flex-wrap gap-2">
    <a class="btn btn-primary" href={`/${repo.name}`}>
      Browse
    </a>
    <button aria-label={`Fetch ${repo.name}`} class="btn" onclick={() => onFetch(repo.name)} type="button">
      <RefreshCcw size={14} />
      Fetch
    </button>
    <button
      aria-label={`Remove ${repo.name}`}
      class="btn btn-danger"
      onclick={() => onRemove(repo.name)}
      type="button"
    >
      <Trash2 size={14} />
      Remove
    </button>
  </div>
</article>
