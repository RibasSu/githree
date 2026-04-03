<script lang="ts">
  interface Props {
    repo: string;
    path: string;
    mode: 'tree' | 'blob';
    refName: string;
  }

  let { repo, path, mode, refName }: Props = $props();
  const segments = $derived(path.split('/').filter((segment) => segment.length > 0));

  const crumbs = $derived.by(() => {
    const output: Array<{ name: string; href: string; isLast: boolean }> = [];
    let current = '';
    for (let i = 0; i < segments.length; i += 1) {
      current = current ? `${current}/${segments[i]}` : segments[i];
      const isLast = i === segments.length - 1;
      const routeMode = isLast ? mode : 'tree';
      output.push({
        name: segments[i],
        href: `/${repo}/${routeMode}/${current}?ref=${encodeURIComponent(refName)}`,
        isLast
      });
    }
    return output;
  });
</script>

<nav aria-label="Breadcrumb" class="text-xs gt-muted">
  <ol class="flex flex-wrap items-center gap-2">
    <li>
      <a class="gt-crumb-link" href={`/${repo}?ref=${encodeURIComponent(refName)}`}>{repo}</a>
    </li>
    {#each crumbs as crumb}
      <li class="text-[#6e7681]">/</li>
      <li>
        {#if crumb.isLast}
          <span class="text-[#c9d1d9]">{crumb.name}</span>
        {:else}
          <a class="gt-crumb-link" href={crumb.href}>{crumb.name}</a>
        {/if}
      </li>
    {/each}
  </ol>
</nav>
