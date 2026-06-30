import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---- Types mirroring the Rust IPC structs (camelCase) ----

export interface RawTokens {
  input: number;
  output: number;
  cacheCreation: number;
  cacheRead: number;
}

export interface WindowStat {
  used: number;
  limit: number;
  pct: number;
  raw: RawTokens;
  resetsAtMs: number | null;
  remainingSeconds: number | null;
  observedMax: number;
  autoLimit: boolean;
  costUsd: number;
}

export interface BurnRate {
  tokensPerMin: number;
  secondsToLimit: number | null;
}

export interface Weights {
  cacheCreation: number;
  cacheRead: number;
}

export interface OfficialStatus {
  available: boolean;
  source: string;
  util5h: number | null;
  util7d: number | null;
  reset5hMs: number | null;
  reset7dMs: number | null;
  utilOpus7d: number | null;
  resetOpus7dMs: number | null;
  note: string;
}

export interface Status {
  generatedAtMs: number;
  tier: string;
  planLabel: string;
  subscriptionType: string | null;
  rateLimitTier: string | null;
  window5h: WindowStat;
  window7d: WindowStat;
  burn: BurnRate;
  weights: Weights;
  totalEvents: number;
  firstEventMs: number | null;
  official: OfficialStatus;
}

export interface BreakdownItem {
  key: string;
  label: string;
  weighted: number;
  raw: RawTokens;
  count: number;
  costUsd: number;
}

export interface Breakdown {
  byModel: BreakdownItem[];
  byProject: BreakdownItem[];
}

export interface LogEntry {
  tsMs: number;
  model: string;
  project: string;
  input: number;
  output: number;
  cacheCreation: number;
  cacheRead: number;
  weighted: number;
  costUsd: number;
}

export interface HistPoint {
  bucketMs: number;
  weighted: number;
  opus: number;
  sonnet: number;
  haiku: number;
  other: number;
}

export interface Settings {
  tierOverride: string | null;
  limit5hOverride: number | null;
  limit7dOverride: number | null;
  autoCalibrate: boolean;
  weightCacheCreation: number;
  weightCacheRead: number;
  alertThresholds: number[];
  autostart: boolean;
  officialEnabled: boolean;
  theme: string;
  pollIntervalSecs: number;
}

// ---- Command wrappers ----

export const getStatus = () => invoke<Status>("get_status");
export const refreshNow = () => invoke<Status>("refresh_now");
export const refreshOfficial = () => invoke<Status>("refresh_official");
export const getSettings = () => invoke<Settings>("get_settings");
export const setSettings = (settings: Settings) =>
  invoke<Status>("set_settings", { settings });
export const getHistory = (days: number, bucketMinutes: number) =>
  invoke<HistPoint[]>("get_history", { days, bucketMinutes });
export const getBreakdown = (window: string) =>
  invoke<Breakdown>("get_breakdown", { window });
export const getLog = (limit: number) => invoke<LogEntry[]>("get_log", { limit });

export const onStatus = (cb: (s: Status) => void): Promise<UnlistenFn> =>
  listen<Status>("status-updated", (e) => cb(e.payload));
