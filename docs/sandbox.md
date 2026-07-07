# Agent 沙箱方案评估

> 是否应该自己做一个类似 Firecracker 的沙箱？

## 结论（先说答案）

**不建议从零自研 Firecracker 级别的微虚拟机。** 更务实的路径是：

```
Phase 1: ProcessSandbox（已实现）     → 超时 / cwd / env 隔离
Phase 2: Wasm 沙箱（wasmtime）         → 轻量、跨平台、适合代码执行
Phase 3: 集成 gVisor / Firecracker     → 生产级不可信代码隔离
Phase 4: K8s 编排 + 自定义调度层        → 海量 Agent 并发（DeepSeek JD 方向）
```

我们的差异化应该在 **Agent Harness 编排层**（调度、生命周期、Trace），而不是重造 VM 内核。

---

## 为什么不应该自研 Firecracker

| 维度 | Firecracker 自研 | 集成现有方案 |
|------|-----------------|-------------|
| **开发周期** | 1-2 年+（内核、设备模型、网络） | 数周（集成 + 适配） |
| **安全审计** | 需要专业安全团队 | 社区已验证（AWS 生产使用） |
| **维护成本** | 持续跟进内核 CVE | 上游维护 |
| **面试价值** | 展示系统深度 | 展示工程判断力和架构能力 |
| **与 JD 匹配** | 间接（底层能力） | 直接（Harness + 容器调度平台） |

Firecracker 的核心价值是 **KVM 微虚拟机 + 极简设备模型**，实现毫秒级启动和强隔离。这是 AWS 团队多年投入的基础设施，不是 2-3 周开源项目能复制的。

---

## 各方案对比

### 1. Process Sandbox（当前实现：`harness-sandbox`）

```
优点：零依赖、跨平台、实现简单
缺点：共享内核，无法防御恶意 syscall
适用：可信工具、Demo、开发阶段
```

已实现：`ProcessSandbox` — 子进程 + 超时 + cwd 限制 + env 过滤 + 输出截断

### 2. WebAssembly Sandbox（推荐 Phase 2）

```
优点：跨平台、轻量（~1ms 启动）、内存安全、可嵌入 Rust
缺点：需要 WASI 支持、不能直接跑任意二进制
适用：AI 生成代码执行、插件系统
```

技术选型：**wasmtime**（Rust 原生，Bytecode Alliance）

```rust
// 未来 API 示意
let sandbox = WasmSandbox::new(config);
sandbox.exec_wasm(wasm_bytes, "main", &[]).await?;
```

### 3. gVisor（推荐 Phase 3 - Linux 生产）

```
优点：用户态内核、syscall 拦截、比容器安全、比 VM 轻
缺点：仅 Linux、部分 syscall 性能损耗
适用：生产环境不可信代码
```

集成方式：作为 K8s RuntimeClass，Harness 通过容器 API 调度

### 4. Firecracker（推荐 Phase 3 - 高安全场景）

```
优点：硬件级隔离（KVM）、AWS 生产验证、毫秒启动
缺点：仅 Linux + KVM、需要嵌套虚拟化
适用：多租户 Agent 平台、不可信代码执行
```

集成方式：通过 **firecracker-containerd** 或 **Kata Containers** 间接使用，不自研 VM

### 5. 自研微 VM

```
优点：完全可控
缺点：投入巨大、安全风险高、重复造轮子
适用：只有当你有专门的虚拟化团队和 1 年+ 时间
```

**结论：不做。**

---

## 推荐架构（贴合 DeepSeek JD）

```
┌─────────────────────────────────────────────┐
│           Agent Harness Platform             │
│  ┌─────────┐  ┌──────────┐  ┌───────────┐ │
│  │ Agent   │  │ Sandbox  │  │ MCP       │ │
│  │ Loop    │  │ Scheduler│  │ Client    │ │
│  └────┬────┘  └────┬─────┘  └─────┬─────┘ │
│       └────────────┼──────────────┘        │
├────────────────────┼───────────────────────┤
│         Sandbox Runtime Layer                │
│  ┌─────────┐  ┌─────────┐  ┌────────────┐  │
│  │ Process │  │  Wasm   │  │ MicroVM    │  │
│  │ Sandbox │  │ Sandbox │  │ (FC/gVisor)│  │
│  └─────────┘  └─────────┘  └────────────┘  │
├─────────────────────────────────────────────┤
│         Kubernetes + RuntimeClass            │
│    firecracker / gvisor / runc               │
└─────────────────────────────────────────────┘
```

### Harness 层的职责（我们应该做的）

1. **Sandbox 生命周期管理** — 创建、执行、销毁、超时、资源配额
2. **调度策略** — 按任务类型选择 Sandbox 后端（Wasm for code, MicroVM for untrusted）
3. **执行 Trace** — 记录每次沙箱执行的输入/输出/资源消耗
4. **安全策略** — 网络隔离、文件系统白名单、权限控制
5. **K8s 集成** — Sandbox Pod 的弹性扩缩容

### 不应该做的

- 自研 KVM 微虚拟机
- 自研 syscall 拦截层
- 自研容器运行时

---

## 开源项目路线

| 阶段 | 内容 | 时间 | 面试价值 |
|------|------|------|----------|
| **现在** | `ProcessSandbox` + 本文档 | 已完成 | 展示工程判断力 |
| **2 周** | `WasmSandbox` (wasmtime) | MVP | 展示 Rust 系统编程 |
| **1 月** | K8s RuntimeClass 集成 demo | PoC | 展示 K8s + 安全架构 |
| **持续** | Sandbox 调度器 + Trace | 迭代 | 直接对标 JD |

---

## 面试话术

> 在评估 Agent 沙箱方案时，我认为不应该从零自研 Firecracker 级别的微 VM。我们的核心价值在 Harness 编排层：根据任务风险等级选择隔离后端（Process → Wasm → MicroVM），管理沙箱生命周期，并将执行 Trace 反馈给模型训练。Firecracker/gVisor 通过 K8s RuntimeClass 集成即可，这与 DeepSeek JD 中「下一代容器调度与隔离平台」的方向一致——重点是调度层，不是重造 VM。
