use harness_sandbox::SandboxScheduler;

const ADD_WAT: &str = r#"
    (module
      (func (export "add") (param i32 i32) (result i32)
        local.get 0
        local.get 1
        i32.add)
    )
"#;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let scheduler = SandboxScheduler::with_defaults().expect("scheduler init failed");
    let policy = scheduler.policy();

    println!("=== SandboxScheduler Demo ===\n");
    println!("Policy routing:");
    println!("  trusted  -> {:?}", policy.level_for("trusted"));
    println!("  code     -> {:?}", policy.level_for("code"));
    println!("  untrusted -> {:?}\n", policy.level_for("untrusted"));

    // Phase 1: Process
    println!("--- Phase 1: Process Sandbox ---");
    let r = scheduler.exec("trusted", "echo", &["hello from process"]).await;
    match r {
        Ok(res) => println!("stdout: {}", res.stdout.trim()),
        Err(e) => eprintln!("error: {e}"),
    }

    // Phase 2: Wasm
    println!("\n--- Phase 2: Wasm Sandbox ---");
    let r = scheduler.exec_wasm(ADD_WAT.as_bytes(), "add", &[3, 5]).await;
    match r {
        Ok(res) => println!("add(3, 5) = {}", res.stdout.trim()),
        Err(e) => eprintln!("error: {e}"),
    }

    // Phase 3: K8s MicroVM (requires kubectl + RuntimeClass)
    println!("\n--- Phase 3: K8s MicroVM (gVisor/Kata) ---");
    if std::env::var("SKIP_K8S_SANDBOX").is_ok() {
        println!("skipped (SKIP_K8S_SANDBOX=1)");
    } else if which_kubectl() {
        let runtime = std::env::var("SANDBOX_RUNTIME_CLASS").unwrap_or_else(|_| "gvisor".into());
        println!("runtimeClass: {runtime}");
        let r = scheduler
            .exec("untrusted", "echo", &["hello from microvm"])
            .await;
        match r {
            Ok(res) => println!("stdout: {}", res.stdout.trim()),
            Err(e) => eprintln!(
                "K8s sandbox failed (expected if RuntimeClass not installed): {e}"
            ),
        }
    } else {
        println!("kubectl not found, skipping K8s demo");
    }

    println!("\nDone.");
}

fn which_kubectl() -> bool {
    std::process::Command::new("kubectl")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
