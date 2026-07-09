use std::path::{Path, PathBuf};

use crate::skills::Skill;
use crate::skills::SkillError;

/// Parse a `SKILL.md` file (YAML frontmatter + markdown body).
pub fn parse_skill_file(path: &Path, content: &str) -> Result<Skill, SkillError> {
    let (name, description, body) = parse_frontmatter(content)?;
    if name.is_empty() {
        return Err(SkillError::InvalidFrontmatter(format!(
            "{}: missing name",
            path.display()
        )));
    }
    if description.is_empty() {
        return Err(SkillError::InvalidFrontmatter(format!(
            "{}: missing description",
            path.display()
        )));
    }
    if name.len() > 100 {
        return Err(SkillError::InvalidFrontmatter(format!(
            "{}: name exceeds 100 chars",
            path.display()
        )));
    }
    if description.len() > 500 {
        return Err(SkillError::InvalidFrontmatter(format!(
            "{}: description exceeds 500 chars",
            path.display()
        )));
    }

    Ok(Skill {
        name: sanitize_one_line(&name),
        description: sanitize_one_line(&description),
        path: path.to_path_buf(),
        body: body.to_string(),
    })
}

fn sanitize_one_line(s: &str) -> String {
    s.replace('\n', " ").trim().to_string()
}

fn parse_frontmatter(content: &str) -> Result<(String, String, &str), SkillError> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let rest = content
        .strip_prefix("---")
        .ok_or_else(|| SkillError::InvalidFrontmatter("missing opening ---".into()))?;
    let rest = rest.strip_prefix('\n').unwrap_or(rest);

    let end = rest
        .find("\n---")
        .ok_or_else(|| SkillError::InvalidFrontmatter("missing closing ---".into()))?;
    let yaml = &rest[..end];
    let body = rest[end + 4..].strip_prefix('\n').unwrap_or(&rest[end + 4..]);

    let mut name = String::new();
    let mut description = String::new();

    for line in yaml.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("name:") {
            name = v.trim().trim_matches('"').trim_matches('\'').to_string();
        } else if let Some(v) = line.strip_prefix("description:") {
            description = v.trim().trim_matches('"').trim_matches('\'').to_string();
        }
    }

    Ok((name, description, body))
}

/// Recursively discover `SKILL.md` files under `root`.
pub fn discover_skill_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk_dir(root, &mut found);
    found.sort();
    found
}

fn walk_dir(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, out);
        } else if path.file_name().is_some_and(|n| n == "SKILL.md") {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter() {
        let md = r#"---
name: rust-review
description: Review Rust code for safety and idioms
---

# Rust Review
Check ownership and error handling.
"#;
        let skill = parse_skill_file(Path::new("SKILL.md"), md).unwrap();
        assert_eq!(skill.name, "rust-review");
        assert!(skill.description.contains("Review Rust"));
        assert!(skill.body.contains("ownership"));
    }
}
