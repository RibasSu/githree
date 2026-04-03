<script lang="ts">
  import '../app.css';
  import ToastStack from '$lib/components/ToastStack.svelte';

  let { children } = $props();
</script>

<a class="sr-only focus:not-sr-only focus:absolute focus:left-3 focus:top-3 focus:z-50 focus:rounded focus:bg-black focus:px-3 focus:py-2" href="#main-content">
  Skip to content
</a>

<div class="min-h-screen text-white">
  <header class="border-b border-white/10 bg-black/30 backdrop-blur">
    <div class="mx-auto flex max-w-7xl items-center justify-between px-4 py-4">
      <a class="flex items-center gap-3" href="/">
        <div class="h-8 w-8 rounded-md bg-primary/30 ring-1 ring-primary/60"></div>
        <div>
          <p class="text-base font-semibold tracking-wide">Githree</p>
          <p class="text-xs text-white/60">Browse any git repository. No login required.</p>
        </div>
      </a>
      <a class="btn" href="https://githree.org" rel="noreferrer" target="_blank">githree.org</a>
    </div>
  </header>

  <main class="mx-auto max-w-7xl px-4 py-6" id="main-content">
    {@render children?.()}
  </main>
</div>

<ToastStack />
