<script lang="ts">
  import { ERROR_CODE_CATALOG } from '$lib/error-codes';

  const commonHighlights = [404, 500];
</script>

<section class="space-y-6">
  <header class="card-surface p-5">
    <h1 class="text-xl font-semibold text-[#f0f6fc]">HTTP Error Reference</h1>
    <p class="mt-2 text-sm text-[#c9d1d9]">
      Quick guide for Githree error codes, including 404, 500, and other common failures.
    </p>
    <p class="mt-2 text-xs gt-muted">
      Use "Open page" to preview the actual global error screen for each status.
    </p>
  </header>

  <div class="card-surface overflow-x-auto">
    <table class="w-full min-w-[780px] border-collapse text-left text-sm">
      <thead class="bg-[#0d1117]">
        <tr class="border-b gt-divider">
          <th class="px-4 py-3 font-semibold text-[#f0f6fc]">Status</th>
          <th class="px-4 py-3 font-semibold text-[#f0f6fc]">Title</th>
          <th class="px-4 py-3 font-semibold text-[#f0f6fc]">Description</th>
          <th class="px-4 py-3 font-semibold text-[#f0f6fc]">Guidance</th>
          <th class="px-4 py-3 font-semibold text-[#f0f6fc]">Preview</th>
        </tr>
      </thead>
      <tbody>
        {#each ERROR_CODE_CATALOG as entry}
          <tr class="border-b gt-divider hover:bg-[#0d1117]">
            <td class="px-4 py-3">
              <code class="rounded border gt-divider bg-[#161b22] px-2 py-0.5 text-xs text-[#f0f6fc]">
                {entry.status}
              </code>
            </td>
            <td class="px-4 py-3 text-[#f0f6fc]">
              {entry.title}
              {#if commonHighlights.includes(entry.status)}
                <span class="gt-common-badge ml-2 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide">
                  common
                </span>
              {/if}
            </td>
            <td class="px-4 py-3 text-[#c9d1d9]">{entry.description}</td>
            <td class="px-4 py-3 text-[#8b949e]">{entry.guidance}</td>
            <td class="px-4 py-3">
              <a class="btn" href={`/errors/${entry.status}`}>Open page</a>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</section>
