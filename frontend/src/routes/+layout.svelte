<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import type { AppSettings } from '$lib/types';
  import ToastStack from '$lib/components/ToastStack.svelte';

  let { children } = $props();
  let settings = $state<AppSettings | null>(null);

  const appName = $derived(settings?.app_name?.trim() || 'Githree');
  const logoUrl = $derived(settings?.logo_url?.trim() || '/logo.svg');
  const siteUrl = $derived(settings?.site_url?.trim() || 'https://githree.org');
  const domainLabel = $derived.by(() => {
    const configuredDomain = settings?.domain?.trim();
    if (configuredDomain) return configuredDomain;
    try {
      return new URL(siteUrl).host;
    } catch {
      return siteUrl.replace(/^https?:\/\//i, '');
    }
  });

  onMount(() => {
    void loadSettings();
  });

  async function loadSettings() {
    try {
      settings = await api.getSettings();
    } catch {
      settings = {
        web_repo_management: false,
        repos_dir: './data/repos',
        registry_file: './data/repos.json',
        app_name: 'Githree',
        logo_url: '/logo.svg',
        site_url: 'https://githree.org',
        domain: 'githree.org',
        caddy_enabled: false
      };
    }
  }
</script>

<a class="sr-only focus:not-sr-only focus:absolute focus:left-3 focus:top-3 focus:z-50 focus:rounded-md focus:bg-[#58a6ff] focus:px-3 focus:py-2 focus:text-[#0d1117]" href="#main-content">
  Skip to content
</a>

<div class="flex min-h-screen flex-col">
  <header class="border-b gt-divider bg-[#010409]">
    <div class="repo-shell flex items-center justify-between px-4 py-3">
      <a class="flex items-center gap-3 text-sm font-semibold text-[#f0f6fc]" href="/">
        <img alt={`${appName} logo`} class="h-5 w-5" height="20" src={logoUrl} width="20" />
        <span>{appName}</span>
      </a>
      <nav aria-label="Global" class="hidden items-center gap-4 text-sm md:flex">
        <a class="link-muted" href="/">Repositories</a>
        <a class="link-muted" href={siteUrl} rel="noreferrer" target="_blank">
          {domainLabel}
        </a>
      </nav>
    </div>
  </header>

  <main class="w-full flex-1" id="main-content">
    <div class="repo-shell px-4 py-6">
      {@render children?.()}
    </div>
  </main>

  <footer class="border-t gt-divider bg-[#010409]">
    <div class="repo-shell px-4 py-4 text-center text-sm gt-muted">
      Made with 🧡 by
      <a class="link-accent hover:underline" href={siteUrl} rel="noreferrer" target="_blank">
        {appName}
      </a>
    </div>
  </footer>
</div>

<ToastStack />
