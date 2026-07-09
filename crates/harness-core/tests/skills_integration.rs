use std::path::PathBuf;

use harness_core::{AgentLoop, HarnessConfig};
use harness_models::MockModel;
use harness_tools::{SkillRegistry, SkillsConfig, ToolRegistry};
use harness_trace::Tracer;

fn repo_skills_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.agents/skills")
}

#[tokio::test]
async fn agent_loop_registers_skill_tools() {
    let config = HarnessConfig {
        skills: SkillsConfig::enabled_with_paths([repo_skills_path()]),
        guardian: harness_core::GuardianConfig::disabled(),
        exec_policy: harness_core::ExecPolicy {
            mode: harness_core::ApprovalMode::Auto,
            ..Default::default()
        },
        max_iterations: 3,
        ..Default::default()
    };

    let registry = SkillRegistry::load(&config.skills);
    assert!(!registry.is_empty());

    let tools = ToolRegistry::new();
    let model = MockModel;
    let loop_engine = AgentLoop::new(model, tools, Tracer::new(), config);

    assert!(
        loop_engine
            .orchestrator()
            .schemas()
            .iter()
            .any(|s| s.name == "load_skill")
    );
}
