use crate::{Error, Harness};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn resolve_in(target: &str, directories: &[PathBuf]) -> Result<PathBuf, Error> {
    let target = target.trim();
    if !target.is_empty() && Path::new(target).is_file() {
        return Ok(PathBuf::from(target));
    }
    let pattern = target.strip_suffix(".jsonl").unwrap_or(target);
    if pattern.is_empty() {
        return Err(Error::NotFound(target.into()));
    }
    for directory in directories {
        let mut best: Option<(u8, PathBuf)> = None;
        for entry in WalkDir::new(directory)
            .sort_by_file_name()
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.path().is_file() {
                continue;
            }
            let Some(base) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.strip_suffix(".jsonl"))
            else {
                continue;
            };
            let rank = if base == pattern
                || base.ends_with(&format!("_{pattern}"))
                || base.ends_with(&format!("-{pattern}"))
            {
                0
            } else if base.starts_with(pattern)
                || base.contains(&format!("_{pattern}"))
                || base.contains(&format!("-{pattern}"))
            {
                1
            } else if base.contains(pattern) {
                2
            } else {
                continue;
            };
            if best.as_ref().is_none_or(|(r, _)| rank < *r) {
                best = Some((rank, entry.into_path()));
            }
            if rank == 0 {
                break;
            }
        }
        if let Some((_, path)) = best {
            return Ok(path);
        }
    }
    Err(Error::NotFound(target.into()))
}

pub fn directories(harness: Harness) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    let mut env_path = |name: &str, suffix: &str| {
        if let Some(value) = std::env::var_os(name).filter(|s| !s.is_empty()) {
            let path = PathBuf::from(value);
            directories.push(if suffix.is_empty() {
                path
            } else {
                path.join(suffix)
            });
        }
    };
    let (name, subpaths, legacy): (&str, &[&str], &[&str]) = match harness {
        Harness::Claude => {
            env_path("CLAUDE_PROJECTS_DIR", "");
            env_path("CLAUDE_SESSIONS_DIR", "");
            (
                "claude-code",
                &["projects", "sessions"],
                &[".claude/projects", ".claude/sessions"],
            )
        }
        Harness::Pi => {
            env_path("PI_SESSIONS_DIR", "");
            ("pi", &["sessions"], &[".pi/agent/sessions"])
        }
        Harness::Codex => {
            env_path("CODEX_SESSIONS_DIR", "");
            env_path("CODEX_HOME", "sessions");
            ("codex", &["sessions"], &[".codex/sessions"])
        }
    };
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|s| !s.is_empty()) {
        for suffix in subpaths {
            directories.push(
                PathBuf::from(&xdg)
                    .join("ai-registry")
                    .join(name)
                    .join(suffix),
            );
        }
    }
    if let Some(home) = dirs::home_dir() {
        for suffix in legacy {
            directories.push(home.join(suffix));
        }
        for suffix in subpaths {
            directories.push(
                home.join(".local/share/ai-registry")
                    .join(name)
                    .join(suffix),
            );
        }
    }
    directories
}
