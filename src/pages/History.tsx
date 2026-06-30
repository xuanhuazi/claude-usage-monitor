import { useEffect, useMemo, useState } from "react";
import type { BarSeriesOption, EChartsOption } from "echarts";
import { getHistory, type HistPoint } from "../lib/ipc";
import { Chart, readPalette } from "../components/Chart";
import { Card } from "../components/ui";
import { fmtDate, fmtDateTime, fmtTokens } from "../lib/format";

const RANGES = [
  { label: "7 天", days: 7, bucketMin: 1440, hourly: false },
  { label: "14 天", days: 14, bucketMin: 1440, hourly: false },
  { label: "30 天", days: 30, bucketMin: 1440, hourly: false },
  { label: "近 48 小时", days: 2, bucketMin: 60, hourly: true },
];

function buildOption(data: HistPoint[], hourly: boolean): EChartsOption {
  const p = readPalette();
  const x = data.map((d) => (hourly ? fmtDateTime(d.bucketMs) : fmtDate(d.bucketMs)));
  const families: [string, keyof HistPoint, string][] = [
    ["Opus", "opus", p.accent],
    ["Sonnet", "sonnet", p.warn],
    ["Haiku", "haiku", p.success],
    ["其他", "other", p.muted],
  ];
  const series: BarSeriesOption[] = families.map(([name, key, color]) => ({
    name,
    type: "bar",
    stack: "tok",
    data: data.map((d) => d[key] as number),
    itemStyle: { color, borderRadius: name === "其他" ? [3, 3, 0, 0] : 0 },
    barMaxWidth: 28,
  }));
  return {
    backgroundColor: "transparent",
    textStyle: { fontFamily: "Inter Variable, sans-serif", color: p.muted },
    grid: { left: 8, right: 16, top: 30, bottom: 8, containLabel: true },
    legend: { top: 0, textStyle: { color: p.muted }, icon: "roundRect", itemHeight: 9, itemWidth: 14 },
    tooltip: {
      trigger: "axis",
      backgroundColor: p.surface,
      borderColor: p.border,
      textStyle: { color: p.ink },
      valueFormatter: (v) => fmtTokens(Number(v)),
    },
    xAxis: {
      type: "category",
      data: x,
      axisLine: { lineStyle: { color: p.border } },
      axisLabel: { color: p.muted },
      axisTick: { show: false },
    },
    yAxis: {
      type: "value",
      splitLine: { lineStyle: { color: p.border, type: "dashed" } },
      axisLabel: { color: p.muted, formatter: (v: number) => fmtTokens(v) },
    },
    series,
  };
}

export function History({ theme }: { theme: string }) {
  const [idx, setIdx] = useState(0);
  const [data, setData] = useState<HistPoint[]>([]);
  const range = RANGES[idx];

  useEffect(() => {
    let on = true;
    getHistory(range.days, range.bucketMin).then((d) => {
      if (on) setData(d);
    });
    return () => {
      on = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [idx]);

  const option = useMemo(() => buildOption(data, range.hourly), [data, theme]);
  const total = data.reduce((s, d) => s + d.weighted, 0);

  return (
    <div className="space-y-5">
      <div className="flex items-center gap-2">
        {RANGES.map((r, i) => (
          <button
            key={r.label}
            className={`btn ${i === idx ? "btn-accent" : ""}`}
            onClick={() => setIdx(i)}
          >
            {r.label}
          </button>
        ))}
        <div className="ml-auto text-sm text-muted">
          区间合计 (计权)：<span className="text-ink font-medium">{fmtTokens(total)}</span>
        </div>
      </div>

      <Card>
        <h2 className="text-lg mb-2">用量趋势 · 按模型堆叠</h2>
        {data.length === 0 ? (
          <div className="text-muted py-12 text-center">该区间暂无数据</div>
        ) : (
          <Chart option={option} height={380} />
        )}
      </Card>
    </div>
  );
}
