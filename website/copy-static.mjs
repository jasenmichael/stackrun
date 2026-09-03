import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dest = join(root, "website", "static", "install.sh");
mkdirSync(join(root, "website", "static"), { recursive: true });
copyFileSync(join(root, "scripts", "install.sh"), dest);
