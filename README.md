# 🥞 stackrun <span style="transform: scaleX(-1); display: inline-block;">🏃</span>

stackrun is a wrapper around [concurrently](https://www.npmjs.com/package/concurrently) and [cf-tunnel](https://www.npmjs.com/package/cf-tunnel) that simplifies running multiple services with optional integrated Cloudflare tunneling.

<!-- automd:badges name="stackrun" codecov license -->

[![npm version](https://img.shields.io/npm/v/stackrun)](https://npmjs.com/package/stackrun)
[![npm downloads](https://img.shields.io/npm/dm/stackrun)](https://npm.chart.dev/stackrun)
[![codecov](https://img.shields.io/codecov/c/gh/jasenmichael/stackrun)](https://codecov.io/gh/jasenmichael/stackrun)
[![license](https://img.shields.io/github/license/jasenmichael/stackrun)](https://github.com/jasenmichael/stackrun/blob/main/LICENSE)

<!-- /automd -->

## ✅ Features

- **Simple Configuration**: Single config file for all your services and tunneling needs
- **Service Execution**: Run multiple services simultaneously with [concurrently](https://github.com/open-cli-tools/concurrently)
- **Cloudflare Tunneling**: Built-in support for exposing local services via [cf-tunnel](https://www.npmjs.com/package/cf-tunnel)
- **Flexible Configuration**: TS/JS/JSON config files with [c12](https://github.com/unjs/c12) loader
- **Environment Management**: Define regular and tunnel-specific environment variables for each service
- **Lifecycle Hooks**: Run commands before starting and after stopping your services
- **Smart Output Handling**: Color-coded service output with customizable prefixes

## 🚀 Usage

CLI:

Install package globally:

```sh
# npm
npm install -g stackrun

# pnpm
pnpm install -g stackrun

```

CLI usage:

```sh
# Run with default config file
stackrun

# With custom config file
stackrun -c custom.config.js

# Enable tunneling
stackrun --tunnel
# or set env var
TUNNEL=true stackrun
```

Configuration file:

```ts
// stack.config.ts
import { defineStackrunConfig } from "stackrun";

export default defineStackrunConfig({
  concurrentlyOptions: { killOthers: "failure" },
  commands: [
    {
      name: "api",
      command: "npm run dev",
      cwd: "./api",
      url: "http://localhost:4000",
      tunnelUrl: "https://api.example.dev",
    },
    {
      name: "web",
      command: "npm run dev",
      cwd: "./web",
    },
  ],
});
```

API:

Install package:

<!-- automd:pm-install name="stackrun" -->

```sh
# ✨ Auto-detect
npx nypm install stackrun

# npm
npm install stackrun

# yarn
yarn add stackrun

# pnpm
pnpm install stackrun

# bun
bun install stackrun

# deno
deno install stackrun
```

<!-- /automd -->

API usage:

<!-- automd:jsimport cjs name="stackrun" imports="stackrun,defineStackrunConfig" -->

**ESM** (Node.js, Bun, Deno)

```js
import { stackrun, defineStackrunConfig } from "stackrun";
```

**CommonJS** (Legacy Node.js)

```js
const { stackrun, defineStackrunConfig } = require("stackrun");
```

<!-- /automd -->

## ⚙️ Options

### `concurrentlyOptions`

- Type: `ConcurrentlyOptions`
- Default: `{ killOthers: "failure", handleInput: true, prefixColors: "auto" }`

All options from the [concurrently API](https://github.com/open-cli-tools/concurrently#concurrentlycommands-options) including killOthers, prefix formatting, max processes, and more.

### `tunnelEnabled`

- Type: `boolean`
- Default: `false`

When true, creates tunnels for services with `url` and `tunnelUrl` defined.

### `cfTunnelConfig`

- Type: `Omit<TunnelConfig, "ingress">`
- Default:

  ```
  {
    cfToken: process.env.CLOUDFLARE_TOKEN,
    tunnelName: "stackrun",
    removeExistingTunnel: false,
    removeExistingDns: false,
  }

  ```

Configuration for [cf-tunnel](https://www.npmjs.com/package/cf-tunnel). All options except `ingress` are supported (ingress is automatically generated from command entries).

### `beforeCommands`

- Type: `string[]`
- Default: `[]`

Commands to run before starting the services.

### `afterCommands`

- Type: `string[]`
- Default: `[]`

Commands to run after all services have completed.

### `commands`

- Type: `StackrunConfigCommands[]`
- Required: Yes

An array of command configurations to run concurrently. Extends [concurrently's Command type](https://github.com/open-cli-tools/concurrently#command) with additional stackrun-specific properties for tunneling.

Each command configuration supports:

| Option        | Type                                             | Description                                                               |
| ------------- | ------------------------------------------------ | ------------------------------------------------------------------------- |
| `command`     | `string`                                         | Command to run (required)                                                 |
| `name`        | `string`                                         | Name for the command in logs                                              |
| `cwd`         | `string`                                         | Working directory for the command                                         |
| `env`         | `Record<string, string \| boolean \| undefined>` | Environment variables                                                     |
| `prefixColor` | `string`                                         | Color for the command prefix                                              |
| `url`         | `string`                                         | Local URL for tunneling (required for tunnel creation)                    |
| `tunnelUrl`   | `string`                                         | Public URL for tunnel (required for tunnel creation)                      |
| `tunnelEnv`   | `Record<string, string \| boolean \| undefined>` | Environment variables that override regular env when tunneling is enabled |

Additional options from [concurrently's Command](https://github.com/open-cli-tools/concurrently#command) are also supported.

## 📝 Examples

### Basic Stack

```ts
// stack.config.ts
import { defineStackrunConfig } from "stackrun";

export default defineStackrunConfig({
  commands: [
    {
      name: "api",
      command: "npm run dev",
      cwd: "./api",
      prefixColor: "green",
    },
    {
      name: "web",
      command: "npm run dev",
      cwd: "./web",
      prefixColor: "blue",
    },
  ],
});
```

### With Docker and Tunneling

```ts
// stack.config.ts
import { defineStackrunConfig } from "stackrun";
import "dotenv/config";

export default defineStackrunConfig({
  tunnelEnabled: true,
  cfTunnelConfig: {
    cfToken: process.env.CF_TOKEN,
    tunnelName: "my-project",
    removeExistingTunnel: true,
  },
  beforeCommands: ["docker compose -f docker-compose.dev.yml up -d db"],
  afterCommands: ["docker compose -f docker-compose.dev.yml down db"],
  commands: [
    {
      name: "api",
      command: "npm run dev",
      cwd: "./api",
      url: "http://localhost:4000",
      tunnelUrl: "https://api.example.dev",
      prefixColor: "green",
    },
    {
      name: "web",
      command: "npm run dev",
      cwd: "./web",
      url: "http://localhost:3000",
      tunnelUrl: "https://app.example.dev",
      prefixColor: "blue",
    },
  ],
});
```

## 💻 Development

<details>
<summary>Local development</summary>

- Clone this repository
- Install latest LTS version of [Node.js](https://nodejs.org/en/)
- Enable [Corepack](https://github.com/nodejs/corepack) using `corepack enable`
- Install dependencies using `pnpm install`
- Run interactive tests using `pnpm dev`

</details>

## License

Published under the [MIT](./LICENSE) license.

## Contributors

Published under the [MIT](https://github.com/jasenmichael/stackrun/blob/main/LICENSE) license.
Made by [@jasenmichael](https://github.com/jasenmichael) ❤️

<!-- automd:with-automd -->

---

_🤖 auto updated with [automd](https://automd.unjs.io)_

<!-- /automd -->
