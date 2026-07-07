# Agent Harness — Kubernetes Deployment

Deploy the coding-agent example and sandbox infrastructure to a K8s cluster.

## Prerequisites

- Kubernetes 1.25+
- `kubectl` configured
- (Optional) gVisor or Kata Containers installed on worker nodes

## Quick Start

```bash
# 1. Create namespace and RuntimeClasses
kubectl apply -f deploy/k8s/namespace.yaml
kubectl apply -f deploy/k8s/runtimeclass-gvisor.yaml
# kubectl apply -f deploy/k8s/runtimeclass-kata-firecracker.yaml  # if Kata available

# 2. Test sandbox Job manually
kubectl apply -f deploy/k8s/sandbox-job-example.yaml
kubectl logs -n agent-sandbox job/sandbox-example
kubectl delete -n agent-sandbox job sandbox-example

# 3. Use SandboxScheduler in Rust (auto-creates Jobs via kubectl)
export SANDBOX_RUNTIME_CLASS=gvisor
export SANDBOX_NAMESPACE=agent-sandbox
cargo run -p coding-agent -- "run untrusted task"
```

## Sandbox Isolation Levels

| RuntimeClass | Handler | Isolation | Requires |
|-------------|---------|-----------|----------|
| `gvisor` | runsc | User-space kernel | gVisor on nodes |
| `kata-fc` | kata-fc | KVM MicroVM | Kata Containers |

## Architecture

```
Agent Harness (SandboxScheduler)
    │
    ├── Process  → local subprocess (demo)
    ├── Wasm     → wasmtime bytecode (code execution)
    └── MicroVM  → K8s Job + RuntimeClass (untrusted)
                        │
                        ├── gvisor (runsc)
                        └── kata-fc (Firecracker VMM)
```

## Helm (optional)

```bash
helm install agent-harness deploy/helm/agent-harness \
  --set sandbox.runtimeClass=gvisor \
  --set deepseek.apiKey=$DEEPSEEK_API_KEY
```

See [docs/sandbox.md](../docs/sandbox.md) for the full isolation strategy.
