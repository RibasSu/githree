<script lang="ts">
  import type { CommitInfo } from '$lib/types';
  import { formatRelativeTime } from '$lib/time';

  interface Props {
    repo: string;
    commits: CommitInfo[];
  }

  let { repo, commits }: Props = $props();
  let filter = $state('');

  const filtered = $derived(
    commits.filter((commit) => {
      const needle = filter.trim().toLowerCase();
      if (needle.length === 0) return true;
      return (
        commit.author_name.toLowerCase().includes(needle) ||
        commit.author_email.toLowerCase().includes(needle) ||
        commit.message.toLowerCase().includes(needle) ||
        commit.hash.toLowerCase().includes(needle)
      );
    })
  );

  function avatarUrl(email: string): string {
    const hash = pseudoHash(email.trim().toLowerCase());
    return `https://www.gravatar.com/avatar/${hash}?d=identicon&s=64`;
  }

  function pseudoHash(value: string): string {
    let h1 = 0xdeadbeef ^ value.length;
    let h2 = 0x41c6ce57 ^ value.length;
    for (let i = 0; i < value.length; i += 1) {
      const char = value.charCodeAt(i);
      h1 = Math.imul(h1 ^ char, 2654435761);
      h2 = Math.imul(h2 ^ char, 1597334677);
    }
    h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507) ^ Math.imul(h2 ^ (h2 >>> 13), 3266489909);
    h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507) ^ Math.imul(h1 ^ (h1 >>> 13), 3266489909);
    const combined = (4294967296 * (2097151 & h2) + (h1 >>> 0)).toString(16);
    return combined.padStart(32, '0').slice(0, 32);
  }
</script>

<section class="space-y-3">
  <label class="sr-only" for="commit-filter">Filter commits</label>
  <input
    id="commit-filter"
    bind:value={filter}
    class="input"
    placeholder="Filter by author or commit message"
    type="text"
  />

  <div class="card-surface divide-y divide-[#30363d] overflow-hidden">
    {#each filtered as commit}
      <a
        class="grid grid-cols-[auto_1fr_auto] items-center gap-3 px-4 py-3 hover:bg-[#161b22]"
        href={`/${repo}/commit/${commit.hash}`}
      >
        <img
          alt={commit.author_name}
          class="h-8 w-8 rounded-full border gh-divider"
          loading="lazy"
          src={avatarUrl(commit.author_email)}
        />
        <div class="min-w-0">
          <div class="flex flex-wrap items-center gap-2">
            <code class="rounded-sm border gh-divider bg-[#0d1117] px-1.5 py-0.5 font-mono text-xs text-[#2f81f7]">
              {commit.short_hash}
            </code>
            <span class="truncate text-sm text-[#c9d1d9]">{commit.message_short}</span>
          </div>
          <p class="truncate text-xs gh-muted">
            {commit.author_name} · {commit.author_email}
          </p>
        </div>
        <time class="text-xs gh-muted">{formatRelativeTime(commit.authored_at)}</time>
      </a>
    {/each}
    {#if filtered.length === 0}
      <p class="px-4 py-6 text-sm gh-muted">No commits match your filter.</p>
    {/if}
  </div>
</section>
