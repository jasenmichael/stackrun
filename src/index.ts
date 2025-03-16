import { execSync } from "node:child_process";
import chalk from "chalk";
import concurrently, { Command } from "concurrently";
import consola from "consola";
import type { TunnelConfig } from "cf-tunnel";

// Get options type from concurrently's function parameters
type ConcurrentlyOptions = Parameters<typeof concurrently>[1];

/**
 * Configuration for a command to be run by stackrun.
 * Extends all concurrently Command options (command, name, prefixColor, cwd, env, etc.)
 * with additional tunnel-specific properties.
 * @see https://www.npmjs.com/package/concurrently#command
 */
export type StackrunConfigCommands = Partial<Command> & {
  /**
   * The local URL that will be tunneled to when tunneling is enabled.
   * Required with tunnelUrl for tunnel creation.
   */
  url?: string;

  /**
   * Environment variables for the command when tunneling is enabled.
   * These override the regular env variables when tunnelEnabled is true.
   */
  tunnelEnv?: Record<string, string | boolean | undefined>;

  /**
   * The public URL for the tunnel.
   * Required with url for tunnel creation.
   */
  tunnelUrl?: string;
};

/**
 * Configuration for stackrun.
 * @property concurrentlyOptions - All options from concurrently are supported.
 * @see https://www.npmjs.com/package/concurrently#api
 */
export type StackrunConfig = {
  /**
   * All concurrently options are supported.
   * @see https://www.npmjs.com/package/concurrently#concurrentlycommands-options
   */
  concurrentlyOptions?: ConcurrentlyOptions;

  /** When true, creates tunnels for services with url and tunnelUrl defined */
  tunnelEnabled?: boolean;

  /**
   * Configuration for Cloudflare Tunnel using cf-tunnel package.
   * All cf-tunnel configuration options are supported EXCEPT 'ingress',
   * which is automatically generated from commands with url and tunnelUrl properties.
   * @see https://www.npmjs.com/package/cf-tunnel#configuration-options
   */
  cfTunnelConfig?: Omit<Partial<TunnelConfig>, "ingress"> & {
    /**
     * Options for the concurrently command that runs the tunnel.
     * These customize how the tunnel command appears in the output.
     */
    commandOptions?: {
      /** Name for the tunnel command in the output */
      name?: string;
      /** Color for the tunnel command prefix */
      prefixColor?: string;
      /** Environment variables for the tunnel command */
      env?: Record<string, string | boolean | undefined>;
      /** Working directory for the tunnel command */
      cwd?: string;
      /** Whether to enable IPC for the tunnel command */
      ipc?: number;
    };
  };

  /** Commands to run before starting the concurrent processes */
  beforeCommands?: string[];

  /** Commands to run after all concurrent processes have completed */
  afterCommands?: string[];

  /** List of commands to run concurrently, with optional tunnel configuration */
  commands?: StackrunConfigCommands[];
};

export function defineStackrunConfig(config: StackrunConfig) {
  return config;
}

