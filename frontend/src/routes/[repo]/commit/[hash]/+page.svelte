<script lang="ts">
  import { api } from '$lib/api';
  import { formatDateTime } from '$lib/time';
  import type { CommitDetail } from '$lib/types';
  import { onMount } from 'svelte';

  interface PageData {
    repo: string;
    hash: string;
  }

  interface Props {
    data: PageData;
  }

  let { data }: Props = $props();
  let detail = $state<CommitDetail | null>(null);
  let loading = $state(true);

  onMount(() => {
    void loadCommit();
  });

  async function loadCommit() {
    loading = true;
    try {
      detail = await api.getCommit(data.repo, data.hash);
    } finally {
      loading = false;
    }
  }

  function lineClass(lineType: string): string {
    if (lineType === 'add') return 'bg-emerald-500/15 text-emerald-100';
    if (lineType === 'delete') return 'bg-red-500/15 text-red-100';
    return 'text-white/80';
  }
</script>

{#if loading}
  <p class="text-sm text-white/60">Loading commit...</p>
{:else if detail === null}
  <div class="card-surface p-6 text-sm text-white/60">Commit not found.</div>
{:else}
  <section class="space-y-4">
    <a class="btn" href={`/${data.repo}/commits`}>Back to history</a>

    <article class="card-surface p-5">
      <h1 class="text-lg font-semibold">{detail.commit.message_short}</h1>
      <p class="mt-2 text-sm text-white/70">{detail.commit.message}</p>

      <dl class="mt-4 grid gap-2 text-xs md:grid-cols-2">
        <div>
          <dt class="text-white/50">Hash</dt>
          <dd class="font-mono text-primary">{detail.commit.hash}</dd>
        </div>
        <div>
          <dt class="text-white/50">Author</dt>
          <dd>{detail.commit.author_name} ({detail.commit.author_email})</dd>
        </div>
        <div>
          <dt class="text-white/50">Date</dt>
          <dd>{formatDateTime(detail.commit.authored_at)}</dd>
        </div>
        <div>
          <dt class="text-white/50">Parents</dt>
          <dd class="font-mono">{detail.parents.join(', ')}</dd>
        </div>
      </dl>
    </article>

    <div class="card-surface p-4 text-xs">
      <span class="font-semibold text-white">Diff Stats: </span>
      <span class="text-white/70">{detail.stats.files_changed} files changed</span>
      <span class="mx-2 text-emerald-300">+{detail.stats.insertions}</span>
      <span class="text-red-300">-{detail.stats.deletions}</span>
    </div>

    <section class="space-y-3">
      {#each detail.diffs as file}
        <article class="card-surface overflow-hidden">
          <header class="border-b border-white/10 bg-black/20 px-4 py-2 text-xs text-white/70">
            <span class="uppercase text-white/50">{file.status}</span>
            <span class="mx-2">·</span>
            <span class="font-mono">{file.old_path ?? file.new_path}</span>
          </header>
          {#if file.is_binary}
            <p class="px-4 py-3 text-sm text-white/60">Binary file changed.</p>
          {:else}
            {#each file.hunks as hunk}
              <div class="border-t border-white/10">
                <div class="bg-black/30 px-4 py-1 font-mono text-xs text-primary">{hunk.header}</div>
                <pre class="overflow-x-auto p-3 font-mono text-xs leading-6">{#each hunk.lines as line}<div class={lineClass(line.line_type)}>{line.content}</div>{/each}</pre>
              </div>
            {/each}
          {/if}
        </article>
      {/each}
    </section>
  </section>
{/if}
