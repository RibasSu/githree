# Githree

Githree is an open-source, GitLab-inspired, read-only repository browser.

- No user accounts
- No login/session flow
- No write operations on repository contents
- One backend API that handles all git operations

The goal is simple: browse any git repository (public or private, when credentials are provided) through a clean UI and download code snapshots.

## Highlights

- Add repositories by URL (`https://...` or `git@host:org/repo.git`)
- Browse refs (branches/tags)
- Browse trees and files
- Render README automatically
- View commit history and commit diffs
- Download raw files, `.tar.gz`, and `.zip`
- Background periodic fetch (optional)
- Flat-file registry (`data/repos.json`) with file locking

## Stack

- Backend: Rust 2024, Axum, Tokio, git2/libgit2, tracing
- Frontend: Svelte 5 + SvelteKit + Tailwind, Bun package manager
- Rendering: Shiki, marked, DOMPurify, lucide-svelte
- Caching: moka (in-memory tree cache, 60s TTL)

## Repository Layout

```text
backend/              Rust backend API and git services
frontend/             SvelteKit frontend
config/default.toml   Runtime config
data/                 Runtime state (repos cache + repos.json)
Dockerfile
docker-compose.yml
```

## Quick Start

### Docker

```bash
docker compose up --build
```

Open `http://localhost:3001`.

### Local Development

1. Backend

```bash
cd backend
cargo check
cargo run
```

2. Frontend (Bun)

```bash
cd frontend
bun install
bun run dev
```

Frontend API base defaults to `http://localhost:3001`.  
Override with `PUBLIC_API_URL` if needed.

## Configuration

Default file: [`config/default.toml`](./config/default.toml)

```toml
[server]
host = "0.0.0.0"
port = 3001

[storage]
repos_dir = "./data/repos"
registry_file = "./data/repos.json"
static_dir = "./static"

[git]
clone_timeout_secs = 120
fetch_on_request = true
ssh_private_key_path = "~/.ssh/id_rsa"

[fetch]
enabled = false
interval_minutes = 30

[repos]
credentials = []
```

### HTTPS Credentials Per Host

```toml
[[repos.credentials]]
host = "gitlab.mycompany.com"
username = "gitlab-ci"
password = "token"
```

### SSH Behavior

- Uses libgit2 credential callbacks
- Tries configured HTTPS host credentials first when requested
- Supports SSH key auth (`ssh_private_key_path`)
- Falls back to SSH agent (`ssh_key_from_agent`)

## API Reference

All endpoints are under `/api`.

| Method | Endpoint | Description |
|---|---|---|
| POST | `/repos` | Register/clone repository |
| GET | `/repos` | List registered repositories |
| DELETE | `/repos/{name}` | Remove repository from registry/cache |
| POST | `/repos/{name}/fetch` | Force fetch from remote |
| GET | `/repos/{name}/refs` | Branches, tags, default branch |
| GET | `/repos/{name}/tree?ref=&path=` | List directory entries |
| GET | `/repos/{name}/blob?ref=&path=` | File content + metadata |
| GET | `/repos/{name}/raw?ref=&path=` | Raw file download |
| GET | `/repos/{name}/readme?ref=` | README auto-detection |
| GET | `/repos/{name}/commits?ref=&path=&skip=&limit=` | Commit history |
| GET | `/repos/{name}/commit/{hash}` | Commit detail + file diffs |
| GET | `/repos/{name}/archive?ref=&format=tar.gz\|zip` | Archive download stream |

## Frontend Routes

| Route | Purpose |
|---|---|
| `/` | Add/list repositories, fuzzy search |
| `/{repo}` | Repository overview |
| `/{repo}/tree/{...path}` | Directory browsing |
| `/{repo}/blob/{...path}` | File viewer |
| `/{repo}/commits` | Commit history |
| `/{repo}/commit/{hash}` | Commit detail |

## Build and Validation

Backend:

```bash
cd backend
cargo fmt
cargo clippy -- -D warnings
cargo test
```

Frontend:

```bash
cd frontend
bun run check
bun run build
```

## Security and Governance

- Security policy: [`SECURITY.md`](./SECURITY.md)
- Contribution guide: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Contribution licensing: [`CONTRIBUTION_LICENSE.md`](./CONTRIBUTION_LICENSE.md)
- Code of conduct: [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md)
- Support channels: [`SUPPORT.md`](./SUPPORT.md)

## License

MIT — see [`LICENSE`](./LICENSE).