export async function stackrun(config: StackrunConfig) {
  //  set defaults
  const {
    concurrentlyOptions = {},
    tunnelEnabled = false,
    cfTunnelConfig = {
      cloudflaredConfigDir: undefined,
      cfToken: process.env.CLOUDFLARE_TOKEN,
      tunnelName: process.env.CLOUDFLARE_TUNNEL_NAME || "stackrun",
      removeExistingTunnel: false,
      removeExistingDns: false,
    },
    beforeCommands = [],
    afterCommands = [],
    commands = [],
  } = config;

  concurrentlyOptions.killOthers = concurrentlyOptions?.killOthers || "failure";
  concurrentlyOptions.handleInput = concurrentlyOptions?.handleInput || true;
  concurrentlyOptions.prefixColors =
    concurrentlyOptions?.prefixColors || "auto";
  concurrentlyOptions.prefixLength = concurrentlyOptions?.prefixLength || 10;

  // Filter out any commands that don't have a required command property
  const concurrentlyCommands = commands
    .filter((command) => typeof command.command === "string")
    .map((command) => {
      //
      const env = command.env || {};
      const tunnelEnv = { ...env, ...command.tunnelEnv };
      return {
        ...command, // Include everything
        name: (() => {
          if (!command.name) return undefined;
          if (concurrentlyOptions.prefixLength) {
            return command.name.slice(0, concurrentlyOptions.prefixLength);
          }
          return command.name;
        })(),
        command: command.command as string,
        env: tunnelEnabled ? tunnelEnv : env,
      };
    });

  if (tunnelEnabled) {
    consola.info("Tunneling is enabled");

    const cfToken =
      cfTunnelConfig.cfToken ||
      process.env.CF_TOKEN ||
      process.env.CLOUDFLARE_TOKEN;

    if (!cfToken) {
      consola.error("Cloudflare token is required for tunneling");
      return;
    }

    const tunnelName =
      cfTunnelConfig.tunnelName ||
      process.env.CF_TUNNEL_NAME ||
      process.env.CLOUDFLARE_TUNNEL_NAME ||
      "stackrun";

    const ingress = commands
      .filter((cmd) => cmd.tunnelUrl && cmd.url)
      .map((command) => ({
        hostname: command
          .tunnelUrl!.replace("https://", "")
          .replace("http://", ""),
        service: command.url!,
      }));

    if (ingress.length === 0) {
      consola.warn("No valid tunnel configurations found");
      return;
    }

    const removeExistingDns = cfTunnelConfig.removeExistingDns || false;
    const removeExistingTunnel = cfTunnelConfig.removeExistingTunnel || false;

    const tunnelConfig = {
      cfToken,
      tunnelName,
      ingress,
      removeExistingDns,
      removeExistingTunnel,
    };

    const tunnelCommand = {
      // command: `node --no-warnings -e "require('cf-tunnel').cfTunnel(${JSON.stringify(JSON.stringify(tunnelConfig))})"`,
      command: `node --no-warnings -e 'require("cf-tunnel").cfTunnel(${JSON.stringify(tunnelConfig)})'`,
      // { name, prefixColor, env, cwd, ipc }
      name: (() => {
        // Get the raw display name first
        let displayName = cfTunnelConfig?.commandOptions?.name || "Tunnel";

        // Apply prefixLength truncation if needed
        if (concurrentlyOptions.prefixLength) {
          displayName = displayName.slice(0, concurrentlyOptions.prefixLength);
        }

        // Now apply colors - either a single color or rainbow effect
        if (cfTunnelConfig?.commandOptions?.prefixColor) {
          return displayName; // Let concurrently apply the color via prefixColor
        } else {
          // Create rainbow effect when no prefixColor is provided
          const rainbowColors = [
            "red",
            "yellowBright",
            "yellow",
            "green",
            "blue",
            "magenta",
            "cyan",
          ];

          return [...displayName]
            .map((char, i) => {
              const colorIndex = i % rainbowColors.length;
              const color = rainbowColors[colorIndex];
              // @ts-ignore - Using dynamic color lookup
              return chalk[color](char);
            })
            .join("");
        }
      })(),
      prefixColor: cfTunnelConfig?.commandOptions?.prefixColor,
      env: cfTunnelConfig?.commandOptions?.env || {},
      cwd: cfTunnelConfig?.commandOptions?.cwd || undefined,
      ipc: cfTunnelConfig?.commandOptions?.ipc || undefined,
    };

    concurrentlyCommands.push(tunnelCommand);
  } else {
    consola.info("Tunneling is disabled");
  }

  const execOptions = {
    env: { ...process.env, PATH: process.env.PATH },
    stdio: "inherit" as const,
  };

  // run before commands
  if (beforeCommands.length > 0) {
    consola.info("Running beforeCommands");

    for (const command of beforeCommands) {
      consola.info(`Running beforeCommand: ${command}`);
      execSync(command, execOptions);
    }
  } else {
    consola.info("No beforeCommands to run");
  }

  const { result, commands: cmds } = concurrently(
    concurrentlyCommands,
    concurrentlyOptions,
  );
  await result;

  // run after commands
  if (afterCommands.length > 0) {
    consola.info("Running afterCommands");

    for (const command of afterCommands) {
      consola.info(`Running afterCommand: ${command}`);
      execSync(command, execOptions);
    }
  } else {
    consola.info("No afterCommands to run");
  }

  if (cmds && typeof cmds[Symbol.iterator] === "function") {
    for (const cmd of cmds) {
      consola.info(`Command ${cmd.name} ${cmd.state}`);
    }
  }

  consola.info("Stackrun completed");
}
