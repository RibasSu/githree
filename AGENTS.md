# AGENTS.md — Githree

> Instructions for AI agents (Codex, Claude Code, Cursor, etc.) working in
> this repository. Read this file **before** any task.

---

## Project Overview

**Githree** is an open-source platform to view, browse, and download git
repositories — no login, no authentication, fully read-only.
It supports GitHub, GitLab, and public or private git repositories (via SSH or HTTPS).

| Layer     | Technology                                        |
| --------- | ------------------------------------------------- |
| Backend   | Rust (stable) · Axum · git2 · Tokio               |
| Frontend  | Svelte 5 · SvelteKit · Skeleton UI · Tailwind CSS |
| Container | Docker (multi-stage) · docker-compose             |
| Registry  | JSON flat-file (`data/repos.json`)                |

---

## Repository Structure

```
githree/
├── backend/               # Rust crate — API + HTTP server
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── config.rs
│       ├── router.rs
│       ├── error.rs
│       ├── git/           # clone, browse, archive, refs
│       └── handlers/      # repos, tree, blob, commits, archive
├── frontend/              # SvelteKit app
│   ├── package.json
│   ├── svelte.config.js
│   ├── tailwind.config.ts
│   └── src/
│       ├── lib/           # api.ts, types.ts, components/
│       └── routes/        # SvelteKit pages
├── config/
│   └── default.toml       # Default configuration
├── data/                  # Generated at runtime — DO NOT commit
│   └── repos.json
├── Dockerfile
├── docker-compose.yml
├── AGENTS.md              # This file
└── README.md
```

---

## Non-Negotiable Rules

These rules must never be violated, regardless of the task:

1. **No user authentication.** Githree has no login, session, JWT, session
   cookie, or any identity mechanism for end users. Do not add any of this,
   not even as an optional feature.

2. **Read-only for visitors.** The API must never expose endpoints that modify
   repository contents (`git push`, `git commit`, etc.). The only write
   endpoints are: add/remove a repo from the registry and force fetch.

3. **The backend is the single source of truth for git operations.** The
   frontend must never call the GitHub/GitLab API directly. Every git operation
   goes through the Rust backend.

4. **No database.** Persistent state lives exclusively in `data/repos.json`.
   Use file locking (`fd-lock` crate) for concurrent access.

5. **No additional JavaScript dependencies in the production bundle beyond
   those listed** in the approved `package.json` (Skeleton UI, Shiki, Marked,
   DOMPurify, lucide-svelte). Do not add React, Vue, or other UI libraries.

---

## Code Conventions

### Rust (backend/)

- **Edition**: Rust 2021
- **Formatting**: `rustfmt` with default configuration — run `cargo fmt` before
  any commit.
- **Linting**: `cargo clippy -- -D warnings`. Zero warnings allowed in CI.
- **Errors**: Use the `AppError` type in `src/error.rs`. Never use `.unwrap()`
  or `.expect()` in production code — only in tests.
- **Async**: Tokio everywhere. Never block the async thread with heavy
  synchronous operations; use `tokio::task::spawn_blocking` for `git2` calls,
  which are synchronous.
- **Logging**: `tracing::info!`, `tracing::warn!`, `tracing::error!`. Never
  use `println!` in production code.
- **Tests**: Inline `#[cfg(test)]` modules for unit tests. Integration tests in
  `backend/tests/`. Run with `cargo test`.
- **Naming**: snake_case for everything except types (PascalCase) and constants
  (SCREAMING_SNAKE_CASE).

```rust
// GOOD — explicit error return, no unwrap
pub async fn get_blob(repo: &Repository, oid: Oid) -> Result<Blob, AppError> {
    repo.find_blob(oid).map_err(AppError::Git)
}

// BAD — never do this in production
pub async fn get_blob(repo: &Repository, oid: Oid) -> Blob {
    repo.find_blob(oid).unwrap()
}
```

### Svelte / TypeScript (frontend/)

- **Svelte 5**: Use the new runes syntax (`$state`, `$derived`, `$effect`).
  Do not use the legacy Svelte 4 API (`$:`, `export let`) in new components.
- **TypeScript**: `strict: true` in `tsconfig.json`. No implicit `any`.
- **Formatting**: Prettier with the Svelte plugin. Config in `.prettierrc`.
- **Linting**: ESLint with `eslint-plugin-svelte`. Zero CI errors.
- **Components**: One component per file. Use PascalCase names.
  Place them in `src/lib/components/`.
- **API calls**: Always through `src/lib/api.ts`. Never call `fetch` directly
  inside `.svelte` files.
- **Styles**: Tailwind utility-first. Use local styles only when Tailwind is
  not enough, using scoped `<style>` in the component.
- **No `console.log`** in committed code. Use dev guards if needed:
  ```ts
  if (import.meta.env.DEV) console.debug(...)
  ```

### Git

- **Branches**: `main` (production), `dev` (integration), `feat/<name>`,
  `fix/<name>`, `chore/<name>`.
- **Commits**: Conventional Commits in English.
  - `feat:` new feature
  - `fix:` bug fix
  - `refactor:` behavior-preserving refactor
  - `chore:` maintenance tasks (deps, configs)
  - `docs:` documentation
  - `test:` tests
- **PRs**: Always target `dev`. Never open directly against `main`.

---

## Workflow for Agents

When you receive a task, **always** follow this sequence:

### 1. Understand before writing

