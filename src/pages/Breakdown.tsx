import { useEffect, useState } from "react";
import { getBreakdown, type Breakdown as BD, type BreakdownItem } from "../lib/ipc";
import { Card } from "../components/ui";
import { fmtTokens, fmtUsd } from "../lib/format";

const WINDOWS = [
  { k: "5h", label: "5 小时" },
  { k: "7d", label: "7 天" },
  { k: "30d", label: "30 天" },
  { k: "all", label: "全部" },
];

function BarList({ items }: { items: BreakdownItem[] }) {
  if (!items.length) return <div className="text-muted py-10 text-center">暂无数据</div>;
  const max = Math.max(1, ...items.map((i) => i.weighted));
  return (
    <div className="space-y-3.5">
      {items.slice(0, 12).map((it) => (
        <div key={it.key}>
          <div className="flex justify-between text-sm mb-1.5">
            <span className="text-ink truncate max-w-[55%]" title={it.key}>
              {it.label}
            </span>
            <span className="text-muted tabular-nums">
              {fmtTokens(it.weighted)} · {fmtUsd(it.costUsd)} · {it.count} 条
            </span>
          </div>
          <div style={{ height: 8, borderRadius: 999, background: "var(--c-border)" }}>
            <div
              style={{
                height: "100%",
                width: `${(it.weighted / max) * 100}%`,
                background: "var(--c-accent)",
                borderRadius: 999,
                transition: "width .5s ease",
              }}
            />
          </div>
        </div>
      ))}
    </div>
  );
}

export function Breakdown() {
  const [win, setWin] = useState("7d");
  const [bd, setBd] = useState<BD | null>(null);

  useEffect(() => {
    let on = true;
    getBreakdown(win).then((d) => {
      if (on) setBd(d);
    });
    return () => {
      on = false;
    };
  }, [win]);

  return (
    <div className="space-y-5">
      <div className="flex gap-2">
        {WINDOWS.map((w) => (
          <button
            key={w.k}
            className={`btn ${win === w.k ? "btn-accent" : ""}`}
            onClick={() => setWin(w.k)}
          >
            {w.label}
          </button>
        ))}
      </div>
      <div className="grid grid-cols-2 gap-5">
        <Card>
          <h2 className="text-lg mb-4">按模型</h2>
          <BarList items={bd?.byModel ?? []} />
        </Card>
        <Card>
          <h2 className="text-lg mb-4">按项目</h2>
          <BarList items={bd?.byProject ?? []} />
        </Card>
      </div>
    </div>
  );
}
