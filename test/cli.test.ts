import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { existsSync } from "node:fs";
import { loadConfig } from "c12";
import { stackrun } from "../src/index";

// Store the run function so we can access it in tests
let runFunction: (context: { args: any }) => Promise<void>;

// Mock dependencies
vi.mock("node:fs", () => ({
  existsSync: vi.fn().mockReturnValue(true),
}));

vi.mock("consola", () => ({
  consola: {
    info: vi.fn(),
    error: vi.fn(),
    warn: vi.fn(),
  },
}));

vi.mock("c12", () => ({
  loadConfig: vi.fn(),
}));

vi.mock("../src/index", () => ({
  stackrun: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("citty", () => ({
  defineCommand: (options: any) => {
    // Store the run function when defineCommand is called
    runFunction = options.run;
    return { ...options };
  },
  runMain: vi.fn(),
}));

// Access package.json version for tests
vi.mock("../package.json", () => ({
  version: "0.0.9",
  description: "Test",
  name: "stackrun",
}));

// Mock process.exit
const mockExit = vi.fn();
process.exit = mockExit as any;

// Mock console.log for version output
const mockConsoleLog = vi.fn();
console.log = mockConsoleLog;

describe("CLI", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockExit.mockReset();
    mockConsoleLog.mockReset();

    // Set up default loadConfig mock to return a valid config
    vi.mocked(loadConfig).mockResolvedValue({
      config: { tunnelEnabled: false },
      configFile: "stack.config.ts",
    });

    // Set up default existsSync mock to return true
    vi.mocked(existsSync).mockReturnValue(true);
  });

  afterEach(() => {
    // Clean up environment variables set during tests
    delete process.env.TUNNEL;
  });

  it("should run stackrun with loaded config", async () => {
    // Import CLI file to trigger defineCommand
    await import("../src/cli");

    // Ensure the run function was captured
    expect(runFunction).toBeDefined();

    // Call the run function directly with properly structured args
    await runFunction({
      args: {
        config: "stack.config.ts",
        _: [], // Required array property
      },
    });

    // Check stackrun was called with the right config
    expect(stackrun).toHaveBeenCalledWith(
      expect.objectContaining({ tunnelEnabled: false }),
    );
  });

  it("should handle missing config file", async () => {
    // Override existsSync mock for this test
    vi.mocked(existsSync).mockReturnValue(false);

    // Import CLI file
    await import("../src/cli");

    // Call the run function with properly structured args
    await runFunction({
      args: {
        config: "nonexistent.ts",
        _: [],
      },
    });

    // Check process.exit was called with error code
    expect(mockExit).toHaveBeenCalledWith(1);
  });

  it("should enable tunneling when --tunnel flag is provided", async () => {
    // Import CLI file
    await import("../src/cli");

    // Call with tunnel flag and proper args structure
    await runFunction({
      args: {
        config: "stack.config.ts",
        tunnel: true,
        _: [],
      },
    });

    // Check tunnelEnabled was set to true in the config passed to stackrun
    expect(stackrun).toHaveBeenCalledWith(
      expect.objectContaining({ tunnelEnabled: true }),
    );
  });

  it("should enable tunneling when TUNNEL env var is set", async () => {
    // Set environment variable
    process.env.TUNNEL = "true";

    // Import CLI file
    await import("../src/cli");

    // Call without tunnel flag but with proper args structure
    await runFunction({
      args: {
        config: "stack.config.ts",
        _: [],
      },
    });

    // Check tunnelEnabled was set to true
    expect(stackrun).toHaveBeenCalledWith(
      expect.objectContaining({ tunnelEnabled: true }),
    );
  });

  it("should display version and exit when --version flag is used", async () => {
    // Import CLI file
    await import("../src/cli");

    // Set up process.exit to interrupt execution
    mockExit.mockImplementation(() => {
      throw new Error("Exit called");
    });

    // Call with version flag and proper args structure
    try {
      await runFunction({
        args: {
          version: true,
          _: [],
        },
      });
    } catch {
      // Expected exit to be called
    }

    // Should print version and exit
    expect(mockConsoleLog).toHaveBeenCalledWith("0.0.9");
    expect(mockExit).toHaveBeenCalledWith(0);

    // Note: We can't assert loadConfig wasn't called because in the actual implementation
    // process.exit() immediately exits, so any code after that doesn't run
  });

  it("should exit with error when no config is loaded", async () => {
    // Return undefined for config
    vi.mocked(loadConfig).mockResolvedValue({
      config: undefined as any,
      configFile: "stack.config.ts",
    });

    // Import CLI file
    await import("../src/cli");

    // Call with proper args structure
    await runFunction({
      args: {
        config: "stack.config.ts",
        _: [],
      },
    });

    // Should exit with error
    expect(mockExit).toHaveBeenCalledWith(1);
  });
});
