use std::time::Duration;

use tokio::task::spawn_blocking;
use tracing::debug;
use wasmtime::{Config, Engine, Instance, Module, Store, Val};

use crate::config::SandboxConfig;
use crate::error::{Result, SandboxError};
use crate::traits::ExecutionResult;

/// WebAssembly sandbox using wasmtime.
pub struct WasmSandbox {
    engine: Engine,
    timeout: Duration,
    max_output_bytes: usize,
}

impl WasmSandbox {
    pub fn new(config: &SandboxConfig) -> Result<Self> {
        let mut wasm_config = Config::new();
        wasm_config.consume_fuel(true);
        wasm_config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Disable);

        let engine = Engine::new(&wasm_config)
            .map_err(|e| SandboxError::NotAvailable(format!("wasmtime engine: {e}")))?;

        Ok(Self {
            engine,
            timeout: config.timeout,
            max_output_bytes: config.max_output_bytes,
        })
    }

    pub fn with_defaults() -> Result<Self> {
        Self::new(&SandboxConfig::default())
    }

    /// Execute a WASM module, calling `func_name` with i32 arguments.
    pub async fn exec_wasm(
        &self,
        wasm_bytes: &[u8],
        func_name: &str,
        args: &[i32],
    ) -> Result<ExecutionResult> {
        let engine = self.engine.clone();
        let timeout = self.timeout;
        let max_output = self.max_output_bytes;
        let wasm_bytes = wasm_bytes.to_vec();
        let func_name = func_name.to_string();
        let args = args.to_vec();

        let result = tokio::time::timeout(
            timeout,
            spawn_blocking(move || run_wasm(&engine, &wasm_bytes, &func_name, &args, max_output)),
        )
        .await;

        match result {
            Ok(Ok(inner)) => inner,
            Ok(Err(join_err)) => Err(SandboxError::Execution(format!("task join: {join_err}"))),
            Err(_) => Ok(ExecutionResult {
                stdout: String::new(),
                stderr: "wasm execution timed out".into(),
                exit_code: None,
                timed_out: true,
            }),
        }
    }

    pub fn name(&self) -> &'static str {
        "wasm"
    }
}

fn run_wasm(
    engine: &Engine,
    wasm_bytes: &[u8],
    func_name: &str,
    args: &[i32],
    max_output: usize,
) -> Result<ExecutionResult> {
    debug!(func = func_name, "wasm sandbox exec");

    let module = Module::new(engine, wasm_bytes)
        .map_err(|e| SandboxError::Execution(format!("compile wasm: {e}")))?;

    let mut store = Store::new(engine, ());
    store
        .set_fuel(1_000_000)
        .map_err(|e| SandboxError::Execution(format!("set fuel: {e}")))?;

    let instance = Instance::new(&mut store, &module, &[])
        .map_err(|e| SandboxError::Execution(format!("instantiate: {e}")))?;

    let func = instance
        .get_func(&mut store, func_name)
        .ok_or_else(|| SandboxError::Execution(format!("function not found: {func_name}")))?;

    let ty = func.ty(&store);
    let wasm_args: Vec<Val> = args.iter().map(|&v| Val::I32(v)).collect();
    let mut results = vec![Val::I32(0); ty.results().len()];

    func.call(&mut store, &wasm_args, &mut results)
        .map_err(|e| SandboxError::Execution(format!("call: {e}")))?;

    let output = results
        .iter()
        .map(|v| match v {
            Val::I32(n) => n.to_string(),
            Val::I64(n) => n.to_string(),
            Val::F32(n) => n.to_string(),
            Val::F64(n) => n.to_string(),
            other => format!("{other:?}"),
        })
        .collect::<Vec<_>>()
        .join(", ");

    let stdout = if output.len() > max_output {
        output[..max_output].to_string()
    } else {
        output
    };

    Ok(ExecutionResult {
        stdout,
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADD_WAT: &str = r#"
        (module
          (func (export "add") (param i32 i32) (result i32)
            local.get 0
            local.get 1
            i32.add)
        )
    "#;

    #[tokio::test]
    async fn wasm_add() {
        let sandbox = WasmSandbox::with_defaults().unwrap();
        let result = sandbox
            .exec_wasm(ADD_WAT.as_bytes(), "add", &[3, 5])
            .await
            .unwrap();
        assert_eq!(result.stdout, "8");
        assert_eq!(result.exit_code, Some(0));
    }
}
