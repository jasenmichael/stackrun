import { execSync } from "node:child_process";
import config from "./stack.config.js";

execSync("cd ../ && pnpm build");
const { stackrun } = await import("../dist/index.mjs");
stackrun(config);
