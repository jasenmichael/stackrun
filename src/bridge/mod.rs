use crate::cli::JitiMode;
use crate::error::Error;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const JITI_MISSING_CODE: i32 = 3;

const BRIDGE_SOURCE: &str = r#"
import { pathToFileURL } from "node:url";
import path from "node:path";

const file = process.env.STACKRUN_BRIDGE_FILE;
if (!file) {
  console.error("STACKRUN_BRIDGE_FILE is not set");
  process.exit(2);
}

async function loadJiti() {
  try {
    const mod = await import("jiti");
    return mod.createJiti ?? mod.default?.createJiti ?? mod.default;
  } catch {
    process.exit(3);
  }
}

const createJiti = await loadJiti();
if (typeof createJiti !== "function") {
  console.error("jiti did not export createJiti");
  process.exit(1);
}

const jiti = createJiti(pathToFileURL(process.cwd() + "/").href, { interopDefault: true });
let mod;
try {
  mod = await jiti.import(path.resolve(file));
} catch (err) {
  console.error(err && err.stack ? err.stack : err);
  process.exit(1);
}

let config = mod && typeof mod === "object" && "default" in mod ? mod.default : mod;
if (typeof config === "function") {
  config = await config({});
}
process.stdout.write(JSON.stringify(config ?? {}));
"#;

enum BridgeResult {
    Config(Value),
    JitiMissing,
    Failed(String),
}

/// Load a JS/TS config by spawning Node + Jiti. Never embeds a JS runtime.
///
/// Local first: `node` + `import("jiti")` from `cwd`. If jiti is missing and
/// `mode` is [`JitiMode::Npx`], retry via `npx -p jiti node ...`. Never installs
/// into the project.
pub fn load_js_ts(path: &Path, cwd: &Path, mode: JitiMode) -> Result<Value, Error> {
    let node = which("node").ok_or_else(|| Error::NodeRequired {
        path: path.to_path_buf(),
    })?;

    match run_bridge(&node, &[], path, cwd)? {
        BridgeResult::Config(value) => Ok(value),
        BridgeResult::JitiMissing => match mode {
            JitiMode::Local => Err(Error::JitiRequired {
                path: path.to_path_buf(),
            }),
            JitiMode::Npx => load_via_npx(path, cwd),
        },
        BridgeResult::Failed(message) => Err(Error::JsBridge {
            path: path.to_path_buf(),
            message,
        }),
    }
}

fn load_via_npx(path: &Path, cwd: &Path) -> Result<Value, Error> {
    let npx = which("npx").ok_or_else(|| Error::NpxRequired {
        path: path.to_path_buf(),
    })?;

    match run_bridge(&npx, &["-p", "jiti", "node"], path, cwd)? {
        BridgeResult::Config(value) => Ok(value),
        BridgeResult::JitiMissing => Err(Error::JsBridge {
            path: path.to_path_buf(),
            message: "npx -p jiti could not import jiti. First run may need network.".into(),
        }),
        BridgeResult::Failed(message) => Err(Error::JsBridge {
            path: path.to_path_buf(),
            message,
        }),
    }
}

fn run_bridge(
    program: &Path,
    prefix: &[&str],
    path: &Path,
    cwd: &Path,
) -> Result<BridgeResult, Error> {
    let output = spawn_bridge(program, prefix, path, cwd)?;
    interpret(path, output)
}

fn spawn_bridge(program: &Path, prefix: &[&str], path: &Path, cwd: &Path) -> Result<Output, Error> {
    Ok(Command::new(program)
        .args(prefix)
        .arg("--input-type=module")
        .arg("-e")
        .arg(BRIDGE_SOURCE)
        .env("STACKRUN_BRIDGE_FILE", path)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?)
}

fn interpret(path: &Path, output: Output) -> Result<BridgeResult, Error> {
    if output.status.code() == Some(JITI_MISSING_CODE) {
        return Ok(BridgeResult::JitiMissing);
    }
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Ok(BridgeResult::Failed(stderr.trim().to_string()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value = serde_json::from_str(&stdout).map_err(|err| Error::JsBridge {
        path: path.to_path_buf(),
        message: format!("Jiti bridge returned invalid JSON: {err}"),
    })?;
    Ok(BridgeResult::Config(value))
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        if cfg!(windows) {
            for ext in [".exe", ".cmd", ".bat", ""] {
                let candidate = dir.join(format!("{name}{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        } else {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
