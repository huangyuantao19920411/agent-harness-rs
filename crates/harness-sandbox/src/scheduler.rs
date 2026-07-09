use std::sync::Arc;

use tracing::info;

use crate::config::SandboxConfig;
use crate::error::{Result, SandboxError};
use crate::exec_manager::{ExecCommandRequest, ExecMode, ExecProcessManager};
use crate::k8s::{run_job_kube, K8sBackend};
use crate::policy::{IsolationLevel, SandboxPolicy};
use crate::process::ProcessSandbox;
use crate::traits::{ExecutionResult, Sandbox};
use crate::wasm::WasmSandbox;

/// Routes execution to the appropriate sandbox backend based on policy.
pub struct SandboxScheduler {
    policy: SandboxPolicy,
    process: ProcessSandbox,
    wasm: WasmSandbox,
    k8s_runtime_class: String,
    k8s_namespace: String,
    k8s_backend: K8sBackend,
    exec_mode: ExecMode,
    exec_manager: Option<Arc<ExecProcessManager>>,
}

impl SandboxScheduler {
    pub fn new(policy: SandboxPolicy, config: SandboxConfig) -> Result<Self> {
        let exec_mode = ExecMode::from_env();
        let exec_manager = if exec_mode.uses_exec_manager() {
            Some(Arc::new(ExecProcessManager::new(config.clone(), exec_mode)))
        } else {
            None
        };

        Ok(Self {
            policy,
            process: ProcessSandbox::new(config.clone()),
            wasm: WasmSandbox::new(&config)?,
            k8s_runtime_class: std::env::var("SANDBOX_RUNTIME_CLASS")
                .unwrap_or_else(|_| "gvisor".into()),
            k8s_namespace: std::env::var("SANDBOX_NAMESPACE")
                .unwrap_or_else(|_| "agent-sandbox".into()),
            k8s_backend: K8sBackend::from_env(),
            exec_mode,
            exec_manager,
        })
    }

    pub fn with_defaults() -> Result<Self> {
        Self::new(SandboxPolicy::default(), SandboxConfig::default())
    }

    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    pub fn k8s_backend(&self) -> K8sBackend {
        self.k8s_backend
    }

    pub fn exec_mode(&self) -> ExecMode {
        self.exec_mode
    }

    pub fn exec_manager(&self) -> Option<&Arc<ExecProcessManager>> {
        self.exec_manager.as_ref()
    }

    /// Execute a shell command using the isolation level for `task_type`.
    pub async fn exec(
        &self,
        task_type: &str,
        command: &str,
        args: &[&str],
    ) -> Result<ExecutionResult> {
        self.exec_with_process(task_type, command, args, None).await
    }

    /// Execute with optional managed worker reuse (`process_id` from prior call).
    pub async fn exec_with_process(
        &self,
        task_type: &str,
        command: &str,
        args: &[&str],
        process_id: Option<i32>,
    ) -> Result<ExecutionResult> {
        let level = self.policy.level_for(task_type);
        info!(?level, task_type, command, ?process_id, "sandbox scheduler routing");

        match level {
            IsolationLevel::Process => {
                if self.exec_mode.uses_exec_manager() {
                    self.exec_managed(command, args, process_id).await
                } else {
                    self.process.exec(command, args).await
                }
            }
            IsolationLevel::Wasm => Err(SandboxError::NotAvailable(
                "Wasm isolation requires exec_wasm() with bytecode; use task_type=trusted for shell".into(),
            )),
            IsolationLevel::MicroVm => self.exec_k8s_job(command, args).await,
        }
    }

    async fn exec_managed(
        &self,
        command: &str,
        args: &[&str],
        process_id: Option<i32>,
    ) -> Result<ExecutionResult> {
        let mgr = self.exec_manager.as_ref().ok_or_else(|| {
            SandboxError::NotAvailable("managed exec mode but no ExecProcessManager".into())
        })?;

        let (_id, result) = mgr
            .exec(ExecCommandRequest {
                process_id,
                command: command.into(),
                args: args.iter().map(|s| (*s).into()).collect(),
                cwd: None,
            })
            .await?;

        Ok(result)
    }

    /// Execute WASM bytecode using Wasm isolation (ignores policy default).
    pub async fn exec_wasm(
        &self,
        wasm_bytes: &[u8],
        func_name: &str,
        args: &[i32],
    ) -> Result<ExecutionResult> {
        self.wasm.exec_wasm(wasm_bytes, func_name, args).await
    }

