<script lang="ts">
  import '../app.css';
  import ToastStack from '$lib/components/ToastStack.svelte';

  let { children } = $props();
</script>

<a class="sr-only focus:not-sr-only focus:absolute focus:left-3 focus:top-3 focus:z-50 focus:rounded-md focus:bg-[#1f6feb] focus:px-3 focus:py-2 focus:text-white" href="#main-content">
  Skip to content
</a>

<div class="min-h-screen">
  <header class="border-b gh-divider bg-[#010409]">
    <div class="repo-shell flex items-center justify-between px-4 py-3">
      <a class="flex items-center gap-3 text-sm font-semibold text-[#f0f6fc]" href="/">
        <img alt="Githree logo" class="h-5 w-5" height="20" src="/logo.svg" width="20" />
        <span>Githree</span>
      </a>
      <nav aria-label="Global" class="hidden items-center gap-4 text-sm md:flex">
        <a class="link-muted hover:text-[#c9d1d9]" href="/">Repositories</a>
        <a class="link-muted hover:text-[#c9d1d9]" href="/errors">Errors</a>
        <a class="link-muted hover:text-[#c9d1d9]" href="https://githree.org" rel="noreferrer" target="_blank">
          githree.org
        </a>
      </nav>
    </div>
  </header>

  <main class="repo-shell px-4 py-6" id="main-content">
    {@render children?.()}
  </main>

  <footer class="mt-8 border-t gh-divider bg-[#010409]">
    <div class="repo-shell px-4 py-4 text-center text-sm gh-muted">
      Made with 🧡 by
      <a class="link-accent hover:underline" href="https://githree.org/" rel="noreferrer" target="_blank">
        Githree
      </a>
    </div>
  </footer>
</div>

<ToastStack />
