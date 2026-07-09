//! Kubernetes Job runner via `kube` crate (in-cluster / kubeconfig).
//!
//! Replaces external `kubectl` CLI for production deployments.

use std::time::Duration;

use k8s_openapi::api::batch::v1::{Job, JobSpec};
use k8s_openapi::api::core::v1::{
    Container, PodSpec, PodSecurityContext, PodTemplateSpec, ResourceRequirements, SecurityContext,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Api, DeleteParams, ListParams, LogParams, PostParams};
use kube::runtime::wait::{await_condition, conditions::is_job_completed};
use kube::{Client, Config};
use tracing::{info, warn};

use crate::error::{Result, SandboxError};
use crate::traits::ExecutionResult;

const JOB_LABEL: &str = "app";
const JOB_LABEL_VALUE: &str = "agent-harness-sandbox";

/// K8s API backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum K8sBackend {
    /// Use kube-rs client (default, works in-cluster with ServiceAccount).
    Kube,
    /// Fallback to external kubectl CLI.
    Kubectl,
}

impl K8sBackend {
    pub fn from_env() -> Self {
        match std::env::var("SANDBOX_K8S_BACKEND")
            .unwrap_or_else(|_| "kube".into())
            .to_lowercase()
            .as_str()
        {
            "kubectl" => Self::Kubectl,
            _ => Self::Kube,
        }
    }
}

/// Build a Job spec for sandboxed command execution.
pub fn build_job(
    job_name: &str,
    namespace: &str,
    runtime_class: &str,
    command: &str,
    args: &[&str],
) -> Job {
    Job {
        metadata: ObjectMeta {
            name: Some(job_name.to_string()),
            namespace: Some(namespace.to_string()),
            labels: Some(
                [(JOB_LABEL.into(), JOB_LABEL_VALUE.into())]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        },
        spec: Some(JobSpec {
            ttl_seconds_after_finished: Some(120),
            backoff_limit: Some(0),
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(
                        [(JOB_LABEL.into(), JOB_LABEL_VALUE.into())]
                            .into_iter()
                            .collect(),
                    ),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    runtime_class_name: Some(runtime_class.to_string()),
                    restart_policy: Some("Never".into()),
                    containers: vec![Container {
                        name: "sandbox".into(),
                        image: Some("alpine:3.20".into()),
                        command: Some(vec![command.to_string()]),
                        args: Some(args.iter().map(|s| s.to_string()).collect()),
                        resources: Some(ResourceRequirements {
                            limits: Some(
                                [
                                    ("cpu".into(), Quantity("500m".into())),
                                    ("memory".into(), Quantity("256Mi".into())),
                                ]
                                .into_iter()
                                .collect(),
                            ),
                            requests: Some(
                                [
                                    ("cpu".into(), Quantity("100m".into())),
                                    ("memory".into(), Quantity("64Mi".into())),
                                ]
                                .into_iter()
                                .collect(),
                            ),
                            ..Default::default()
                        }),
                        security_context: Some(SecurityContext {
                            run_as_non_root: Some(true),
                            run_as_user: Some(65534),
                            read_only_root_filesystem: Some(true),
                            allow_privilege_escalation: Some(false),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    security_context: Some(PodSecurityContext {
                        run_as_non_root: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Run a sandbox Job via kube-rs API.
pub async fn run_job_kube(
    namespace: &str,
    runtime_class: &str,
    command: &str,
    args: &[&str],
) -> Result<ExecutionResult> {
    let job_name = format!(
        "sandbox-{}",
        uuid::Uuid::new_v4().to_string()[..8].to_string()
    );

    info!(%job_name, namespace, runtime_class, command, "k8s: creating sandbox job");

    let config = Config::infer()
        .await
        .map_err(|e| SandboxError::NotAvailable(format!("k8s config infer: {e}")))?;
    let client = Client::try_from(config)
        .map_err(|e| SandboxError::NotAvailable(format!("k8s client: {e}")))?;

    let jobs: Api<Job> = Api::namespaced(client.clone(), namespace);
    let job = build_job(&job_name, namespace, runtime_class, command, args);

    jobs.create(&PostParams::default(), &job)
        .await
        .map_err(|e| SandboxError::Execution(format!("k8s create job: {e}")))?;

    let _ = tokio::time::timeout(
        Duration::from_secs(60),
        await_condition(jobs.clone(), &job_name, is_job_completed()),
    )
    .await
    .map_err(|_| SandboxError::Timeout(60))?
    .map_err(|e| SandboxError::Execution(format!("k8s wait job: {e}")))?;

    let job = jobs
        .get(&job_name)
        .await
        .map_err(|e| SandboxError::Execution(format!("k8s get job: {e}")))?;

    let success = job
        .status
        .as_ref()
        .and_then(|s| s.succeeded)
        .unwrap_or(0)
        > 0;

    let stdout = fetch_job_logs(&client, namespace, &job_name).await.unwrap_or_else(|e| {
        warn!(%e, "k8s: failed to fetch logs");
        String::new()
    });

    // Cleanup (best effort)
    let _ = jobs
        .delete(&job_name, &DeleteParams::background())
        .await;

    Ok(ExecutionResult {
        stdout,
        stderr: if success {
            String::new()
        } else {
            "job failed or did not succeed".into()
        },
        exit_code: Some(if success { 0 } else { 1 }),
        timed_out: false,
    })
}

async fn fetch_job_logs(client: &Client, namespace: &str, job_name: &str) -> Result<String> {
    use k8s_openapi::api::core::v1::Pod;

    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let lp = ListParams::default().labels(&format!("job-name={job_name}"));
    let pod_list = pods
        .list(&lp)
        .await
        .map_err(|e| SandboxError::Execution(format!("k8s list pods: {e}")))?;

    let pod_name = pod_list
        .items
        .first()
        .and_then(|p| p.metadata.name.clone())
        .ok_or_else(|| SandboxError::Execution("no pod found for job".into()))?;

    let logs = pods
        .logs(&pod_name, &LogParams::default())
        .await
        .map_err(|e| SandboxError::Execution(format!("k8s pod logs: {e}")))?;

    Ok(logs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_job_with_runtime_class() {
        let job = build_job("test-job", "agent-sandbox", "gvisor", "echo", &["hi"]);
        let spec = job.spec.as_ref().unwrap();
        let pod_spec = spec.template.spec.as_ref().unwrap();
        assert_eq!(pod_spec.runtime_class_name.as_deref(), Some("gvisor"));
        assert_eq!(job.metadata.name.as_deref(), Some("test-job"));
    }

    #[test]
    fn backend_from_env_defaults_kube() {
        std::env::remove_var("SANDBOX_K8S_BACKEND");
        assert_eq!(K8sBackend::from_env(), K8sBackend::Kube);
    }
}
