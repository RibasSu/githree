<script lang="ts">
  import { FileArchive, FileCode2, FileText, Folder, Image as ImageIcon } from 'lucide-svelte';
  import type { TreeEntry } from '$lib/types';

  interface Props {
    repo: string;
    refName: string;
    entries: TreeEntry[];
  }

  let { repo, refName, entries }: Props = $props();

  const sortedEntries = $derived(
    [...entries].sort((a, b) => {
      if (a.entry_type === b.entry_type) return a.name.localeCompare(b.name);
      return a.entry_type === 'tree' ? -1 : 1;
    })
  );

  function hrefFor(entry: TreeEntry): string {
    if (entry.entry_type === 'tree') {
      return `/${repo}/tree/${entry.path}?ref=${encodeURIComponent(refName)}`;
    }
    return `/${repo}/blob/${entry.path}?ref=${encodeURIComponent(refName)}`;
  }

  function sizeLabel(size: number | null): string {
    if (size === null) return '';
    if (size < 1024) return `${size} B`;
    if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
    return `${(size / (1024 * 1024)).toFixed(1)} MB`;
  }

  function extension(name: string): string {
    const value = name.split('.').pop();
    return value ? value.toLowerCase() : '';
  }
</script>

<div aria-label="Repository file tree" class="card-surface overflow-hidden">
  <table class="w-full text-sm">
    <thead class="bg-[#161b22] text-xs uppercase tracking-wide gh-muted">
      <tr>
        <th class="px-4 py-3 text-left">Name</th>
        <th class="px-4 py-3 text-right">Size</th>
      </tr>
    </thead>
    <tbody>
      {#each sortedEntries as entry}
        <tr class="border-t gh-divider hover:bg-[#161b22] focus-within:bg-[#161b22]">
          <td class="px-4 py-3">
            <a
              aria-label={`Open ${entry.path}`}
              class="flex items-center gap-2 rounded-sm outline-none focus:ring-2 focus:ring-[#2f81f7]/70"
              href={hrefFor(entry)}
            >
              {#if entry.entry_type === 'tree'}
                <Folder class="text-[#8b949e]" size={16} />
              {:else if ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg'].includes(extension(entry.name))}
                <ImageIcon class="text-[#8b949e]" size={16} />
              {:else if ['zip', 'gz', 'tar'].includes(extension(entry.name))}
                <FileArchive class="text-[#8b949e]" size={16} />
              {:else if ['md', 'txt'].includes(extension(entry.name))}
                <FileText class="text-[#8b949e]" size={16} />
              {:else}
                <FileCode2 class="text-[#8b949e]" size={16} />
              {/if}
              <span class="truncate text-[#c9d1d9] hover:underline">{entry.name}</span>
            </a>
          </td>
          <td class="px-4 py-3 text-right gh-muted">{sizeLabel(entry.size)}</td>
        </tr>
      {/each}
      {#if sortedEntries.length === 0}
        <tr class="border-t gh-divider">
          <td class="px-4 py-6 text-sm gh-muted" colspan="2">No entries found in this directory.</td>
        </tr>
      {/if}
    </tbody>
  </table>
</div>
