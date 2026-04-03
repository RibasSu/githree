<script lang="ts">
  import { api } from '$lib/api';
  import { formatDateTime } from '$lib/time';
  import type { CommitDetail } from '$lib/types';
  import { onMount } from 'svelte';
  import ShimmerRows from '$lib/components/ShimmerRows.svelte';

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
    if (lineType === 'add') return 'bg-[#033a16] text-[#aff5b4]';
    if (lineType === 'delete') return 'bg-[#490202] text-[#ffdcd7]';
    return 'text-[#c9d1d9]';
  }
</script>

{#if loading}
  <div class="space-y-3">
    <ShimmerRows rows={5} />
    <ShimmerRows rows={7} />
  </div>
{:else if detail === null}
  <div class="card-surface p-6 text-sm gt-muted">Commit not found.</div>
{:else}
  <section class="space-y-4">
    <a class="btn" href={`/${data.repo}/commits`}>Back to history</a>

    <article class="card-surface p-5">
      <h1 class="text-lg font-semibold text-[#f0f6fc]">{detail.commit.message_short}</h1>
      <p class="mt-2 text-sm gt-muted">{detail.commit.message}</p>

      <dl class="mt-4 grid gap-2 text-xs md:grid-cols-2">
        <div>
          <dt class="gt-muted">Hash</dt>
          <dd class="link-accent font-mono">{detail.commit.hash}</dd>
        </div>
        <div>
          <dt class="gt-muted">Author</dt>
          <dd>{detail.commit.author_name} ({detail.commit.author_email})</dd>
        </div>
        <div>
          <dt class="gt-muted">Date</dt>
          <dd>{formatDateTime(detail.commit.authored_at)}</dd>
        </div>
        <div>
          <dt class="gt-muted">Parents</dt>
          <dd class="font-mono">{detail.parents.join(', ')}</dd>
        </div>
      </dl>
    </article>

    <div class="card-surface p-4 text-xs">
      <span class="font-semibold text-[#f0f6fc]">Diff Stats: </span>
      <span class="gt-muted">{detail.stats.files_changed} files changed</span>
      <span class="mx-2 text-[#3fb950]">+{detail.stats.insertions}</span>
      <span class="text-[#da3633]">-{detail.stats.deletions}</span>
    </div>

    {#if detail.is_truncated}
      <div class="card-surface border-[#9e6a03] bg-[#3d2d00] px-4 py-3 text-sm text-[#f3c969]">
        {detail.truncated_reason || 'Diff is too large to display completely.'}
        <div class="mt-1 text-xs text-[#e6b450]">
          Displaying {detail.displayed_file_count} files and {detail.displayed_line_count} changed lines.
        </div>
      </div>
    {/if}

    <section class="space-y-3">
      {#each detail.diffs as file}
        <article class="card-surface overflow-hidden">
          <header class="border-b gt-divider bg-[#0d1117] px-4 py-2 text-xs gt-muted">
            <span class="uppercase">{file.status}</span>
            <span class="mx-2">·</span>
            <span class="font-mono text-[#c9d1d9]">{file.old_path ?? file.new_path}</span>
          </header>
          {#if file.is_binary}
            <p class="px-4 py-3 text-sm gt-muted">Binary file changed.</p>
          {:else}
            {#each file.hunks as hunk}
              <div class="border-t gt-divider">
                <div class="bg-[#161b22] px-4 py-1 font-mono text-xs link-accent">{hunk.header}</div>
                <pre class="overflow-x-auto p-3 font-mono text-xs leading-6">{#each hunk.lines as line}<div class={lineClass(line.line_type)}>{line.content}</div>{/each}</pre>
              </div>
            {/each}
          {/if}
        </article>
      {/each}
    </section>
  </section>
{/if}
