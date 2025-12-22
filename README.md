# Move Clippy

A Move linter for Sui, inspired by Rust Clippy.

> **Research Project** — This was built agentically with Claude Opus 4.5 and Factory Cli as the harness. Not a substitute for security audits.

## Quick Start

```bash
# Basic linting
move-clippy path/to/sources

# With compiler type info (requires --features full build)
move-clippy --mode full path/to/package

# Enable more lints (higher FP risk)
move-clippy --preview path/to/sources
move-clippy --experimental path/to/sources

# List available lints
move-clippy list-rules
```

## Lint Tiers

| Tier | Flag | Use Case |
|------|------|----------|
| **Stable** | default | CI, daily dev — low false positives |
| **Preview** | `--preview` | Exploration — still validating |
| **Experimental** | `--experimental` | Audits/research — expect noise |

## What's Included

~80 lints covering:
- **Security**: capability patterns, overflow checks, oracle usage
- **Style**: naming, abilities ordering, modern syntax
- **Modernization**: Move 2024 syntax, method calls

Stable security lints validated against 100 repos with zero false positives.

Run `move-clippy list-rules` for the full list.

## Background

Built as a learning project to explore Move static analysis. Most code was AI-generated with manual review and iteration. Started with syntactic pattern matching, evolved toward type-based detection to reduce false positives.

See [docs/](docs/) for technical details.

## Contributing

Issues and PRs welcome. This is a side project — no guarantees on response time.

---

*An experiment in AI-assisted tooling for the Move ecosystem.*
