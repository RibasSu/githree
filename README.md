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
- Request-time fetch guard cache (short TTL) to avoid repeated git network calls
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
fetch_cooldown_secs = 20
ssh_private_key_path = "~/.ssh/id_rsa"

[fetch]
enabled = true
interval = "60s"

[repos]
credentials = []

[features]
web_repo_management = false

[branding]
app_name = "Githree"
logo_url = "/logo.svg"
site_url = "https://githree.org"
domain = "githree.org"

[caddy]
enabled = false
command = "caddy"
config_file = "./config/Caddyfile"
adapter = "caddyfile"
args = []
# working_dir = "."
```

When `web_repo_management = false`, `POST /api/repos`, `DELETE /api/repos/{name}`, and `POST /api/repos/{name}/fetch` are blocked from the web API and the frontend switches to CLI-command generation for repository add/remove operations.

You can override this flag at runtime:

```bash
GITHREE_WEB_REPO_MANAGEMENT=true ./githree
```

Background sync interval supports seconds, minutes, and hours:

- `interval = "60s"` (default when omitted)
- `interval = "5m"`
- `interval = "1h"`

You can also override it at runtime:

```bash
GITHREE_FETCH_INTERVAL=2m ./githree
```

### Branding and Public URL

You can define application identity and links directly in config:

- `branding.app_name`: UI product name
- `branding.logo_url`: logo path or URL used in header/footer
- `branding.site_url`: canonical project URL
- `branding.domain`: domain label shown in navigation

Runtime overrides are available:

```bash
GITHREE_APP_NAME="My Git Viewer" \
GITHREE_LOGO_URL="/assets/logo.svg" \
GITHREE_SITE_URL="https://git.example.com" \
GITHREE_DOMAIN="git.example.com" \
./githree
```

### Optional Caddy Launcher

Set `caddy.enabled = true` to let Githree spawn Caddy on startup.

- If `caddy.args` is set, those args are used as-is.
- Else if `caddy.config_file` is set, Githree runs:
  - `caddy run --config <file> --adapter <adapter>`
- Else Githree falls back to:
  - `caddy reverse-proxy --from <branding.domain|branding.site_url> --to 127.0.0.1:<server.port>`

Runtime overrides:

```bash
GITHREE_CADDY_ENABLED=true \
GITHREE_CADDY_COMMAND=caddy \
GITHREE_CADDY_CONFIG_FILE=./config/Caddyfile \
GITHREE_CADDY_WORKING_DIR=. \
./githree
```

A starter Caddyfile is included at [`config/Caddyfile`](./config/Caddyfile).

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
| GET | `/settings` | Runtime settings (repo management mode, branding, and Caddy flag) |
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
