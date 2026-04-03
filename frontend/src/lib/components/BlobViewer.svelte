<script lang="ts">
  import { codeToHtml } from 'shiki';
  import type { BlobResponse } from '$lib/types';

  interface Props {
    blob: BlobResponse;
    filePath: string;
    rawUrl: string;
  }

  let { blob, filePath, rawUrl }: Props = $props();
  let highlightedHtml = $state('');
  let plainContent = $state('');
  let renderError = $state<string | null>(null);

  const isImage = $derived(Boolean(blob.mime && blob.mime.startsWith('image/')));
  const lineCount = $derived(
    plainContent.length > 0 ? plainContent.split('\n').length : 0
  );

  $effect(() => {
    void renderBlob();
  });

  async function renderBlob() {
    renderError = null;
    highlightedHtml = '';
    plainContent = '';

    if (blob.is_binary) {
      return;
    }

    plainContent = decodeText(blob.content, blob.encoding);

    try {
      highlightedHtml = await codeToHtml(plainContent, {
        lang: blob.language || 'text',
        theme: 'github-dark'
      });
    } catch {
      renderError = 'Falling back to plain text because syntax highlighting failed.';
      highlightedHtml = `<pre>${escapeHtml(plainContent)}</pre>`;
    }
  }

  function decodeText(content: string, encoding: string): string {
    if (encoding === 'base64' && typeof window !== 'undefined') {
      try {
        return atob(content);
      } catch {
        return content;
      }
    }
    return content;
  }

  function lineNumbers(count: number): number[] {
    return Array.from({ length: count }, (_, index) => index + 1);
  }

  function escapeHtml(value: string): string {
    return value
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;')
      .replaceAll('"', '&quot;')
      .replaceAll("'", '&#039;');
  }
</script>

<section class="card-surface overflow-hidden">
  <header class="flex flex-wrap items-center justify-between gap-3 border-b border-white/10 px-4 py-3">
    <div>
      <h2 class="text-sm font-semibold text-white">{filePath}</h2>
      <p class="text-xs text-white/60">
        {blob.language} · {blob.size} bytes · {lineCount} lines
      </p>
    </div>
    <div class="flex flex-wrap items-center gap-2">
      <button class="btn" onclick={() => navigator.clipboard.writeText(plainContent)} type="button">
        Copy
      </button>
      <a class="btn btn-primary" href={rawUrl}>Download raw</a>
      <button class="btn" disabled type="button">View blame (soon)</button>
    </div>
  </header>

  {#if blob.is_binary}
    <div class="p-6">
      {#if isImage}
        <img
          alt={`Preview for ${filePath}`}
          class="max-h-[28rem] w-auto rounded-lg border border-white/10"
          src={`data:${blob.mime || 'image/png'};base64,${blob.content}`}
        />
      {:else}
        <p class="text-sm text-white/70">
          Binary file detected ({blob.mime || 'application/octet-stream'}). Download the raw file to inspect it.
        </p>
      {/if}
    </div>
  {:else}
    <div class="grid grid-cols-[56px_1fr] overflow-x-auto bg-code font-mono text-xs">
      <div aria-hidden="true" class="select-none border-r border-white/10 px-3 py-4 text-right text-white/40">
        {#each lineNumbers(lineCount) as lineNo}
          <div class="leading-6">{lineNo}</div>
        {/each}
      </div>
      <div class="p-4 text-white/90">
        {@html highlightedHtml}
      </div>
    </div>
    {#if renderError}
      <div class="border-t border-white/10 px-4 py-2 text-xs text-amber-300">{renderError}</div>
    {/if}
  {/if}
</section>
