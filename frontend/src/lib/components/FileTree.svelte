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
    <thead class="bg-white/5 text-xs uppercase tracking-wide text-white/60">
      <tr>
        <th class="px-4 py-3 text-left">Name</th>
        <th class="px-4 py-3 text-right">Size</th>
      </tr>
    </thead>
    <tbody>
      {#each sortedEntries as entry}
        <tr class="border-t border-white/5 hover:bg-white/5 focus-within:bg-white/10">
          <td class="px-4 py-3">
            <a
              aria-label={`Open ${entry.path}`}
              class="flex items-center gap-2 rounded outline-none focus:ring-2 focus:ring-primary/60"
              href={hrefFor(entry)}
            >
              {#if entry.entry_type === 'tree'}
                <Folder class="text-primary" size={16} />
              {:else if ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg'].includes(extension(entry.name))}
                <ImageIcon class="text-white/70" size={16} />
              {:else if ['zip', 'gz', 'tar'].includes(extension(entry.name))}
                <FileArchive class="text-white/70" size={16} />
              {:else if ['md', 'txt'].includes(extension(entry.name))}
                <FileText class="text-white/70" size={16} />
              {:else}
                <FileCode2 class="text-white/70" size={16} />
              {/if}
              <span class="truncate">{entry.name}</span>
            </a>
          </td>
          <td class="px-4 py-3 text-right text-white/50">{sizeLabel(entry.size)}</td>
        </tr>
      {/each}
      {#if sortedEntries.length === 0}
        <tr class="border-t border-white/5">
          <td class="px-4 py-6 text-sm text-white/60" colspan="2">No entries found in this directory.</td>
        </tr>
      {/if}
    </tbody>
  </table>
</div>
