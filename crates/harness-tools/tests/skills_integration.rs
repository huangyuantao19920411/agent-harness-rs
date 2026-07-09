use std::path::PathBuf;

use harness_tools::{SkillRegistry, SkillsConfig};

fn repo_skills_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.agents/skills")
}

#[test]
fn loads_example_rust_review_skill() {
    let config = SkillsConfig::enabled_with_paths([repo_skills_path()]);
    let registry = SkillRegistry::load(&config);
    assert!(registry.len() >= 1, "expected at least one skill");
    let skill = registry.get("rust-review").expect("rust-review skill");
    assert!(skill.description.contains("Review Rust"));
    let content = registry.load_content("rust-review").unwrap();
    assert!(content.contains("Ownership"));
}

#[test]
fn format_catalog_lists_skills() {
    let config = SkillsConfig::enabled_with_paths([repo_skills_path()]);
    let registry = SkillRegistry::load(&config);
    let catalog = registry.format_catalog();
    assert!(catalog.contains("## Skills"));
    assert!(catalog.contains("rust-review"));
}
