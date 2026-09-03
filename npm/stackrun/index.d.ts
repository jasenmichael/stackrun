export type EnvValue = string | boolean | number;

export type CommandTunnel = {
  local?: string;
  public?: string;
  env?: Record<string, EnvValue>;
  resource?: string;
  prefix?: string;
  color?: string;
  removeExisting?: boolean;
};

export type Command = {
  run: string;
  name?: string;
  cwd?: string;
  env?: Record<string, EnvValue>;
  color?: string;
  tunnel?: CommandTunnel;
};

export type ProcessOptions = {
  killOthers?: "failure" | string | string[];
  handleInput?: boolean;
  colors?: "auto" | boolean | string | string[];
  prefixLength?: number;
};

export type TunnelDefaults = {
  removeExisting?: boolean;
  prefix?: string;
  color?: string;
  resource?: string;
};

export type StackrunConfig = {
  before?: string[];
  after?: string[];
  process?: ProcessOptions;
  tunnel?: false | true | TunnelDefaults;
  commands?: Array<Command | string>;
};

export type StackrunOptions = {
  tunnel?: boolean;
  dryRun?: boolean;
};

export function defineStackrunConfig<T extends StackrunConfig>(config: T): T;

export function stackrun(
  config?: StackrunConfig,
  options?: StackrunOptions,
): Promise<0>;

export function resolveBinary(): string;
export function platformKey(): string;
