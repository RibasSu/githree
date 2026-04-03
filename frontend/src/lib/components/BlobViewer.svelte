<script lang="ts">
  import { codeToHtml } from 'shiki';
  import DOMPurify from 'dompurify';
  import { highlightMarkdownCodeBlocks } from '$lib/markdown';
  import { marked } from 'marked';
  import type { BlobResponse } from '$lib/types';

  interface Props {
    blob: BlobResponse;
    filePath: string;
    rawUrl: string;
    repo?: string;
    refName?: string;
  }

  type RenderMode = 'code' | 'markdown' | 'binary' | 'truncated';

  const MAX_RENDERABLE_LINES = 20_000;

  let { blob, filePath, rawUrl, repo = '', refName = '' }: Props = $props();
  let highlightedHtml = $state('');
  let markdownHtml = $state('');
  let plainContent = $state('');
  let renderError = $state<string | null>(null);
  let renderMode = $state<RenderMode>('code');

  const lineCount = $derived(
    plainContent.length > 0 ? plainContent.split('\n').length : 0
  );

  $effect(() => {
    const snapshot = {
      content: blob.content,
      encoding: blob.encoding,
      size: blob.size,
      language: blob.language,
      isBinary: blob.is_binary,
      isTruncated: blob.is_truncated,
      truncatedReason: blob.truncated_reason || '',
      filePath
    };
    void renderBlob(snapshot);
  });

  async function renderBlob(snapshot: {
    content: string;
    encoding: string;
    size: number;
    language: string;
    isBinary: boolean;
    isTruncated: boolean;
    truncatedReason: string;
    filePath: string;
  }) {
    renderError = null;
    highlightedHtml = '';
    markdownHtml = '';
    plainContent = '';

    if (snapshot.isTruncated) {
      renderMode = 'truncated';
      renderError = snapshot.truncatedReason || 'File is too large to render in the browser.';
      return;
    }

    if (snapshot.isBinary) {
      renderMode = 'binary';
      return;
    }

    plainContent = decodeText(snapshot.content, snapshot.encoding);

    if (plainContent.split('\n').length > MAX_RENDERABLE_LINES) {
      renderMode = 'truncated';
      renderError = `File has more than ${MAX_RENDERABLE_LINES} lines. Download the raw file to inspect it.`;
      return;
    }

    if (isMarkdownFile(snapshot.filePath, snapshot.language)) {
      renderMode = 'markdown';
      try {
        const rendered = await marked.parse(plainContent);
        const rewritten = rewriteMarkdownLinks(rendered);
        const highlighted = await highlightMarkdownCodeBlocks(rewritten);
        const sanitized = DOMPurify.sanitize(highlighted);
        markdownHtml = sanitized.trim().length > 0
          ? sanitized
          : `<pre>${escapeHtml(plainContent)}</pre>`;
        if (sanitized.trim().length === 0 && plainContent.trim().length > 0) {
          renderError = 'Markdown renderer returned empty output; showing plain text fallback.';
        }
      } catch {
        renderError = 'Falling back to plain text because markdown rendering failed.';
        markdownHtml = `<pre>${escapeHtml(plainContent)}</pre>`;
      }
      return;
    }

    renderMode = 'code';
    try {
      const html = await codeToHtml(plainContent, {
        lang: snapshot.language || 'text',
        theme: 'github-dark'
      });

      highlightedHtml = html.trim().length > 0 ? html : `<pre>${escapeHtml(plainContent)}</pre>`;
    } catch {
      renderError = 'Falling back to plain text because syntax highlighting failed.';
      highlightedHtml = `<pre>${escapeHtml(plainContent)}</pre>`;
    }
  }

  function decodeText(content: string, encoding: string): string {
    if (encoding === 'base64' && typeof window !== 'undefined') {
      try {
        const binary = atob(content);
        const bytes = Uint8Array.from(binary, (ch) => ch.charCodeAt(0));
        return new TextDecoder().decode(bytes);
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
      const suffix = hashFragment ? `#${hashFragment}` : '';

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

  function isMarkdownFile(path: string, language: string): boolean {
    const lowered = path.toLowerCase();
    return (
      language === 'markdown' ||
      lowered.endsWith('.md') ||
      lowered.endsWith('.markdown')
    );
  }

  function isImageBlob(value: BlobResponse): boolean {
    return Boolean(value.mime && value.mime.startsWith('image/'));
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
      <button
        class="btn"
        disabled={plainContent.length === 0}
        onclick={() => navigator.clipboard.writeText(plainContent)}
        type="button"
      >
        Copy
      </button>
      <a class="btn btn-primary" href={rawUrl}>Download raw</a>
      <button class="btn" disabled type="button">View blame (soon)</button>
    </div>
  </header>

  {#if renderMode === 'truncated'}
    <div class="space-y-3 p-6">
      <p class="text-sm text-[#d29922]">{renderError || 'This file is too large to display in the browser.'}</p>
      <div>
        <a class="btn btn-primary" href={rawUrl}>Download raw file</a>
      </div>
    </div>
  {:else if renderMode === 'binary'}
    <div class="p-6">
      {#if isImageBlob(blob) && blob.content.length > 0}
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
  {:else if renderMode === 'markdown'}
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

  {#if renderError && renderMode !== 'truncated'}
    <div class="border-t gh-divider px-4 py-2 text-xs text-[#d29922]">{renderError}</div>
  {/if}
</section>
