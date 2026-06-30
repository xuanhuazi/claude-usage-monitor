import type { ReactNode } from "react";
import type { Status, WindowStat } from "../lib/ipc";
import { Gauge } from "../components/Gauge";
import { Card, StatCard, ProgressBar } from "../components/ui";
import { fmtTokens, fmtPct, fmtDuration, fmtResetTime, statusColor, fmtUsd } from "../lib/format";

function Row({ k, v }: { k: string; v: ReactNode }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-muted">{k}</span>
      <span className="text-ink font-medium tabular-nums">{v}</span>
    </div>
  );
}

function WindowCard({
  title,
  w,
  showReset,
  official,
}: {
  title: string;
  w: WindowStat;
  showReset?: boolean;
  official?: boolean;
}) {
  return (
    <Card>
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-lg">{title}</h2>
        {official ? (
          <span className="tag">官方利用率</span>
        ) : (
          w.autoLimit && <span className="tag">自动校准上限</span>
        )}
      </div>
      <div className="flex items-center gap-6">
        <Gauge pct={w.pct} label={fmtPct(w.pct)} sublabel={`${fmtTokens(w.used)} / ${fmtTokens(w.limit)}`} />
        <div className="flex-1 space-y-2.5 text-sm">
          <Row k="已用 (计权)" v={fmtTokens(w.used)} />
          <Row
            k="真实消耗 (含缓存读)"
            v={fmtTokens(w.raw.input + w.raw.output + w.raw.cacheCreation + w.raw.cacheRead)}
          />
          <Row k="等效成本" v={fmtUsd(w.costUsd)} />
          <Row k={official ? "上限 (官方推算)" : "上限 (估算)"} v={fmtTokens(w.limit)} />
          {!official && <Row k="历史峰值" v={fmtTokens(w.observedMax)} />}
          {showReset && <Row k="重置时间" v={fmtResetTime(w.resetsAtMs)} />}
          {showReset && <Row k="剩余时间" v={fmtDuration(w.remainingSeconds)} />}
        </div>
      </div>
    </Card>
  );
}

function TokenSplit({ w }: { w: WindowStat }) {
  const { input, output, cacheCreation, cacheRead } = w.raw;
  const total = input + output + cacheCreation + cacheRead;
  const hitRate = input + cacheCreation + cacheRead > 0
    ? cacheRead / (input + cacheCreation + cacheRead)
    : 0;
  const items: [string, number][] = [
    ["输入", input],
    ["输出", output],
    ["缓存写入", cacheCreation],
    ["缓存读取", cacheRead],
  ];
  return (
    <Card>
      <div className="flex items-baseline justify-between mb-3">
        <h2 className="text-lg">原始 token 明细 · 近 7 天</h2>
        <div className="text-sm text-muted">
          真实消耗合计 <span className="text-ink font-medium tabular-nums">{fmtTokens(total)}</span>
          <span className="ml-3">缓存命中率 <span className="text-ink font-medium">{(hitRate * 100).toFixed(1)}%</span></span>
        </div>
      </div>
      <div className="grid grid-cols-4 gap-4">
        {items.map(([label, n]) => (
          <div key={label}>
            <div className="text-muted text-[0.7rem] uppercase tracking-wider">{label}</div>
            <div className="text-xl font-serif mt-1 tabular-nums">{fmtTokens(n)}</div>
          </div>
        ))}
      </div>
    </Card>
  );
}

function OpusWeekly({ official }: { official: Status["official"] }) {
  if (!official.available || official.utilOpus7d == null) return null;
  const pct = official.utilOpus7d;
  return (
    <Card>
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-lg">
          Opus 周限 <span className="tag ml-2">官方</span>
        </h2>
        <span className="text-sm text-muted">重置 {fmtResetTime(official.resetOpus7dMs)}</span>
      </div>
      <div className="flex items-center gap-4">
        <div
          className="text-2xl font-serif tabular-nums w-20"
          style={{ color: `var(--c-${statusColor(pct)})` }}
        >
          {fmtPct(pct)}
        </div>
        <div className="flex-1">
          <ProgressBar pct={pct} />
        </div>
      </div>
      <p className="text-muted text-xs mt-2">
        Max 套餐对 Opus 有单独的周配额，重度使用 Opus 时常常先于「近 7 天」总配额触顶。
      </p>
    </Card>
  );
}

export function Dashboard({ status }: { status: Status | null }) {
  if (!status) {
    return <div className="text-muted">正在读取本地用量数据…</div>;
  }

  return (
    <div className="space-y-5">
      <div className="grid grid-cols-2 gap-5">
        <WindowCard title="5 小时滚动窗口" w={status.window5h} showReset official={status.official.available} />
        <WindowCard title="近 7 天" w={status.window7d} showReset={status.official.available} official={status.official.available} />
      </div>

      <OpusWeekly official={status.official} />

      <div className="grid grid-cols-4 gap-4">
        <StatCard
          label="消耗速率"
          value={`${fmtTokens(status.burn.tokensPerMin)}/分`}
          sub="最近 60 分钟"
        />
        <StatCard
          label="预计触顶 (5h)"
          value={fmtDuration(status.burn.secondsToLimit)}
          sub="按当前速率"
        />
        <StatCard label="累计消息" value={fmtTokens(status.totalEvents)} sub="已索引" />
        <StatCard
          label="缓存计权"
          value={`写×${status.weights.cacheCreation} · 读×${status.weights.cacheRead}`}
          sub="可在设置中调整"
        />
      </div>

      <TokenSplit w={status.window7d} />

      {status.official.available ? (
        <p className="text-muted text-xs leading-relaxed">
          ✅ 利用率来自 Claude 服务端<strong>官方只读接口</strong>（不消耗额度）；token 明细仍由本地日志统计。{status.official.note}
        </p>
      ) : (
        <p className="text-muted text-xs leading-relaxed">
          ⚠️ 当前为基于本地 JSONL 日志的<strong>估算值</strong>；额度上限按套餐档位估计并随历史用量自动校准，可能与真实限额有偏差。可在「设置」启用官方利用率源获得精确百分比。
          {status.official.note ? ` · ${status.official.note}` : ""}
        </p>
      )}
      <p className="text-muted text-xs leading-relaxed">
        ℹ️「已用(计权)」默认只计 输入 + 输出 + 缓存写入；<strong>缓存读取</strong>（cache_read，通常占 90%+ 且计费极低）默认<strong>不计入额度</strong>，因此比 cc-switch 等显示的「真实消耗(含缓存)」小很多——底层数据一致，只是口径不同。如需让缓存读取计入额度，可在「设置 → 缓存读取计权」调高。
      </p>
    </div>
  );
}
