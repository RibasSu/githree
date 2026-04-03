<script lang="ts">
  import { GitBranch } from 'lucide-svelte';
  import type { RefsResponse } from '$lib/types';

  interface Props {
    refs: RefsResponse | null;
    selected: string;
    onSelect: (value: string) => void;
    compact?: boolean;
  }

  let { refs, selected, onSelect, compact = false }: Props = $props();

  function handleChange(event: Event) {
    const target = event.currentTarget as HTMLSelectElement;
    onSelect(target.value);
  }
</script>

{#if compact}
  <label class="gh-ref-select" aria-label="Git reference selector">
    <GitBranch size={14} />
    <select
      class="gh-ref-select-control"
      aria-label="Select git reference"
      value={selected}
      onchange={handleChange}
    >
      {#if refs}
        <optgroup label="Branches">
          {#each refs.branches as branch}
            <option value={branch}>{branch}</option>
          {/each}
        </optgroup>
        <optgroup label="Tags">
          {#each refs.tags as tag}
            <option value={tag}>{tag}</option>
          {/each}
        </optgroup>
      {:else}
        <option value={selected}>{selected}</option>
      {/if}
    </select>
  </label>
{:else}
  <label class="flex min-w-52 flex-col gap-1 text-xs gh-muted">
    Ref
    <select
      class="input text-sm normal-case"
      aria-label="Select git reference"
      value={selected}
      onchange={handleChange}
    >
      {#if refs}
        <optgroup label="Branches">
          {#each refs.branches as branch}
            <option value={branch}>{branch}</option>
          {/each}
        </optgroup>
        <optgroup label="Tags">
          {#each refs.tags as tag}
            <option value={tag}>{tag}</option>
          {/each}
        </optgroup>
      {:else}
        <option value={selected}>{selected}</option>
      {/if}
    </select>
  </label>
{/if}