```
- Read relevant files BEFORE editing anything.
- If the task involves backend code, read src/router.rs and the related handler.
- If it involves frontend code, read the SvelteKit route and affected component.
- If it involves git2, read src/git/mod.rs to understand existing helpers.
```

### 2. Plan

For non-trivial tasks, write a plan in comments before coding:

```rust
// PLAN:
// 1. Receive `repo_name` and `ref` from query string
// 2. Look up the repo in the registry (repos.json)
// 3. Open with git2::Repository::open_bare()
// 4. Resolve ref to a commit OID
// 5. Walk the commit tree and serialize as Vec<TreeEntry>
// 6. Return JSON
```

### 3. Implement

- Write code following the conventions above.
- Add `tracing` spans in handler functions.
- Always return `Result<Json<T>, AppError>` in Axum handlers.

### 4. Test locally

```bash
# Backend
cd backend && cargo test && cargo clippy -- -D warnings && cargo fmt --check

# Frontend
cd frontend && npm run check && npm run lint && npm run build
```

### 5. Do not leave TODOs untracked

If you leave a `// TODO:` in code, also open an issue in the tracker
(or list it at the end of the PR). Never leave `// TODO: fix this later`
without context.

---

## Area Guide

### Adding a new API endpoint

1. Define the response type in `backend/src/git/` or create a new module.
2. Implement the handler in `backend/src/handlers/<area>.rs`.
3. Register the route in `backend/src/router.rs`.
4. Add the corresponding method in `frontend/src/lib/api.ts`.
5. Write an integration test in `backend/tests/`.

### Adding a new Svelte component

1. Create the file in `frontend/src/lib/components/ComponentName.svelte`.
2. Use Skeleton UI as a base when possible.
3. Export component TypeScript types at the top of the file.
4. Document props with JSDoc.

### Adding support for a new file format (viewer)

1. Edit `frontend/src/lib/components/BlobViewer.svelte`.
2. Add extension-based type detection in `detectLanguage()`.
3. For binaries: render a download card instead of the viewer.
4. For images: render `<img>` with base64 content.

### Modifying the `repos.json` structure

1. Update the `RepoInfo` type in `backend/src/git/mod.rs`.
2. Update migration logic in `backend/src/config.rs` (`schema_version` field).
3. Update the corresponding type in `frontend/src/lib/types.ts`.
4. **Never** make breaking changes without migration — the file may exist in
   live installations.

---

## Environment Variables

| Variable            | Default                 | Description                                |
| ------------------- | ----------------------- | ------------------------------------------ |
| `GITHREE_HOST`      | `0.0.0.0`               | Server bind address                        |
| `GITHREE_PORT`      | `3001`                  | Server port                                |
| `GITHREE_REPOS_DIR` | `./data/repos`          | Directory for cloned repositories          |
| `GITHREE_CONFIG`    | `./config/default.toml` | Config file path                           |
| `RUST_LOG`          | `info`                  | Log level (trace/debug/info/warn/error)    |
| `PUBLIC_API_URL`    | `http://localhost:3001` | API URL (used by frontend)                 |

---

## What to Never Do

| ❌ Forbidden                               | ✅ Correct alternative                    |
| ------------------------------------------ | ----------------------------------------- |
| `unwrap()` / `expect()` in production      | `map_err(AppError::...)` or `?`           |
| `any` in TypeScript                        | Explicit types or `unknown`               |
| Direct `fetch()` in Svelte component       | Use `src/lib/api.ts`                      |
| Call GitHub/GitLab APIs from frontend      | Backend handles all git operations        |
| Add login/authentication system            | It does not exist by design               |
| `println!` in production Rust              | `tracing::info!` / `tracing::debug!`      |
| Commit `data/` or `target/`                | They are in `.gitignore`                  |
| Use `$:` (Svelte 4) in new components      | Use Svelte 5 runes (`$state`, `$derived`) |
| Block async thread with synchronous `git2` | `tokio::task::spawn_blocking`             |
| `console.log` in production                | `import.meta.env.DEV` guard               |

---

## CI/CD (GitHub Actions)

The pipeline runs on every PR and push to `main`/`dev`:

```
backend-ci:
  - cargo fmt --check
  - cargo clippy -- -D warnings
  - cargo test --all

frontend-ci:
  - npm run check  (svelte-check)
  - npm run lint   (eslint)
  - npm run build  (ensures it builds with no errors)

docker-ci:
  - docker build .  (ensures the Dockerfile image is valid)
```

No PR can be merged with failing CI.

---

## Frequently Asked Questions for Agents

**Q: Can I use `sqlx` or any ORM?**
A: No. The project intentionally uses a flat-file JSON approach. Keep it that way.

**Q: Can I add authentication for the admin panel (add/remove repos)?**
A: Yes, but only via config (static token in `default.toml`), never with a
full user system. This is optional and must be feature-flagged.

**Q: Should the frontend be SSR or SPA?**
A: SvelteKit with SSR enabled for browse routes (better SEO and initial
performance). Use `+page.server.ts` for data that can be loaded on the server.

**Q: How do I run locally without Docker?**
A:

```bash
# Terminal 1 — Backend
cd backend
cargo run

# Terminal 2 — Frontend
cd frontend
npm install
PUBLIC_API_URL=http://localhost:3001 npm run dev
```

**Q: Can I rename API routes?**
A: Not without updating `backend/src/router.rs` and `frontend/src/lib/api.ts`
at the same time. They must always stay in sync.
If it is a breaking change, bump API version (`/api/v2/`).

---

_Last updated: 2026 — Githree Project_
