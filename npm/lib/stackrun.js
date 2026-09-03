import { spawn } from "node:child_process";
import { resolveBinary } from "./resolve-binary.js";

export function defineStackrunConfig(config) {
  return config;
}

function buildArgs(config, options = {}) {
  const args = [];
  if (config !== undefined && config !== null) {
    args.push("--json", JSON.stringify(config));
  }
  if (options.tunnel) {
    args.push("--tunnel");
  }
  if (options.dryRun) {
    args.push("--dry-run");
  }
  return args;
}

export function stackrun(config, options = {}) {
  const bin = resolveBinary();
  const args = buildArgs(config, options);
  return new Promise((resolve, reject) => {
    const child = spawn(bin, args, {
      stdio: "inherit",
      windowsHide: true,
    });

    const forward = (signal) => {
      if (child.killed) {
        return;
      }
      try {
        child.kill(signal);
      } catch {
        // child already gone
      }
    };
    process.on("SIGINT", forward);
    process.on("SIGTERM", forward);

    child.on("error", (err) => {
      process.off("SIGINT", forward);
      process.off("SIGTERM", forward);
      reject(err);
    });
    child.on("exit", (code, signal) => {
      process.off("SIGINT", forward);
      process.off("SIGTERM", forward);
      if (signal) {
        reject(new Error(`stackrun exited from ${signal}`));
        return;
      }
      const status = code ?? 1;
      if (status === 0) {
        resolve(0);
      } else {
        const err = new Error(`stackrun exited with code ${status}`);
        err.exitCode = status;
        reject(err);
      }
    });
  });
}

export { buildArgs };
