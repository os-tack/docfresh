use std::path::Path;

/// Site framework preset — defines how to find and route page files.
#[derive(Debug, Clone)]
pub struct SitePreset {
    pub name: &'static str,
    pub page_patterns: &'static [&'static str],
    pub base_dir: &'static str,
    pub extensions: &'static [&'static str],
}

/// Source language preset — defines scan patterns and term extraction.
#[derive(Debug, Clone)]
pub struct SourcePreset {
    pub name: &'static str,
    pub scan_patterns: &'static [&'static str],
}

// ── Site framework presets ──────────────────────────────────────────

pub const ASTRO: SitePreset = SitePreset {
    name: "astro",
    page_patterns: &["src/pages/**/*.astro"],
    base_dir: "src/pages",
    extensions: &[".astro"],
};

pub const NEXTJS_APP: SitePreset = SitePreset {
    name: "nextjs-app",
    page_patterns: &["app/**/page.tsx", "app/**/page.jsx", "app/**/page.mdx"],
    base_dir: "app",
    extensions: &[".tsx", ".jsx", ".mdx"],
};

pub const NEXTJS_PAGES: SitePreset = SitePreset {
    name: "nextjs-pages",
    page_patterns: &["pages/**/*.tsx", "pages/**/*.jsx", "pages/**/*.mdx"],
    base_dir: "pages",
    extensions: &[".tsx", ".jsx", ".mdx"],
};

pub const HUGO: SitePreset = SitePreset {
    name: "hugo",
    page_patterns: &["content/**/*.md", "content/**/*.html"],
    base_dir: "content",
    extensions: &[".md", ".html"],
};

pub const DOCUSAURUS: SitePreset = SitePreset {
    name: "docusaurus",
    page_patterns: &["docs/**/*.md", "docs/**/*.mdx", "blog/**/*.md"],
    base_dir: "docs",
    extensions: &[".md", ".mdx"],
};

pub const MKDOCS: SitePreset = SitePreset {
    name: "mkdocs",
    page_patterns: &["docs/**/*.md"],
    base_dir: "docs",
    extensions: &[".md"],
};

pub const VITEPRESS: SitePreset = SitePreset {
    name: "vitepress",
    page_patterns: &["docs/**/*.md", "*.md"],
    base_dir: "docs",
    extensions: &[".md"],
};

pub const JEKYLL: SitePreset = SitePreset {
    name: "jekyll",
    page_patterns: &["_posts/**/*.md", "**/*.md", "**/*.html"],
    base_dir: ".",
    extensions: &[".md", ".html"],
};

pub const MDBOOK: SitePreset = SitePreset {
    name: "mdbook",
    page_patterns: &["src/**/*.md"],
    base_dir: "src",
    extensions: &[".md"],
};

pub const SPHINX: SitePreset = SitePreset {
    name: "sphinx",
    page_patterns: &["**/*.rst", "**/*.md"],
    base_dir: ".",
    extensions: &[".rst", ".md"],
};

pub const MARKDOWN: SitePreset = SitePreset {
    name: "markdown",
    page_patterns: &["docs/**/*.md", "**/*.md"],
    base_dir: "docs",
    extensions: &[".md"],
};

// ── Source language presets ──────────────────────────────────────────

pub const RUST: SourcePreset = SourcePreset {
    name: "rust",
    scan_patterns: &["src/**/*.rs", "docs/spec/**/*.md"],
};

pub const GO: SourcePreset = SourcePreset {
    name: "go",
    scan_patterns: &["**/*.go", "docs/**/*.md"],
};

pub const PYTHON: SourcePreset = SourcePreset {
    name: "python",
    scan_patterns: &["**/*.py", "docs/**/*.md", "docs/**/*.rst"],
};

pub const TYPESCRIPT: SourcePreset = SourcePreset {
    name: "typescript",
    scan_patterns: &["src/**/*.ts", "src/**/*.tsx", "lib/**/*.ts", "docs/**/*.md"],
};

pub const JAVASCRIPT: SourcePreset = SourcePreset {
    name: "javascript",
    scan_patterns: &["src/**/*.js", "src/**/*.jsx", "lib/**/*.js", "docs/**/*.md"],
};

pub const JAVA: SourcePreset = SourcePreset {
    name: "java",
    scan_patterns: &["src/**/*.java", "docs/**/*.md"],
};

pub const CSHARP: SourcePreset = SourcePreset {
    name: "csharp",
    scan_patterns: &["**/*.cs", "docs/**/*.md"],
};

pub const RUBY: SourcePreset = SourcePreset {
    name: "ruby",
    scan_patterns: &["lib/**/*.rb", "app/**/*.rb", "docs/**/*.md"],
};

pub const PHP: SourcePreset = SourcePreset {
    name: "php",
    scan_patterns: &["src/**/*.php", "app/**/*.php", "docs/**/*.md"],
};

