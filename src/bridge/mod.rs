use crate::error::Error;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};

const BRIDGE_SOURCE: &str = r#"
import { createRequire } from "node:module";
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
  } catch (err) {
    console.error(
      "Could not import jiti. Install it in this project (`npm i jiti`) or use a native config file (YAML, TOML, JSON).",
    );
    console.error(err && err.message ? err.message : err);
    process.exit(1);
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

/// Load a JS/TS config by spawning Node + Jiti. Never embeds a JS runtime.
pub fn load_js_ts(path: &Path) -> Result<Value, Error> {
    let node = which_node().ok_or_else(|| Error::NodeRequired {
        path: path.to_path_buf(),
    })?;

    let output = Command::new(node)
        .arg("--input-type=module")
        .arg("-e")
        .arg(BRIDGE_SOURCE)
        .env("STACKRUN_BRIDGE_FILE", path)
        .current_dir(
            std::env::current_dir().unwrap_or_else(|_| path.parent().unwrap_or(path).to_path_buf()),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::JsBridge {
            path: path.to_path_buf(),
            message: stderr.trim().to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).map_err(|err| Error::JsBridge {
        path: path.to_path_buf(),
        message: format!("Jiti bridge returned invalid JSON: {err}"),
    })
}

fn which_node() -> Option<std::path::PathBuf> {
    let name = if cfg!(windows) { "node.exe" } else { "node" };
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
