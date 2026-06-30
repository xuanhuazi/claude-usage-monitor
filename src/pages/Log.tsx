import { useEffect, useState } from "react";
import { getLog, type LogEntry } from "../lib/ipc";
import { fmtTokens, fmtUsd, fmtDateTime } from "../lib/format";

function modelLabel(m: string): string {
  const l = m.toLowerCase();
  if (l.includes("opus")) return "Opus";
  if (l.includes("sonnet")) return "Sonnet";
  if (l.includes("haiku")) return "Haiku";
  return m.replace(/^claude-/, "");
}

export function Log() {
  const [limit, setLimit] = useState(200);
  const [rows, setRows] = useState<LogEntry[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let on = true;
    setLoading(true);
    getLog(limit).then((r) => {
      if (on) {
        setRows(r);
        setLoading(false);
      }
    });
    return () => {
      on = false;
    };
  }, [limit]);

  const th = "text-left font-medium px-4 py-2.5";
  const thR = "text-right font-medium px-4 py-2.5";
  const td = "px-4 py-2 tabular-nums text-right whitespace-nowrap";

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3">
        <h2 className="text-lg">请求日志</h2>
        <span className="text-sm text-muted">最近 {rows.length} 条 · 按时间倒序</span>
        <button className="btn ml-auto" onClick={() => setLimit((l) => l + 200)} disabled={loading}>
          显示更多
        </button>
      </div>
      <div className="card overflow-hidden">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-muted text-xs uppercase tracking-wider border-b border-border">
              <th className={th}>时间</th>
              <th className={th}>模型</th>
              <th className={th}>项目</th>
              <th className={thR}>输入</th>
              <th className={thR}>输出</th>
              <th className={thR}>缓存写</th>
              <th className={thR}>缓存读</th>
              <th className={thR}>计权</th>
              <th className={thR}>等效成本</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={i} className="border-b border-border last:border-0 hover:bg-elevated">
                <td className="px-4 py-2 tabular-nums whitespace-nowrap text-muted">{fmtDateTime(r.tsMs)}</td>
                <td className="px-4 py-2">{modelLabel(r.model)}</td>
                <td className="px-4 py-2 truncate max-w-[180px]" title={r.project}>
                  {r.project}
                </td>
                <td className={td}>{fmtTokens(r.input)}</td>
                <td className={td}>{fmtTokens(r.output)}</td>
                <td className={td}>{fmtTokens(r.cacheCreation)}</td>
                <td className={td}>{fmtTokens(r.cacheRead)}</td>
                <td className={`${td} text-ink`}>{fmtTokens(r.weighted)}</td>
                <td className={td}>{fmtUsd(r.costUsd)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {rows.length === 0 && <div className="text-muted text-center py-10">暂无数据</div>}
      </div>
    </div>
  );
}
