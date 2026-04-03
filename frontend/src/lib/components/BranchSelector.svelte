<script lang="ts">
  import type { RefsResponse } from '$lib/types';

  interface Props {
    refs: RefsResponse | null;
    selected: string;
    onSelect: (value: string) => void;
  }

  let { refs, selected, onSelect }: Props = $props();

  function handleChange(event: Event) {
    const target = event.currentTarget as HTMLSelectElement;
    onSelect(target.value);
  }
</script>

<label class="flex min-w-56 flex-col gap-2 text-xs uppercase tracking-wide text-white/60">
  Branch / Tag
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
