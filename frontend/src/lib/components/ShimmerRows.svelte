<script lang="ts">
  interface Props {
    rows?: number;
    lineHeightClass?: string;
  }

  let { rows = 5, lineHeightClass = 'h-4' }: Props = $props();

  const widths = ['w-full', 'w-11/12', 'w-10/12', 'w-9/12', 'w-8/12'];
</script>

<div class="card-surface p-4" role="status" aria-live="polite" aria-label="Loading content">
  <div class="space-y-2">
    {#each Array.from({ length: rows }) as _, idx}
      <div class={`gh-shimmer ${lineHeightClass} ${widths[idx % widths.length]} rounded-sm`}></div>
    {/each}
  </div>
</div>
