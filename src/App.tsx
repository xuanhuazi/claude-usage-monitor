import { useCallback, useEffect, useState } from "react";
import {
  getStatus,
  getSettings,
  setSettings as ipcSetSettings,
  refreshNow,
  onStatus,
  type Status,
  type Settings,
} from "./lib/ipc";
import { Dashboard } from "./pages/Dashboard";
import { History } from "./pages/History";
import { Breakdown } from "./pages/Breakdown";
import { Log } from "./pages/Log";
import { Settings as SettingsPage } from "./pages/Settings";
import { fmtDateTime } from "./lib/format";

type Page = "dashboard" | "history" | "breakdown" | "log" | "settings";

const NAV: { k: Page; label: string; icon: string }[] = [
  { k: "dashboard", label: "仪表盘", icon: "◎" },
  { k: "history", label: "趋势", icon: "∿" },
  { k: "breakdown", label: "细分", icon: "▦" },
  { k: "log", label: "日志", icon: "≣" },
  { k: "settings", label: "设置", icon: "⚙" },
];

function resolveTheme(theme: string): "light" | "dark" {
  if (theme === "light" || theme === "dark") return theme;
  return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export default function App() {
  const [page, setPage] = useState<Page>("dashboard");
  const [status, setStatus] = useState<Status | null>(null);
  const [settings, setSettingsState] = useState<Settings | null>(null);
  const [resolved, setResolved] = useState<"light" | "dark">("light");

  useEffect(() => {
    getStatus().then(setStatus);
    getSettings().then(setSettingsState);
  }, []);

  useEffect(() => {
    const un = onStatus(setStatus);
    return () => {
      un.then((f) => f());
    };
  }, []);

  const themePref = settings?.theme ?? "system";
  useEffect(() => {
    const apply = () => {
      const r = resolveTheme(themePref);
      document.documentElement.setAttribute("data-theme", r);
      setResolved(r);
    };
    apply();
    if (themePref === "system") {
      const mq = matchMedia("(prefers-color-scheme: dark)");
      mq.addEventListener("change", apply);
      return () => mq.removeEventListener("change", apply);
    }
  }, [themePref]);

  const saveSettings = useCallback(async (s: Settings) => {
    const st = await ipcSetSettings(s);
    setSettingsState(s);
    setStatus(st);
  }, []);

  const refresh = async () => setStatus(await refreshNow());

  return (
    <div className="flex h-screen overflow-hidden">
      <aside className="w-56 shrink-0 bg-surface border-r border-border flex flex-col">
        <div className="px-5 py-5">
          <div className="flex items-center gap-2">
            <span className="inline-block size-3 rounded-full" style={{ background: "var(--c-accent)" }} />
            <span className="font-serif text-lg">Claude 额度</span>
          </div>
          <div className="text-xs text-muted mt-1">用量实时监控</div>
        </div>
        <nav className="px-3 space-y-1">
          {NAV.map((n) => (
            <button
              key={n.k}
              onClick={() => setPage(n.k)}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-md text-sm transition ${
                page === n.k
                  ? "bg-accent-soft text-accent-hover font-medium"
                  : "text-muted hover:bg-elevated"
              }`}
            >
              <span className="w-4 text-center">{n.icon}</span>
              {n.label}
            </button>
          ))}
        </nav>
        <div className="mt-auto px-5 py-4 text-xs text-muted leading-relaxed">
          {status?.rateLimitTier && <div>tier: {status.rateLimitTier}</div>}
          {status && <div>{status.totalEvents.toLocaleString()} 条已索引</div>}
        </div>
      </aside>

      <main className="flex-1 flex flex-col overflow-hidden">
        <header className="h-14 shrink-0 border-b border-border flex items-center px-6 gap-3">
          <h1 className="text-base">{NAV.find((n) => n.k === page)?.label}</h1>
          {status && <span className="tag">{status.planLabel}</span>}
          <div className="ml-auto flex items-center gap-3 text-xs text-muted">
            {status && <span>更新于 {fmtDateTime(status.generatedAtMs)}</span>}
            <button className="btn" onClick={refresh}>
              ↻ 刷新
            </button>
          </div>
        </header>
        <div className="flex-1 overflow-auto p-6">
          {page === "dashboard" && <Dashboard status={status} />}
          {page === "history" && <History theme={resolved} />}
          {page === "breakdown" && <Breakdown />}
          {page === "log" && <Log />}
          {page === "settings" && <SettingsPage settings={settings} onSave={saveSettings} />}
        </div>
      </main>
    </div>
  );
}
