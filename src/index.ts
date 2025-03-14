import concurrently, { Command } from "concurrently";
import consola from "consola";
import { execSync } from "node:child_process";
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
  cfTunnelConfig?: Omit<Partial<TunnelConfig>, "ingress">;

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
    concurrentlyOptions = {
      killOthers: "failure",
      handleInput: true,
    },
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

  // Filter out any commands that don't have a required command property
  const concurrentlyCommands = commands
    .filter((command) => typeof command.command === "string")
    .map((command) => {
      //
      const env = command.env || {};
      const tunnelEnv = { ...env, ...command.tunnelEnv };
      return {
        ...command, // Include everything
        command: command.command as string,
        env: tunnelEnabled ? tunnelEnv : env,
      };
    });

  if (tunnelEnabled) {
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

    concurrentlyCommands.push({
      name: "TUNN",
      // command: `CF_TOKEN=${cfToken} npx cf-tunnel --json '${JSON.stringify(tunnelConfig).replace(/'/g, String.raw`\'`)}'`,
      command: (() => {
        // Encode config as Base64 to avoid any string escaping issues
        const configBase64 = Buffer.from(JSON.stringify(tunnelConfig)).toString(
          "base64",
        );

        return `node --no-warnings -e "
          const { cfTunnel } = require('cf-tunnel');
          const tunnelConfig = JSON.parse(Buffer.from('${configBase64}', 'base64').toString());
          cfTunnel(tunnelConfig)
            .catch((error) => {
              console.error(error);
              process.exit(1);
            });
        "`;
      })(),
      cwd: undefined,
      env: {},
      prefixColor: "red",
    });
  } else {
    consola.info("Tunneling is disabled");
  }

  const execOptions = {
    env: { ...process.env, PATH: process.env.PATH },
    stdio: "inherit" as const, // Type assertion
  };

  // run before commands
  for (const command of beforeCommands) {
    execSync(command, execOptions);
  }

  const { result } = concurrently(concurrentlyCommands, concurrentlyOptions);
  await result;

  // run after commands
  for (const command of afterCommands) {
    execSync(command, execOptions);
  }

  // remove temp config file
  execSync(`rm -rf .tmp`);
}
