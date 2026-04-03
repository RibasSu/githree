<script lang="ts">
  import { onMount } from 'svelte';
  import { Check, ChevronDown, GitBranch, Search, Tag, X } from 'lucide-svelte';
  import type { RefsResponse } from '$lib/types';

  interface Props {
    refs: RefsResponse | null;
    selected: string;
    onSelect: (value: string) => void;
    compact?: boolean;
    repoName?: string;
  }

  let { refs, selected, onSelect, compact = false, repoName = '' }: Props = $props();

  let rootEl = $state<HTMLElement | null>(null);
  let open = $state(false);
  let search = $state('');
  let activeTab = $state<'branches' | 'tags'>('branches');

  const branches = $derived(refs?.branches ?? []);
  const tags = $derived(refs?.tags ?? []);
  const normalizedSearch = $derived(search.trim().toLowerCase());

  const filteredBranches = $derived(
    normalizedSearch.length === 0
      ? branches
      : branches.filter((branch) => branch.toLowerCase().includes(normalizedSearch))
  );

  const filteredTags = $derived(
    normalizedSearch.length === 0 ? tags : tags.filter((tag) => tag.toLowerCase().includes(normalizedSearch))
  );

  const hasAnyRefs = $derived(branches.length > 0 || tags.length > 0);
  const showViewAllBranches = $derived(activeTab === 'branches' && branches.length > 0);
  const viewAllBranchesHref = $derived.by(() =>
    repoName.length > 0 ? `/${repoName}/branches?ref=${encodeURIComponent(selected)}` : ''
  );

  $effect(() => {
    if (refs?.tags.includes(selected) && !refs?.branches.includes(selected)) {
      activeTab = 'tags';
      return;
    }
    activeTab = 'branches';
  });

  onMount(() => {
    const handlePointerDown = (event: MouseEvent) => {
      if (!open || !rootEl) return;
      const target = event.target;
      if (target instanceof Node && !rootEl.contains(target)) {
        closeMenu();
      }
    };

    document.addEventListener('mousedown', handlePointerDown);
    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
    };
  });

  function setDefaultTabFromSelected() {
    if (refs?.tags.includes(selected) && !refs?.branches.includes(selected)) {
      activeTab = 'tags';
      return;
    }
    activeTab = 'branches';
  }

  function toggleMenu() {
    open = !open;
    if (open) {
      search = '';
      setDefaultTabFromSelected();
    }
  }

  function closeMenu() {
    open = false;
    search = '';
  }

  function selectRef(value: string) {
    onSelect(value);
    closeMenu();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      closeMenu();
    }
  }

  function handleSearchKeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    const candidate = activeTab === 'branches' ? filteredBranches[0] : filteredTags[0];
    if (!candidate) return;
    selectRef(candidate);
  }
</script>

<div bind:this={rootEl} class={`gt-ref-switcher ${compact ? 'compact' : ''}`}>
  {#if !compact}
    <span class="gt-ref-switcher-label">Ref</span>
  {/if}

  <button
    aria-controls="gt-ref-menu"
    aria-expanded={open}
    class="gt-ref-switcher-trigger"
    onclick={toggleMenu}
    type="button"
  >
    <GitBranch size={14} />
    <span class="truncate">{selected}</span>
    <ChevronDown class={`gt-ref-chevron ${open ? 'open' : ''}`} size={14} />
  </button>

  {#if open}
    <div
      class="gt-ref-menu"
      id="gt-ref-menu"
      role="dialog"
      aria-label="Switch branches and tags"
      tabindex="-1"
      onkeydown={handleKeydown}
    >
      <div class="gt-ref-menu-header">
        <strong>Switch branches/tags</strong>
        <button aria-label="Close branch switcher" class="gt-ref-close" onclick={closeMenu} type="button">
          <X size={14} />
        </button>
      </div>

      <label class="gt-ref-search" for="gt-ref-search-input">
        <Search class="gt-muted" size={14} />
        <input
          bind:value={search}
          id="gt-ref-search-input"
          onkeydown={handleSearchKeydown}
          placeholder="Find a branch..."
          type="text"
        />
      </label>

      <div class="gt-ref-tabs" role="tablist" aria-label="Reference type">
        <button
          aria-selected={activeTab === 'branches'}
          class={`gt-ref-tab ${activeTab === 'branches' ? 'active' : ''}`}
          onclick={() => {
            activeTab = 'branches';
            search = '';
          }}
          role="tab"
          type="button"
        >
          Branches
        </button>
        <button
          aria-selected={activeTab === 'tags'}
          class={`gt-ref-tab ${activeTab === 'tags' ? 'active' : ''}`}
          onclick={() => {
            activeTab = 'tags';
            search = '';
          }}
          role="tab"
          type="button"
        >
          Tags
        </button>
      </div>

      <ul class="gt-ref-items" role="listbox" aria-label="Available references">
        {#if !hasAnyRefs}
          <li class="gt-ref-empty">No references found.</li>
        {:else if activeTab === 'branches'}
          {#if filteredBranches.length === 0}
            <li class="gt-ref-empty">No matching branches.</li>
          {:else}
            {#each filteredBranches as branch}
              <li>
                <button
                  aria-selected={selected === branch}
                  class={`gt-ref-item ${selected === branch ? 'active' : ''}`}
                  onclick={() => selectRef(branch)}
                  role="option"
                  type="button"
                >
                  <span class="gt-ref-check">{#if selected === branch}<Check size={14} />{/if}</span>
                  <span class="truncate">{branch}</span>
                  {#if branch === refs?.default_branch}
                    <span class="gt-ref-default">default</span>
                  {/if}
                </button>
              </li>
            {/each}
          {/if}
        {:else}
          {#if filteredTags.length === 0}
            <li class="gt-ref-empty">No matching tags.</li>
          {:else}
            {#each filteredTags as tag}
              <li>
                <button
                  aria-selected={selected === tag}
                  class={`gt-ref-item ${selected === tag ? 'active' : ''}`}
                  onclick={() => selectRef(tag)}
                  role="option"
                  type="button"
                >
                  <span class="gt-ref-check">
                    {#if selected === tag}
                      <Check size={14} />
                    {:else}
                      <Tag size={12} />
                    {/if}
                  </span>
                  <span class="truncate">{tag}</span>
                </button>
              </li>
            {/each}
          {/if}
        {/if}
      </ul>

      {#if showViewAllBranches}
        <div class="gt-ref-footer">
          {#if viewAllBranchesHref.length > 0}
            <a class="gt-ref-footer-link" href={viewAllBranchesHref} onclick={closeMenu}>
              View all branches
            </a>
          {:else}
            <span>View all branches</span>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</div>
