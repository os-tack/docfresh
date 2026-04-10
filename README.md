# docfresh

Track documentation freshness against source code.

Documentation sites go stale when source code changes. docfresh maintains a machine-readable manifest that maps each documentation page to its source-of-truth files, then uses git history to detect when sources have changed since the docs were last verified.

## Install

```sh
cargo install --path .
```

## Quick Start

```sh
cd your-docs-site
docfresh init --source-repo ../your-source-repo
docfresh suggest --apply 0.6
docfresh verify --all
docfresh audit
```

`init` auto-detects your site framework and source language. `suggest` cross-references source files against page content to populate the manifest. `verify` marks pages as current. `audit` tells you what's gone stale.

## Commands

### `docfresh init`

Bootstrap a `site-manifest.json` by scanning your site for pages.

```sh
docfresh init --source-repo ../api-server
```

Auto-detects the site framework from project files (Astro, Next.js, Hugo, Docusaurus, MkDocs, VitePress, Jekyll, mdBook, Sphinx) and the source language (Rust, Go, Python, TypeScript, JavaScript, Java, C#, Ruby, PHP, C++). Override with `--site <preset>` and `--lang <preset>`.

```sh
docfresh init --list-presets          # see all available presets
docfresh init --site hugo --source-repo ../backend
```

### `docfresh suggest`

Scan unmapped source files and recommend which pages they belong to.

```sh
docfresh suggest                       # show all suggestions
docfresh suggest --file src/auth.rs    # suggest for one file
docfresh suggest --apply 0.6           # auto-apply above 60% confidence
docfresh suggest --json                # machine-readable output
```

Uses three-tier scoring: literal path references in page content, file stem matching, and term overlap from public API symbols and doc comments. Extracts `pub fn`/`pub struct` from Rust, exported names from Go, `export function` from TypeScript, `class`/`def` from Python, and so on.

### `docfresh map`

Add a source mapping to a page.

```sh
docfresh map /docs/auth src/auth/handler.rs
docfresh map /docs/auth src/auth/middleware.rs --sections "authenticate" "authorize"
```

### `docfresh audit`

Check all pages for staleness. Compares each page's `verified_at` SHA against current source repo HEAD.

```sh
docfresh audit                         # colored terminal output
docfresh audit --tag reference         # filter by tag
docfresh audit --json                  # machine-readable for CI
```

Exit code 0 = all current, 1 = stale pages found. Reports which source files changed, the commit log since verification, and transitive warnings for related pages.

### `docfresh verify`

Mark pages as verified at the current source HEAD.

```sh
docfresh verify /docs/auth             # verify one page
docfresh verify --all                  # verify everything
docfresh verify /docs/auth --sha abc123  # pin to specific SHA
```

### `docfresh diff`

Show what changed in a page's sources since it was last verified.

```sh
docfresh diff /docs/auth
```

### `docfresh status`

Show the status of all pages or a specific page.

```sh
docfresh status                        # summary table
docfresh status /docs/auth             # detailed view
docfresh status --json                 # machine-readable
docfresh status --markdown             # for pasting into issues
```

### `docfresh coverage`

Compare documented source files against all tracked files in the source repo.

```sh
docfresh coverage                      # text report
docfresh coverage --json               # machine-readable
docfresh coverage --scan "tests/**/*.rs"  # add extra scan patterns
```

Reports undocumented files, orphan pages, and shared sources.

### `docfresh report`

Combined audit + coverage in one output.

```sh
docfresh report                        # text
docfresh report --format markdown      # for CI summaries
docfresh report --format json          # machine-readable
```

## Manifest Format

`site-manifest.json` lives in the doc site repo root:

```json
{
  "version": 1,
  "source_repo": {
    "path": "../api-server",
    "default_branch": "main"
  },
  "exclude_patterns": [
    "src/internal/**",
    "src/**/mod.rs"
  ],
  "pages": [
    {
      "route": "/docs/auth",
      "file": "src/pages/docs/auth.astro",
      "title": "Authentication",
      "tags": ["reference", "security"],
      "sources": [
        { "path": "src/auth/handler.rs", "sections": ["authenticate"] },
        { "path": "docs/spec/auth.md" }
      ],
      "related": ["/docs/permissions", "/features/sso"],
      "verified_at": {
        "sha": "a1b2c3d",
        "timestamp": "2025-01-15T10:30:00Z"
      },
      "status": "current"
    }
  ]
}
```

### Page status

| Status | Meaning |
|--------|---------|
| `current` | Verified, no source changes since |
| `stale` | Source files changed since `verified_at` |
| `unverified` | Never been verified |
| `outdated` | Manually marked as needing rewrite |
| `missing` | Planned page, no file yet |

### Fields

- **sources** — files in the source repo this page documents. `sections` is an optional human-readable marker for what part of the file matters.
- **related** — other page routes that reference the same concepts. When a page goes stale, its related pages get a "review recommended" warning.
- **exclude_patterns** — glob patterns for source files to ignore in `coverage` and `suggest`.
- **tags** — arbitrary labels for filtering (`docfresh audit --tag reference`).

## Configuration

Create `.docfresh.toml` in your doc site repo to set policy defaults:

```toml
[source]
# Extra scan patterns beyond the language preset defaults
scan = ["tests/**/*.rs", "benches/**/*.rs"]
# Exclude internal files from coverage and suggest
exclude = ["src/**/mod.rs", "src/internal/**", "docs/spec/archive/*"]

[ci]
# Maximum stale pages before audit fails (0 = any stale page fails)
max_stale = 0
# Minimum documentation coverage percentage (0 = disabled)
min_coverage = 20
# Fail if any source file is unmapped and not excluded
fail_on_unmapped = true
# Output format for CI: "text", "markdown", "json"
format = "markdown"
```

All settings have CLI flag overrides (`--max-stale`, `--min-coverage`, `--fail-on-unmapped`).

### `docfresh ci`

One command for CI pipelines. Runs audit + coverage with threshold enforcement:

```sh
docfresh ci                            # uses .docfresh.toml settings
docfresh ci --max-stale 5              # override: allow up to 5 stale pages
docfresh ci --fail-on-unmapped true    # override: fail on unmapped files
docfresh ci --format json              # override: JSON output
```

Exit code 0 = all checks passed, 1 = threshold violation.

## CI Integration

```yaml
# .github/workflows/doc-freshness.yml
name: doc-freshness
on:
  push:
    branches: [main]
  schedule:
    - cron: '0 9 * * 1'

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/checkout@v4
        with:
          repository: your-org/api-server
          path: api-server
      - run: cargo install docfresh
      - run: docfresh ci
        env:
          DOCFRESH_SOURCE_REPO: ./api-server
      - run: docfresh report --format markdown >> $GITHUB_STEP_SUMMARY
```

### Gate on new source files

When a developer adds a new source file, CI can catch it:

```toml
# .docfresh.toml
[source]
exclude = ["src/**/mod.rs", "src/util/**"]

[ci]
fail_on_unmapped = true
max_stale = 3  # allow some staleness during active sprints
```

The developer either:
1. Runs `docfresh map /docs/auth src/auth/new_handler.rs` to map it
2. Adds the file to `exclude` patterns if it's internal
3. Or CI fails with a clear message listing the unmapped files

## Supported Frameworks

### Site frameworks (auto-detected)

| Framework | Detected by | Page patterns |
|-----------|------------|---------------|
| Astro | `astro.config.*` | `src/pages/**/*.astro` |
| Next.js (App Router) | `next.config.*` + `app/` dir | `app/**/page.tsx` |
| Next.js (Pages Router) | `next.config.*` + `pages/` dir | `pages/**/*.tsx` |
| Hugo | `hugo.toml` | `content/**/*.md` |
| Docusaurus | `docusaurus.config.*` | `docs/**/*.md` |
| MkDocs | `mkdocs.yml` | `docs/**/*.md` |
| VitePress | `.vitepress/` dir | `docs/**/*.md` |
| Jekyll | `_config.yml` | `_posts/**/*.md` |
| mdBook | `book.toml` | `src/**/*.md` |
| Sphinx | `conf.py` | `**/*.rst` |
| Markdown | fallback | `docs/**/*.md` |

### Source languages (auto-detected)

| Language | Detected by | Public API extraction |
|----------|------------|----------------------|
| Rust | `Cargo.toml` | `pub fn/struct/enum/trait`, `///` docs |
| Go | `go.mod` | Exported names (uppercase), `//` docs |
| Python | `pyproject.toml` | `def`, `class`, docstrings |
| TypeScript | `tsconfig.json` | `export function/class/interface/const` |
| JavaScript | `package.json` | `export function/class/const` |
| Java | `pom.xml` / `build.gradle` | `public class/interface/enum` |
| C# | `*.csproj` / `*.sln` | `public class/interface`, `///` XML docs |
| Ruby | `Gemfile` | `def`, `class`, `module` |
| PHP | `composer.json` | `public function`, `class/interface/trait` |
| C++ | `CMakeLists.txt` | `class/struct/namespace`, Doxygen |

## License

MIT OR Apache-2.0
