<script lang="ts">
  import { onMount } from 'svelte';
  import { toasts, type ToastMessage } from '$lib/api';
  import { fly } from 'svelte/transition';

  let toastList = $state<ToastMessage[]>([]);

  onMount(() => {
    const unsubscribe = toasts.subscribe((value) => {
      toastList = value;
    });
    return unsubscribe;
  });
</script>

<div aria-live="polite" class="fixed right-4 top-4 z-50 flex w-[22rem] flex-col gap-2">
  {#each toastList as toast (toast.id)}
    <div
      class={`rounded-md border px-3 py-2 text-sm ${
        toast.type === 'error'
          ? 'border-[#da3633] bg-[#2d0b0b] text-[#ffdcd7]'
          : toast.type === 'success'
            ? 'border-[#f05032] bg-[#3d140e] text-[#ffd7ce]'
            : 'border-[var(--gh-accent)] bg-[color-mix(in_srgb,var(--gh-accent)_18%,#0d1117)] text-[#c9d1d9]'
      }`}
      in:fly={{ x: 30, duration: 180 }}
      out:fly={{ x: 30, duration: 150 }}
      role="status"
    >
      {toast.message}
    </div>
  {/each}
</div>
