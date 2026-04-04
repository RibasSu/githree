<script lang="ts">
  import { getErrorDefinition } from '$lib/error-codes';

  interface Props {
    status: number;
    error: unknown;
  }

  let { status }: Props = $props();

  const definition = $derived(getErrorDefinition(status));
  const title = $derived(definition?.title ?? 'Unexpected Error');
  const description = $derived(
    definition?.description ?? 'Something went wrong while rendering this page.'
  );
  const guidance = $derived(
    definition?.guidance ?? 'Try refreshing the page. If this persists, check backend logs.'
  );
</script>

<section class="card-surface mx-auto max-w-3xl overflow-hidden">
  <header class="border-b gt-divider bg-[#0d1117] px-6 py-5">
    <p class="text-xs font-semibold uppercase tracking-wider text-[#8b949e]">Request error</p>
    <h1 class="mt-2 text-2xl font-semibold text-[#f0f6fc]">
      {status} · {title}
    </h1>
    <p class="mt-3 text-sm text-[#c9d1d9]">{description}</p>
  </header>

  <div class="space-y-5 px-6 py-5">
    <div class="rounded-md border gt-divider bg-[#0d1117] p-4">
      <h2 class="text-sm font-semibold text-[#f0f6fc]">What to do next</h2>
      <p class="mt-2 text-sm text-[#c9d1d9]">{guidance}</p>
    </div>

    <div class="flex flex-wrap items-center gap-2">
      <a class="btn btn-primary" href="/">Go home</a>
      <button class="btn" onclick={() => window.history.back()} type="button">Go back</button>
    </div>
  </div>
</section>
