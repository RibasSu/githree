<script lang="ts">
  import { codeToHtml } from 'shiki';
  import DOMPurify from 'dompurify';
  import { marked } from 'marked';
  import type { BlobResponse } from '$lib/types';

  interface Props {
    blob: BlobResponse;
    filePath: string;
    rawUrl: string;
    repo?: string;
    refName?: string;
  }

  let { blob, filePath, rawUrl, repo = '', refName = '' }: Props = $props();
  let highlightedHtml = $state('');
  let markdownHtml = $state('');
  let plainContent = $state('');
  let renderError = $state<string | null>(null);

  const isImage = $derived(Boolean(blob.mime && blob.mime.startsWith('image/')));
  const isMarkdown = $derived(
    blob.language === 'markdown' ||
      filePath.toLowerCase().endsWith('.md') ||
      filePath.toLowerCase().endsWith('.markdown')
  );
  const lineCount = $derived(
    plainContent.length > 0 ? plainContent.split('\n').length : 0
  );

  $effect(() => {
    void renderBlob();
  });

  async function renderBlob() {
    renderError = null;
    highlightedHtml = '';
    markdownHtml = '';
    plainContent = '';

    if (blob.is_binary) {
      return;
    }

    plainContent = decodeText(blob.content, blob.encoding);

    if (isMarkdown) {
      try {
        const rendered = await marked.parse(plainContent);
        const rewritten = rewriteMarkdownLinks(rendered);
        markdownHtml = DOMPurify.sanitize(rewritten);
      } catch {
        renderError = 'Falling back to plain text because markdown rendering failed.';
        markdownHtml = `<pre>${escapeHtml(plainContent)}</pre>`;
      }
      return;
    }

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

  function rewriteMarkdownLinks(html: string): string {
    if (typeof window === 'undefined') return html;
    if (repo.length === 0 || refName.length === 0) return html;

    const document = new DOMParser().parseFromString(html, 'text/html');
    const anchors = document.querySelectorAll('a[href]');
    const fileDirectory = filePath.includes('/') ? filePath.slice(0, filePath.lastIndexOf('/')) : '';

    for (const anchor of anchors) {
      const href = anchor.getAttribute('href')?.trim();
      if (!href) continue;
      if (href.startsWith('#')) continue;
      if (isExternalHref(href)) continue;

      const [hrefWithoutHash, hashFragment] = href.split('#', 2);
      if (hrefWithoutHash.length === 0) continue;
      if (hrefWithoutHash.startsWith('?')) continue;

      const baseUrl = new URL(`https://repo.local/${fileDirectory.length > 0 ? `${fileDirectory}/` : ''}`);
      const resolved = new URL(hrefWithoutHash, baseUrl);
      const repoPath = resolved.pathname.replace(/^\/+/, '').replace(/\/+$/, '');
      if (repoPath.length === 0) continue;

      const mode: 'blob' | 'tree' = hrefWithoutHash.endsWith('/') ? 'tree' : 'blob';
      const encodedPath = repoPath
        .split('/')
        .filter((segment) => segment.length > 0)
        .map((segment) => encodeURIComponent(segment))
        .join('/');
      const suffix = hashFragment ? `#${encodeURIComponent(hashFragment)}` : '';

      anchor.setAttribute(
        'href',
        `/${repo}/${mode}/${encodedPath}?ref=${encodeURIComponent(refName)}${suffix}`
      );
    }

    return document.body.innerHTML;
  }

  function isExternalHref(href: string): boolean {
    return /^(https?:|mailto:|tel:|data:)/i.test(href);
  }
</script>

<section class="card-surface overflow-hidden">
  <header class="flex flex-wrap items-center justify-between gap-3 border-b gh-divider px-4 py-3">
    <div>
      <h2 class="text-sm font-semibold text-[#f0f6fc]">{filePath}</h2>
      <p class="text-xs gh-muted">
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
          class="max-h-[28rem] w-auto rounded-md border gh-divider bg-[#0d1117]"
          src={`data:${blob.mime || 'image/png'};base64,${blob.content}`}
        />
      {:else}
        <p class="text-sm gh-muted">
          Binary file detected ({blob.mime || 'application/octet-stream'}). Download the raw file to inspect it.
        </p>
      {/if}
    </div>
  {:else}
    {#if isMarkdown}
      <article class="github-markdown p-5">
        {@html markdownHtml}
      </article>
    {:else}
      <div class="grid grid-cols-[56px_1fr] overflow-x-auto bg-[#0d1117] font-mono text-xs">
        <div aria-hidden="true" class="select-none border-r gh-divider px-3 py-4 text-right gh-muted">
          {#each lineNumbers(lineCount) as lineNo}
            <div class="leading-6">{lineNo}</div>
          {/each}
        </div>
        <div class="p-4 text-[#c9d1d9]">
          {@html highlightedHtml}
        </div>
      </div>
    {/if}
    {#if renderError}
      <div class="border-t gh-divider px-4 py-2 text-xs text-[#d29922]">{renderError}</div>
    {/if}
  {/if}
</section>
