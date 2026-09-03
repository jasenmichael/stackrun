---
id: api
sidebar_position: 7
title: Programmatic API
---

The npm package exports `stackrun` and `defineStackrunConfig`. Both spawn the native binary. They do not run the stack in Node.

```ts
import { stackrun, defineStackrunConfig } from "stackrun";

export default defineStackrunConfig({
  commands: [
    { name: "api", run: "npm run dev", cwd: "./api" },
    { name: "web", run: "npm run dev", cwd: "./web" },
  ],
});

await stackrun();
await stackrun({
  commands: [
    { name: "api", run: "npm run dev", cwd: "./api" },
    { name: "web", run: "npm run dev", cwd: "./web" },
  ],
});
```
