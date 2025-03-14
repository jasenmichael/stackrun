#!/usr/bin/env node

import { existsSync } from "node:fs";
import { defineCommand, runMain } from "citty";
import { consola } from "consola";
import { loadConfig } from "c12";
import { stackrun } from "./index";
import type { StackrunConfig } from "./index";

import { version, description, name } from "../package.json";

const main = defineCommand({
  meta: {
    name,
    version,
    description,
  },
  args: {
    config: {
      type: "string",
      description: "Path to config file",
      default: "stack.config",
      shortcut: "c",
      alias: "c",
    },
    json: {
      type: "string",
      description: "input config as JSON",
    },
    tunnel: {
      type: "boolean",
      description: "Enable tunneling",
      shortcut: "t",
      alias: "t",
    },
    help: {
      type: "boolean",
      description: "Show help",
      shortcut: "h",
      alias: "h",
    },
    V: { type: "boolean" },
  },
  async run({ args }) {
    if (args.version || args.V) {
      console.log(version);
      process.exit(0);
    }

    const configPath = args.c || args.config || args._[0];
    const c12Options = {
      name: "stack",
      // defaults to ${name}.config.{ts,js,mjs,cjs,json} if configPath is not provided
      configFile: typeof configPath === "string" ? configPath : undefined,
      cwd: process.cwd(),
      dotenv: true,
    };

    if (!c12Options.configFile) {
      consola.error("No config file found");
      process.exit(1);
    }

    const { config, configFile } = (await loadConfig(c12Options)) as {
      config: StackrunConfig;
      configFile: string;
    };

    // Override tunnelEnabled if --tunnel flag is provided
    if (args.tunnel || process.env.TUNNEL === "true") {
      config.tunnelEnabled = true;
    }

    if (!config) {
      consola.error(`No valid configuration found at ${configPath}`);
      process.exit(1);
    }

    if (existsSync(configFile)) {
      consola.info(`Running stackrun with the loaded config: ${configFile}`);
      await stackrun(config);
      process.exit(0);
    } else {
      consola.error(`No valid configuration found at ${configPath}`);
      process.exit(1);
    }
  },
});

runMain(main);
