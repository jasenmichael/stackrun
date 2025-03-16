import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { defineStackrunConfig, stackrun } from "../src";
import concurrently from "concurrently";
import { execSync } from "node:child_process";

// Mock dependencies
vi.mock("concurrently", () => ({
  default: vi.fn().mockReturnValue({ result: Promise.resolve() }),
}));

vi.mock("node:child_process", () => ({
  execSync: vi.fn(),
}));

vi.mock("consola", () => ({
  default: {
    info: vi.fn(),
    error: vi.fn(),
    warn: vi.fn(),
  },
}));

describe("stackrun", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    process.env.CLOUDFLARE_TOKEN = "mock-token";
  });

  afterEach(() => {
    delete process.env.CLOUDFLARE_TOKEN;
    delete process.env.CLOUDFLARE_TUNNEL_NAME;
  });

  it("should define config correctly", () => {
    const config = { tunnelEnabled: true };
    const result = defineStackrunConfig(config);
    expect(result).toEqual(config);
  });

  it("should run commands with concurrently", async () => {
    const config = {
      commands: [{ command: "echo test", name: "test" }],
    };

    await stackrun(config);

    expect(concurrently).toHaveBeenCalledWith(
      expect.arrayContaining([
        expect.objectContaining({ command: "echo test" }),
      ]),
      expect.objectContaining({ killOthers: "failure", handleInput: true }),
    );
  });

  it("should run beforeCommands and afterCommands", async () => {
    const config = {
      beforeCommands: ["before command"],
      afterCommands: ["after command"],
      commands: [{ command: "main command", name: "main" }],
    };

    await stackrun(config);

    expect(execSync).toHaveBeenCalledTimes(2);
    expect(execSync).toHaveBeenNthCalledWith(
      1,
      "before command",
      expect.objectContaining({ stdio: "inherit" }),
    );
    expect(execSync).toHaveBeenNthCalledWith(
      2,
      "after command",
      expect.objectContaining({ stdio: "inherit" }),
    );
  });

  it("should handle tunnel configuration", async () => {
    process.env.CLOUDFLARE_TUNNEL_NAME = "test-tunnel";

    const config = {
      tunnelEnabled: true,
      commands: [
        {
          command: "web server",
          name: "web",
          url: "http://localhost:3000",
          tunnelUrl: "https://example.com",
        },
      ],
    };

    await stackrun(config);

    // Check if tunnel command was added
    expect(concurrently).toHaveBeenCalledWith(
      expect.arrayContaining([expect.objectContaining({ name: "TUNN" })]),
      expect.anything(),
    );
  });

  it("should handle missing cfToken for tunneling", async () => {
    delete process.env.CLOUDFLARE_TOKEN;

    const config = {
      tunnelEnabled: true,
      commands: [{ command: "test", name: "test" }],
    };

    await stackrun(config);

    // Should not call concurrently if token is missing
    expect(concurrently).not.toHaveBeenCalled();
  });
});