    async fn exec_k8s_job(&self, command: &str, args: &[&str]) -> Result<ExecutionResult> {
        match self.k8s_backend {
            K8sBackend::Kube => {
                match run_job_kube(
                    &self.k8s_namespace,
                    &self.k8s_runtime_class,
                    command,
                    args,
                )
                .await
                {
                    Ok(result) => Ok(result),
                    Err(e) => {
                        tracing::warn!(%e, "kube backend failed, trying kubectl fallback");
                        self.exec_k8s_job_kubectl(command, args).await
                    }
                }
            }
            K8sBackend::Kubectl => self.exec_k8s_job_kubectl(command, args).await,
        }
    }

    /// Legacy kubectl CLI backend (local dev fallback).
    async fn exec_k8s_job_kubectl(&self, command: &str, args: &[&str]) -> Result<ExecutionResult> {
        let job_name = format!(
            "sandbox-{}",
            uuid::Uuid::new_v4().to_string()[..8].to_string()
        );
        let manifest = render_job_manifest(
            &job_name,
            &self.k8s_namespace,
            &self.k8s_runtime_class,
            command,
            args,
        );

        let manifest_path = std::env::temp_dir().join(format!("{job_name}.yaml"));
        std::fs::write(&manifest_path, &manifest)
            .map_err(|e| SandboxError::Execution(format!("write manifest: {e}")))?;

        let apply = tokio::process::Command::new("kubectl")
            .args(["apply", "-f", &manifest_path.to_string_lossy()])
            .output()
            .await
            .map_err(|e| SandboxError::NotAvailable(format!("kubectl not found: {e}")))?;

        if !apply.status.success() {
            let stderr = String::from_utf8_lossy(&apply.stderr);
            return Err(SandboxError::Execution(format!(
                "kubectl apply failed: {stderr}"
            )));
        }

        let wait = tokio::process::Command::new("kubectl")
            .args([
                "wait",
                "--for=condition=complete",
                &format!("job/{job_name}"),
                "-n",
                &self.k8s_namespace,
                "--timeout=60s",
            ])
            .output()
            .await
            .map_err(|e| SandboxError::Execution(format!("kubectl wait: {e}")))?;

        let logs = tokio::process::Command::new("kubectl")
            .args([
                "logs",
                &format!("job/{job_name}"),
                "-n",
                &self.k8s_namespace,
            ])
            .output()
            .await
            .map_err(|e| SandboxError::Execution(format!("kubectl logs: {e}")))?;

        let _ = tokio::process::Command::new("kubectl")
            .args([
                "delete",
                "job",
                &job_name,
                "-n",
                &self.k8s_namespace,
                "--wait=false",
            ])
            .output()
            .await;

        let _ = std::fs::remove_file(&manifest_path);

        Ok(ExecutionResult {
            stdout: String::from_utf8_lossy(&logs.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&logs.stderr).into_owned(),
            exit_code: if wait.status.success() {
                Some(0)
            } else {
                Some(1)
            },
            timed_out: false,
        })
    }
}

/// Render a K8s Job manifest with RuntimeClass for sandboxed execution.
pub fn render_job_manifest(
    job_name: &str,
    namespace: &str,
    runtime_class: &str,
    command: &str,
    args: &[&str],
) -> String {
    let args_yaml: String = args
        .iter()
        .map(|a| format!("            - {a:?}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"apiVersion: batch/v1
kind: Job
metadata:
  name: {job_name}
  namespace: {namespace}
  labels:
    app: agent-harness-sandbox
spec:
  ttlSecondsAfterFinished: 120
  backoffLimit: 0
  template:
    metadata:
      labels:
        app: agent-harness-sandbox
    spec:
      runtimeClassName: {runtime_class}
      restartPolicy: Never
      containers:
        - name: sandbox
          image: alpine:3.20
          command: [{command:?}]
          args:
{args_yaml}
          resources:
            limits:
              cpu: "500m"
              memory: "256Mi"
            requests:
              cpu: "100m"
              memory: "64Mi"
          securityContext:
            runAsNonRoot: true
            runAsUser: 65534
            readOnlyRootFilesystem: true
            allowPrivilegeEscalation: false
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_job_with_runtime_class() {
        let yaml = render_job_manifest("test-job", "agent-sandbox", "gvisor", "echo", &["hi"]);
        assert!(yaml.contains("runtimeClassName: gvisor"));
        assert!(yaml.contains("name: test-job"));
        assert!(yaml.contains("namespace: agent-sandbox"));
    }

    #[test]
    fn policy_routes_by_task_type() {
        let policy = SandboxPolicy::default();
        assert_eq!(policy.level_for("code"), IsolationLevel::Process);
        assert_eq!(policy.level_for("wasm"), IsolationLevel::Wasm);
        assert_eq!(policy.level_for("untrusted"), IsolationLevel::MicroVm);
        assert_eq!(policy.level_for("trusted"), IsolationLevel::Process);
    }
}
