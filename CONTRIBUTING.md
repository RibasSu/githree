# Contributing to Githree

Thanks for your interest in contributing.

## Before You Start

- Read [`AGENTS.md`](./AGENTS.md) for project conventions.
- Read [`CONTRIBUTION_LICENSE.md`](./CONTRIBUTION_LICENSE.md).
- Be respectful and follow [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md).

## Local Setup

### Backend

```bash
cd backend
cargo check
cargo run
```

### Frontend (Bun)

```bash
cd frontend
bun install
bun run dev
```

## Required Checks

Run these before opening a PR.

### Backend

```bash
cd backend
cargo fmt
cargo clippy -- -D warnings
cargo test
```

### Frontend

```bash
cd frontend
bun run check
bun run build
```

## Pull Request Guidelines

- Keep PRs focused and small when possible.
- Include a clear summary:
  - problem
  - solution
  - tradeoffs
- Add screenshots for UI changes.
- Update docs when API/behavior changes.

## Commit Guidelines

Use Conventional Commits:

- `feat: ...`
- `fix: ...`
- `docs: ...`
- `refactor: ...`
- `chore: ...`
- `test: ...`

## Branching

Suggested branch names:

- `feat/<name>`
- `fix/<name>`
- `chore/<name>`

## Reporting Bugs

Please include:

- what you expected
- what happened
- repro steps
- logs/screenshots where relevant

Security issues should not be opened publicly. See [`SECURITY.md`](./SECURITY.md).
