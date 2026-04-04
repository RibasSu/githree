<script lang="ts">
  import { ExternalLink, RefreshCcw, Trash2 } from 'lucide-svelte';
  import SourceLogo from '$lib/components/SourceLogo.svelte';
  import type { RepoInfo } from '$lib/types';
  import { formatRelativeTime } from '$lib/time';

  interface Props {
    repo: RepoInfo;
    onFetch?: (name: string) => Promise<void> | void;
    onRemove?: (name: string) => Promise<void> | void;
    onCopyRemoveCommand?: (name: string) => Promise<void> | void;
    webRepoManagement: boolean;
  }

  let { repo, onFetch, onRemove, onCopyRemoveCommand, webRepoManagement }: Props = $props();
  const source = $derived(repo.source || 'generic');

  function sourceLabel(): string {
    if (source === 'github') return 'GitHub';
    if (source === 'gitlab') return 'GitLab';
    return 'Git';
  }
</script>

<article class="grid gap-3 border-t gt-divider px-4 py-3 md:grid-cols-[minmax(220px,1fr)_minmax(220px,1fr)_auto] md:items-center">
  <div class="min-w-0">
    <h3 class="truncate text-[15px] font-semibold text-[#f0f6fc]">
      <a class="hover:underline" href={`/${repo.name}`}>{repo.name}</a>
    </h3>
    <p class="mt-1 flex items-center gap-2 truncate text-xs gt-muted">
      <span class="inline-flex items-center gap-1">
        <SourceLogo size={14} source={source} />
        <span>{sourceLabel()}</span>
      </span>
      <span class="mx-1">·</span>
      <span>default: {repo.default_branch}</span>
    </p>
  </div>

  <div class="min-w-0">
    <a class="inline-flex max-w-full items-center gap-1 truncate text-xs link-accent hover:underline" href={repo.url} rel="noreferrer" target="_blank">
      <span class="truncate">{repo.url}</span>
      <ExternalLink size={12} />
    </a>
    <p class="mt-1 text-xs gt-muted">
      {#if repo.last_fetched}
        updated {formatRelativeTime(repo.last_fetched)}
      {:else}
        never fetched
      {/if}
      · {repo.size_kb} KB
    </p>
  </div>

  <div class="flex flex-wrap items-center gap-2 md:justify-end">
    <a class="btn btn-primary" href={`/${repo.name}`}>
      Browse
    </a>
    {#if webRepoManagement}
      <button
        aria-label={`Fetch ${repo.name}`}
        class="btn"
        onclick={() => onFetch?.(repo.name)}
        type="button"
      >
        <RefreshCcw size={14} />
        Fetch
      </button>
      <button
        aria-label={`Remove ${repo.name}`}
        class="btn btn-danger"
        onclick={() => onRemove?.(repo.name)}
        type="button"
      >
        <Trash2 size={14} />
        Remove
      </button>
    {:else}
      <button
        aria-label={`Copy remove command for ${repo.name}`}
        class="btn"
        onclick={() => onCopyRemoveCommand?.(repo.name)}
        type="button"
      >
        <Trash2 size={14} />
        Copy Remove CLI
      </button>
    {/if}
  </div>
</article>
