<script lang="ts">
  import { onMount } from 'svelte';
  import RepoCard from '$lib/components/RepoCard.svelte';
  import ShimmerRows from '$lib/components/ShimmerRows.svelte';
  import { api } from '$lib/api';
  import type { AppSettings, RepoInfo } from '$lib/types';

  let repoUrl = $state('');
  let repoAlias = $state('');
  let search = $state('');
  let repos = $state<RepoInfo[]>([]);
  let settings = $state<AppSettings | null>(null);
  let loading = $state(false);
  let generatedAddCommand = $state('');
  let generatingCommand = $state(false);
  let repoUrlError = $state('');

  const webRepoManagement = $derived(settings?.web_repo_management ?? false);
  const commandReposDir = $derived(settings?.repos_dir || './data/repos');
  const commandRegistryFile = $derived(settings?.registry_file || './data/repos.json');

  const filteredRepos = $derived(
    repos.filter((repo) => fuzzyMatch(repo.name, search) || fuzzyMatch(repo.url, search))
  );

  onMount(() => {
    void Promise.all([loadSettings(), loadRepos()]);
  });

  async function loadSettings() {
    try {
      settings = await api.getSettings();
    } catch {
      settings = {
        web_repo_management: false,
        repos_dir: './data/repos',
        registry_file: './data/repos.json'
      };
    }
  }

  async function loadRepos() {
    loading = true;
    try {
      repos = await api.listRepos();
    } catch {
      // toast already emitted in api client
    } finally {
      loading = false;
    }
  }

  async function submitRepo(event: SubmitEvent) {
    event.preventDefault();
    const trimmedUrl = repoUrl.trim();
    const validationError = validateRepoUrl(trimmedUrl);
    if (validationError) {
      repoUrlError = validationError;
      return;
    }
    repoUrlError = '';

    if (webRepoManagement) {
      try {
        await api.addRepo(trimmedUrl, repoAlias.trim() || undefined);
        repoUrl = '';
        repoAlias = '';
        generatedAddCommand = '';
        api.notify('Repository added.', 'success');
        await loadRepos();
      } catch {
        // toast already emitted in api client
      }
      return;
    }

    generatingCommand = true;
    try {
      generatedAddCommand = buildAddCommand(trimmedUrl, repoAlias.trim());
      api.notify('CLI add command generated.', 'info');
    } finally {
      generatingCommand = false;
    }
  }

  async function fetchRepo(name: string) {
    if (!webRepoManagement) return;
    try {
      await api.fetchRepo(name);
      api.notify(`Fetched ${name}.`, 'success');
      await loadRepos();
    } catch {
      // toast already emitted in api client
    }
  }

  async function removeRepo(name: string) {
    if (!webRepoManagement) {
      await copyCommand(buildRemoveCommand(name), `Remove command for ${name} copied.`);
      return;
    }
    const approved = window.confirm(`Remove repository "${name}" from Githree?`);
    if (!approved) return;
    try {
      await api.deleteRepo(name);
      api.notify(`Removed ${name}.`, 'success');
      await loadRepos();
    } catch {
      // toast already emitted in api client
    }
  }

  async function copyCommand(command: string, successMessage = 'Command copied.') {
    try {
      await navigator.clipboard.writeText(command);
      api.notify(successMessage, 'success');
    } catch {
      api.notify('Failed to copy command.', 'error');
    }
  }

  function deriveRepoName(url: string, alias?: string): string {
    const base = alias && alias.trim().length > 0 ? alias.trim() : deriveNameFromUrl(url);
    const lowered = base.toLowerCase();
    const sanitized = lowered.replace(/[^a-z0-9_-]/g, '-').replace(/-+/g, '-').replace(/^-|-$/g, '');
    return sanitized.length > 0 ? sanitized : 'repo';
  }

  function deriveNameFromUrl(url: string): string {
    const normalized = url.trim().replace(/\/+$/, '').replace(/\.git$/i, '');
    const parts = normalized.split(/[/:]/).filter((segment) => segment.length > 0);
    return parts[parts.length - 1] || 'repo';
  }

  function validateRepoUrl(url: string): string {
    if (url.length === 0) return 'Enter a repository URL.';
    if (/\s/.test(url)) return 'Repository URL must not contain spaces.';

    if (isScpLikeSshUrl(url)) return '';

    try {
      const parsed = new URL(url);
      const protocol = parsed.protocol.toLowerCase();
      if (!['https:', 'http:', 'ssh:', 'git:'].includes(protocol)) {
        return 'Use a supported protocol: https, http, ssh, or git.';
      }
      if (!parsed.hostname) return 'Repository URL is missing a host.';
      return '';
    } catch {
      return 'Invalid repository URL. Example: https://github.com/user/repo.git';
    }
  }

  function isScpLikeSshUrl(url: string): boolean {
    // Accepts common SSH scp-style URLs such as git@github.com:org/repo.git
    return /^[a-z0-9._-]+@[a-z0-9.-]+:[^:\s]+$/i.test(url);
  }

  function detectSource(url: string): 'github' | 'gitlab' | 'generic' {
    const lowered = url.toLowerCase();
    if (lowered.includes('github.com')) return 'github';
    if (lowered.includes('gitlab.com')) return 'gitlab';
    return 'generic';
  }

  function shellQuote(value: string): string {
    return `'${value.replaceAll("'", "'\"'\"'")}'`;
  }

  function buildAddCommand(url: string, alias?: string): string {
    const name = deriveRepoName(url, alias);
    const source = detectSource(url);
    const reposDir = commandReposDir;
    const registryFile = commandRegistryFile;

    return [
      `REPO_URL=${shellQuote(url)}`,
      `REPO_NAME=${shellQuote(name)}`,
      `REPOS_DIR=${shellQuote(reposDir)}`,
      `REGISTRY_FILE=${shellQuote(registryFile)}`,
      '',
      'mkdir -p "$REPOS_DIR"',
      'mkdir -p "$(dirname "$REGISTRY_FILE")"',
      'test -f "$REGISTRY_FILE" || echo "[]" > "$REGISTRY_FILE"',
      'git clone --bare "$REPO_URL" "$REPOS_DIR/$REPO_NAME"',
      'DEFAULT_BRANCH="$(git --git-dir "$REPOS_DIR/$REPO_NAME" symbolic-ref --short refs/remotes/origin/HEAD | sed \'s#^origin/##\')"',
      'SIZE_KB="$(du -sk "$REPOS_DIR/$REPO_NAME" | awk \'{print $1}\')"',
      'TMP_FILE="$(mktemp)"',
      `jq --arg name "$REPO_NAME" --arg url "$REPO_URL" --arg branch "$DEFAULT_BRANCH" --arg source ${shellQuote(source)} --argjson size "$SIZE_KB" '\\`,
      "  map(select(.name != $name)) + [{name:$name,url:$url,description:null,default_branch:$branch,last_fetched:null,size_kb:$size,source:$source}]' \\",
      '  "$REGISTRY_FILE" > "$TMP_FILE" && mv "$TMP_FILE" "$REGISTRY_FILE"'
    ].join('\n');
  }

  function buildRemoveCommand(name: string): string {
    const reposDir = commandReposDir;
    const registryFile = commandRegistryFile;
    return [
      `REPO_NAME=${shellQuote(name)}`,
      `REPOS_DIR=${shellQuote(reposDir)}`,
      `REGISTRY_FILE=${shellQuote(registryFile)}`,
      '',
      'rm -rf "$REPOS_DIR/$REPO_NAME"',
      'TMP_FILE="$(mktemp)"',
      "jq --arg name \"$REPO_NAME\" 'map(select(.name != $name))' \"$REGISTRY_FILE\" > \"$TMP_FILE\" && mv \"$TMP_FILE\" \"$REGISTRY_FILE\""
    ].join('\n');
  }

  function fuzzyMatch(value: string, needle: string): boolean {
    const term = needle.trim().toLowerCase();
    if (term.length === 0) return true;
    const source = value.toLowerCase();
    if (source.includes(term)) return true;

    let index = 0;
    for (const ch of source) {
      if (ch === term[index]) index += 1;
      if (index === term.length) return true;
    }
    return false;
  }
