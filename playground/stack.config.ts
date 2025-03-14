import { defineStackrunConfig } from "../src/index.js";
import "dotenv/config";

// const DEV_CONTAINERS = "db mailhog";

export default defineStackrunConfig({
  concurrentlyOptions: {
    killOthers: "failure",
    handleInput: true,
  },
  tunnelEnabled: true,
  cfTunnelConfig: {
    cfToken: process.env.CF_TOKEN,
    tunnelName: process.env.CLOUDFLARE_TUNNEL_NAME || "stackrun",
    removeExistingTunnel: true,
    removeExistingDns: true,
  },
  // beforeCommands: [
  //   `docker compose -f docker-compose.dev.yml up -d ${DEV_CONTAINERS}`,
  // ],
  // afterCommands: [
  //   `docker compose -f docker-compose.dev.yml down ${DEV_CONTAINERS}`,
  // ],
  // concurrently commands: an array of either strings (containing the commands to run)
  //   or objects with the shape { command, name, prefixColor, env, cwd, ipc }.
  // https://www.npmjs.com/package/concurrently#api
  commands: [
    {
      name: "nuxt",
      command: "npm run dev",
      cwd: "./nuxt",
      env: {},
      url: "http://localhost:3000",
      tunnelEnv: {
        NUXT_STRAPI_URL: process.env.NUXT_STRAPI_URL,
        NUXT_PUBLIC_STRAPI_URL: process.env.NUXT_PUBLIC_STRAPI_URL,
      },
      tunnelUrl: process.env.NUXT_PUBLIC_STRAPI_URL,
      prefixColor: "blue",
    },
    {
      name: "strapi",
      command: "npm run develop",
      cwd: "./strapi",
      env: {},
      url: "http://localhost:1337",
      tunnelEnv: {
        SERVER_URL: "https://api-dev.jasenmichael.com",
      },
      tunnelUrl: "https://api-dev.jasenmichael.com",
      prefixColor: "blue",
    },
  ],
});
