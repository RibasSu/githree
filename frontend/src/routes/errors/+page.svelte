<script lang="ts">
  import { ERROR_CODE_CATALOG } from '$lib/error-codes';

  const commonHighlights = [404, 500];
</script>

<section class="space-y-4 sm:space-y-6">
  <header class="card-surface p-4 sm:p-5">
    <h1 class="text-lg font-semibold text-[#f0f6fc] sm:text-xl">HTTP Error Reference</h1>
    <p class="mt-2 text-sm text-[#c9d1d9]">
      Quick guide for Githree error codes, including 404, 500, and other common failures.
    </p>
    <p class="mt-2 text-xs gt-muted">
      Use "Open page" to preview the actual global error screen for each status.
    </p>
  </header>

  <div class="grid gap-3 md:hidden">
    {#each ERROR_CODE_CATALOG as entry}
      <article class="card-surface p-4">
        <div class="flex flex-wrap items-center gap-2">
          <code class="rounded border gt-divider bg-[#161b22] px-2 py-0.5 text-xs text-[#f0f6fc]">
            {entry.status}
          </code>
          <h2 class="text-sm font-semibold text-[#f0f6fc]">{entry.title}</h2>
          {#if commonHighlights.includes(entry.status)}
            <span class="gt-common-badge rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide">
              common
            </span>
          {/if}
        </div>

        <p class="mt-3 text-sm text-[#c9d1d9]">{entry.description}</p>
        <p class="mt-2 text-sm text-[#8b949e]">{entry.guidance}</p>

        <div class="mt-3">
          <a class="btn w-full justify-center" href={`/errors/${entry.status}`}>Open page</a>
        </div>
      </article>
    {/each}
  </div>

  <div class="hidden card-surface overflow-x-auto md:block">
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
