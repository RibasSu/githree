<script lang="ts">
  interface Props {
    source?: string;
    size?: number;
    alt?: string;
    className?: string;
  }

  let { source = 'generic', size = 16, alt = '', className = '' }: Props = $props();

  const normalizedSource = $derived(source.toLowerCase());
  const resolvedSource = $derived.by(() => {
    if (normalizedSource === 'github') return 'github';
    if (normalizedSource === 'gitlab') return 'gitlab';
    return 'generic';
  });

  const resolvedAlt = $derived.by(() => {
    if (alt.length > 0) return alt;
    if (resolvedSource === 'github') return 'GitHub';
    if (resolvedSource === 'gitlab') return 'GitLab';
    return 'Git';
  });
</script>

{#if resolvedSource === 'generic'}
  <img alt={resolvedAlt} class={className} height={size} src="/git-logo.svg" width={size} />
{:else}
  <span class={`gt-source-logo ${className}`} style={`--gt-source-logo-size:${size}px;`}>
    <img alt={resolvedAlt} class="gt-source-logo-light" height={size} src={`/${resolvedSource}-logo-white.svg`} width={size} />
    <img alt={resolvedAlt} class="gt-source-logo-dark" height={size} src={`/${resolvedSource}-logo-black.svg`} width={size} />
  </span>
{/if}

<style>
  .gt-source-logo {
    position: relative;
    display: inline-flex;
    width: var(--gt-source-logo-size);
    height: var(--gt-source-logo-size);
    flex-shrink: 0;
  }

  .gt-source-logo img {
    width: 100%;
    height: 100%;
  }

  .gt-source-logo-dark {
    display: none;
  }

  :global(html.light) .gt-source-logo-light,
  :global(html[data-theme='light']) .gt-source-logo-light {
    display: none;
  }

  :global(html.light) .gt-source-logo-dark,
  :global(html[data-theme='light']) .gt-source-logo-dark {
    display: block;
  }

  @media (prefers-color-scheme: light) {
    :global(html:not(.dark):not([data-theme='dark'])) .gt-source-logo-light {
      display: none;
    }

    :global(html:not(.dark):not([data-theme='dark'])) .gt-source-logo-dark {
      display: block;
    }
  }
</style>