pub const CPP: SourcePreset = SourcePreset {
    name: "cpp",
    scan_patterns: &[
        "src/**/*.cpp",
        "src/**/*.h",
        "include/**/*.h",
        "docs/**/*.md",
    ],
};

// ── Detection ───────────────────────────────────────────────────────

/// All known site presets for lookup by name.
pub fn all_site_presets() -> Vec<&'static SitePreset> {
    vec![
        &ASTRO,
        &NEXTJS_APP,
        &NEXTJS_PAGES,
        &HUGO,
        &DOCUSAURUS,
        &MKDOCS,
        &VITEPRESS,
        &JEKYLL,
        &MDBOOK,
        &SPHINX,
        &MARKDOWN,
    ]
}

/// All known source presets for lookup by name.
pub fn all_source_presets() -> Vec<&'static SourcePreset> {
    vec![
        &RUST,
        &GO,
        &PYTHON,
        &TYPESCRIPT,
        &JAVASCRIPT,
        &JAVA,
        &CSHARP,
        &RUBY,
        &PHP,
        &CPP,
    ]
}

pub fn find_site_preset(name: &str) -> Option<&'static SitePreset> {
    all_site_presets().into_iter().find(|p| p.name == name)
}

pub fn find_source_preset(name: &str) -> Option<&'static SourcePreset> {
    all_source_presets().into_iter().find(|p| p.name == name)
}

/// Auto-detect site framework from files in the given directory.
pub fn detect_site_framework(dir: &Path) -> Option<&'static SitePreset> {
    // Order matters — more specific checks first
    if has_glob(dir, "astro.config.*") {
        return Some(&ASTRO);
    }
    if dir.join("app").is_dir()
        && (has_glob(dir, "next.config.*") || dir.join("next.config.js").exists())
    {
        return Some(&NEXTJS_APP);
    }
    if dir.join("pages").is_dir() && has_glob(dir, "next.config.*") {
        return Some(&NEXTJS_PAGES);
    }
    if has_glob(dir, "docusaurus.config.*") {
        return Some(&DOCUSAURUS);
    }
    if dir.join("mkdocs.yml").exists() {
        return Some(&MKDOCS);
    }
    if dir.join(".vitepress").is_dir() {
        return Some(&VITEPRESS);
    }
    if dir.join("book.toml").exists() {
        return Some(&MDBOOK);
    }
    if dir.join("conf.py").exists() {
        return Some(&SPHINX);
    }
    if has_glob(dir, "hugo.toml") || has_glob(dir, "hugo.yaml") || has_glob(dir, "hugo.json") {
        return Some(&HUGO);
    }
    if dir.join("content").is_dir() && dir.join("config.toml").exists() {
        // Hugo with legacy config
        return Some(&HUGO);
    }
    if dir.join("_config.yml").exists() || dir.join("_config.yaml").exists() {
        return Some(&JEKYLL);
    }
    None
}

/// Auto-detect source language from files in the given directory.
pub fn detect_source_language(dir: &Path) -> Option<&'static SourcePreset> {
    if dir.join("Cargo.toml").exists() {
        return Some(&RUST);
    }
    if dir.join("go.mod").exists() {
        return Some(&GO);
    }
    // TypeScript before JavaScript (tsconfig is the differentiator)
    if dir.join("tsconfig.json").exists() {
        return Some(&TYPESCRIPT);
    }
    if dir.join("package.json").exists() {
        return Some(&JAVASCRIPT);
    }
    if dir.join("pyproject.toml").exists()
        || dir.join("setup.py").exists()
        || dir.join("setup.cfg").exists()
    {
        return Some(&PYTHON);
    }
    if dir.join("pom.xml").exists() || dir.join("build.gradle").exists() {
        return Some(&JAVA);
    }
    if has_glob(dir, "*.csproj") || has_glob(dir, "*.sln") {
        return Some(&CSHARP);
    }
    if dir.join("Gemfile").exists() {
        return Some(&RUBY);
    }
    if dir.join("composer.json").exists() {
        return Some(&PHP);
    }
    if dir.join("CMakeLists.txt").exists() || dir.join("Makefile").exists() {
        // Could be C or C++; default to C++ as superset
        if has_glob(dir, "**/*.cpp") || has_glob(dir, "**/*.cc") {
            return Some(&CPP);
        }
    }
    None
}

fn has_glob(dir: &Path, pattern: &str) -> bool {
    let full = format!("{}/{pattern}", dir.display());
    glob::glob(&full)
        .map(|mut iter| iter.next().is_some())
        .unwrap_or(false)
}

/// Strip known extensions from a filename for route generation.
pub fn strip_page_extension(filename: &str, preset: &SitePreset) -> String {
    let mut result = filename.to_string();
    for ext in preset.extensions {
        if let Some(stripped) = result.strip_suffix(ext) {
            result = stripped.to_string();
            break;
        }
    }
    result
}