</script>

<section class="space-y-6">
  <div class="card-surface p-4">
    <h1 class="text-lg font-semibold text-[#f0f6fc]">Add a Repository</h1>
    <p class="mt-1 text-sm gt-muted">
      {#if webRepoManagement}
        Paste any GitHub, GitLab, or self-hosted git URL (SSH or HTTPS).
      {:else}
        Web repository management is disabled. Generate a CLI command and run it in your shell.
      {/if}
    </p>
    <form class="mt-4 grid gap-3 md:grid-cols-[1fr_240px_auto]" novalidate onsubmit={submitRepo}>
      <input
        bind:value={repoUrl}
        aria-describedby="repo-url-error"
        aria-invalid={repoUrlError.length > 0}
        class="input"
        oninput={() => {
          if (repoUrlError.length > 0) {
            repoUrlError = '';
          }
        }}
        placeholder="https://github.com/user/repo.git or git@github.com:user/repo.git"
        type="text"
      />
      <input bind:value={repoAlias} class="input" placeholder="Optional alias (e.g. my-repo)" type="text" />
      <button class="btn btn-primary justify-center" disabled={generatingCommand} type="submit">
        {#if webRepoManagement}
          Add Repo
        {:else}
          Generate CLI Command
        {/if}
      </button>
    </form>
    {#if repoUrlError.length > 0}
      <p class="mt-2 text-sm text-[#da3633]" id="repo-url-error" role="alert">{repoUrlError}</p>
    {/if}

    {#if !webRepoManagement}
      <div class="mt-4 rounded-sm border gt-divider bg-[#0d1117] p-3 text-xs">
        <p class="gt-muted">
          Repository changes must be done via command line and persisted in:
          <code class="ml-1 text-[#c9d1d9]">{commandRegistryFile}</code>
        </p>
        {#if generatedAddCommand.length > 0}
          <div class="mt-3 space-y-2">
            <pre class="overflow-x-auto rounded-sm border gt-divider bg-[#010409] p-3 text-[#c9d1d9]">{generatedAddCommand}</pre>
            <div>
              <button
                class="btn"
                onclick={() => copyCommand(generatedAddCommand, 'Add command copied.')}
                type="button"
              >
                Copy Add Command
              </button>
            </div>
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <div class="flex flex-wrap items-center justify-between gap-3">
    <h2 class="text-lg font-semibold text-[#f0f6fc]">Registered Repositories</h2>
    <input bind:value={search} class="input max-w-xs" placeholder="Search repositories" type="text" />
  </div>

  {#if loading && repos.length === 0}
    <ShimmerRows rows={6} />
  {:else if filteredRepos.length === 0}
    <div class="card-surface p-6 text-sm gt-muted">
      {#if webRepoManagement}
        No repositories found. Add your first repository URL above.
      {:else}
        No repositories found. Generate and run an add command above.
      {/if}
    </div>
  {:else}
    <div class="card-surface overflow-hidden">
      <div class="grid grid-cols-[minmax(220px,1fr)_minmax(200px,1fr)_auto] gap-3 border-b gt-divider px-4 py-2 text-xs font-semibold uppercase tracking-wide gt-muted">
        <span>Repository</span>
        <span>Remote</span>
        <span class="text-right">Actions</span>
      </div>
      {#each filteredRepos as repo (repo.name)}
        <RepoCard
          {repo}
          onCopyRemoveCommand={removeRepo}
          onFetch={fetchRepo}
          onRemove={removeRepo}
          {webRepoManagement}
        />
      {/each}
    </div>
  {/if}
</section>
