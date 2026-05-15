import { invoke } from "@tauri-apps/api/core";

export type ModelBucket = {
  model: string;
  tokens_total: number;
  input_tok: number;
  output_tok: number;
  cache_w_tok: number;
  cache_r_tok: number;
  cost_usd: number;
  msgs: number;
};
export type ProjectBucket = {
  project: string;
  tokens_total: number;
  cost_usd: number;
  msgs: number;
  sessions: number;
};
export type Summary = {
  total_cost_usd: number;
  msgs: number;
  input_tok: number;
  output_tok: number;
  cache_w_tok: number;
  cache_r_tok: number;
  by_model: ModelBucket[];
  by_project: ProjectBucket[];
};
export type DaySession = {
  session_id: string;
  project: string;
  tokens_total: number;
  cost_usd: number;
  msgs: number;
};
export type DayRow = {
  day: string;
  tokens_total: number;
  cost_usd: number;
  msgs: number;
  sessions: DaySession[];
};
export type SessionRow = {
  session_id: string;
  project: string;
  model: string;
  msgs: number;
  tokens_total: number;
  input_tok: number;
  output_tok: number;
  cache_w_tok: number;
  cache_r_tok: number;
  cost_usd: number;
  start_ts: string;
  end_ts: string;
};
export type TurnTool = { name: string; count: number };
export type TurnRow = {
  ts: string;
  cost_usd: number;
  input_tok: number;
  output_tok: number;
  cache_w_tok: number;
  cache_r_tok: number;
  tools: TurnTool[];
};
export type FileRow = { file_path: string; count: number };
export type SessionDetail = {
  session_id: string;
  project: string;
  model: string;
  msgs: number;
  cost_usd: number;
  input_tok: number;
  output_tok: number;
  cache_w_tok: number;
  cache_r_tok: number;
  turns: TurnRow[];
  top_files: FileRow[];
  tool_counts: Record<string, number>;
};
export type Recommendation = {
  key: string;
  severity: "HIGH" | "MED" | "LOW";
  title: string;
  body: string;
  action: string;
  action_session_id: string | null;
  action_project: string | null;
  evidence: unknown;
  estimated_savings_tokens: number;
  estimated_savings_usd: number;
};
export type HealthSignal = {
  key: string;
  title: string;
  detail: string;
};
export type Block = {
  start: string;
  end: string;
  minutes: number;
  turns: number;
  cost_usd: number;
  top_project: string;
};
export type HourBucket = { hour: string; turns: number };
export type Utilization = {
  turns: number;
  span_min: number;
  active_min: number;
  utilization_pct: number;
  turns_per_active_hour: number;
  cost_per_active_hour: number;
  avg_context: number;
  avg_output: number;
  output_input_ratio: number;
  blocks: Block[];
  hourly: HourBucket[];
};

export type CacheStats = {
  messages: number;
  sessions: number;
  projects: number;
  first_ts: string | null;
  last_ts: string | null;
  db_bytes: number;
};
export type PricingRow = {
  model: string;
  input: number;
  output: number;
  cache_write: number;
  cache_read: number;
};

export const ipc = {
  refreshLogs: () => invoke<number>("refresh_logs"),
  summary: (since?: string, until?: string, project?: string) =>
    invoke<Summary>("summary", { since, until, project }),
  dailyBreakdown: (since: string, until: string) =>
    invoke<DayRow[]>("daily_breakdown", { since, until }),
  sessions: (since?: string, until?: string) =>
    invoke<SessionRow[]>("sessions", { since, until }),
  sessionDetail: (sessionId: string) =>
    invoke<SessionDetail>("session_detail", { sessionId }),
  recommendations: (since: string, until: string) =>
    invoke<Recommendation[]>("recommendations", { since, until }),
  healthSignals: (since: string, until: string) =>
    invoke<HealthSignal[]>("health_signals", { since, until }),
  utilization: (day: string) => invoke<Utilization>("utilization", { day }),
  cacheStats: () => invoke<CacheStats>("cache_stats"),
  clearCache: () => invoke<void>("clear_cache"),
  pricingTable: () => invoke<PricingRow[]>("pricing_table"),
  setPricing: (rows: PricingRow[]) => invoke<number>("set_pricing", { rows }),
};
