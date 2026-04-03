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
      class={`rounded-lg border px-3 py-2 text-sm shadow-glow ${
        toast.type === 'error'
          ? 'border-red-400/30 bg-red-500/15 text-red-100'
          : toast.type === 'success'
            ? 'border-emerald-400/30 bg-emerald-500/15 text-emerald-100'
            : 'border-primary/40 bg-primary/20 text-white'
      }`}
      in:fly={{ x: 30, duration: 180 }}
      out:fly={{ x: 30, duration: 150 }}
      role="status"
    >
      {toast.message}
    </div>
  {/each}
</div>
