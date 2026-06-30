import { useEffect, useState, type ReactNode } from "react";
import { refreshOfficial, type Settings as S } from "../lib/ipc";
import { Card } from "../components/ui";

const inputCls =
  "rounded-md border border-border bg-elevated px-3 py-2 text-sm text-ink outline-none focus:border-accent";

function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="flex items-start justify-between gap-6 py-3 border-b border-border last:border-0">
      <div className="max-w-[60%]">
        <div className="text-sm text-ink font-medium">{label}</div>
        {hint && <div className="text-xs text-muted mt-0.5 leading-relaxed">{hint}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </label>
  );
}

const numOrNull = (v: string): number | null => (v.trim() === "" ? null : Number(v));

export function Settings({
  settings,
  onSave,
}: {
  settings: S | null;
  onSave: (s: S) => Promise<void>;
}) {
  const [draft, setDraft] = useState<S | null>(settings);
  const [thresholdsText, setThresholdsText] = useState("");
  const [saved, setSaved] = useState(false);
  const [refreshed, setRefreshed] = useState(false);

  useEffect(() => {
    setDraft(settings);
    if (settings) setThresholdsText(settings.alertThresholds.join(", "));
  }, [settings]);

  if (!draft) return <div className="text-muted">加载设置中…</div>;

  const set = <K extends keyof S>(k: K, v: S[K]) => {
    setDraft({ ...draft, [k]: v } as S);
    setSaved(false);
  };

  const save = async () => {
    const thresholds = thresholdsText
      .split(/[,，\s]+/)
      .map((s) => parseInt(s, 10))
      .filter((n) => !isNaN(n) && n > 0 && n <= 100)
      .sort((a, b) => a - b);
    await onSave({ ...draft, alertThresholds: thresholds.length ? thresholds : [75, 90, 100] });
    setSaved(true);
  };

  return (
    <div className="space-y-5 max-w-3xl">
      <Card>
        <h2 className="text-lg mb-1">外观与启动</h2>
        <Field label="主题" hint="跟随系统、或固定浅色 / 深色">
          <select className={inputCls} value={draft.theme} onChange={(e) => set("theme", e.target.value)}>
            <option value="system">跟随系统</option>
            <option value="light">浅色</option>
            <option value="dark">深色</option>
          </select>
        </Field>
        <Field label="开机自启" hint="登录 Windows 时自动启动并最小化到托盘">
          <input type="checkbox" className="size-4 accent-[var(--c-accent)]" checked={draft.autostart} onChange={(e) => set("autostart", e.target.checked)} />
        </Field>
        <Field label="刷新间隔（秒）" hint="后台扫描日志与刷新仪表盘的频率">
          <input type="number" min={2} className={`${inputCls} w-24`} value={draft.pollIntervalSecs} onChange={(e) => set("pollIntervalSecs", Math.max(2, Number(e.target.value)))} />
        </Field>
      </Card>

      <Card>
        <h2 className="text-lg mb-1">额度与计权</h2>
        <Field label="套餐档位" hint="默认自动识别（来自 .credentials.json），可手动覆盖">
          <select
            className={inputCls}
            value={draft.tierOverride ?? ""}
            onChange={(e) => set("tierOverride", e.target.value === "" ? null : e.target.value)}
          >
            <option value="">自动识别</option>
            <option value="pro">Pro</option>
            <option value="max5x">Max 5x</option>
            <option value="max20x">Max 20x</option>
            <option value="free">Free</option>
          </select>
        </Field>
        <Field label="自动校准上限" hint="用历史最高用量作为上限的下界，让百分比更贴近真实">
          <input type="checkbox" className="size-4 accent-[var(--c-accent)]" checked={draft.autoCalibrate} onChange={(e) => set("autoCalibrate", e.target.checked)} />
        </Field>
        <Field label="5h 上限覆盖" hint="留空 = 自动（按档位估计 + 校准）。单位：计权 token">
          <input type="number" className={`${inputCls} w-32`} placeholder="自动" value={draft.limit5hOverride ?? ""} onChange={(e) => set("limit5hOverride", numOrNull(e.target.value))} />
        </Field>
        <Field label="7d 上限覆盖" hint="留空 = 自动">
          <input type="number" className={`${inputCls} w-32`} placeholder="自动" value={draft.limit7dOverride ?? ""} onChange={(e) => set("limit7dOverride", numOrNull(e.target.value))} />
        </Field>
        <Field label="缓存写入计权" hint="cache_creation token 计入额度的系数（默认 1.0）">
          <input type="number" step={0.05} className={`${inputCls} w-24`} value={draft.weightCacheCreation} onChange={(e) => set("weightCacheCreation", Number(e.target.value))} />
        </Field>
        <Field label="缓存读取计权" hint="cache_read token 计入额度的系数（默认 0，通常折扣很大）">
          <input type="number" step={0.05} className={`${inputCls} w-24`} value={draft.weightCacheRead} onChange={(e) => set("weightCacheRead", Number(e.target.value))} />
        </Field>
      </Card>

      <Card>
        <h2 className="text-lg mb-1">告警与数据源</h2>
        <Field label="告警阈值（%）" hint="逗号分隔，越线时弹桌面通知，如 75, 90, 100">
          <input className={`${inputCls} w-40`} value={thresholdsText} onChange={(e) => setThresholdsText(e.target.value)} />
        </Field>
        <Field label="启用官方利用率源" hint="用本地 OAuth token 读取官方只读用量接口（不消耗额度，约每 3 分钟刷新）；启用后仪表盘显示官方 5h/7d 百分比与重置时间，失败自动回退本地估算">
          <input type="checkbox" className="size-4 accent-[var(--c-accent)]" checked={draft.officialEnabled} onChange={(e) => set("officialEnabled", e.target.checked)} />
        </Field>
        <Field label="立即刷新官方利用率" hint="跳过冷却，立刻请求一次官方接口（需先启用并保存）">
          <button
            className="btn"
            onClick={async () => {
              await refreshOfficial();
              setRefreshed(true);
            }}
          >
            刷新官方
          </button>
        </Field>
        {refreshed && <div className="text-xs text-success pt-2">已请求官方刷新 ✓</div>}
      </Card>

      <div className="flex items-center gap-3">
        <button className="btn btn-accent" onClick={save}>
          保存设置
        </button>
        {saved && <span className="text-sm text-success">已保存 ✓</span>}
      </div>
    </div>
  );
}
