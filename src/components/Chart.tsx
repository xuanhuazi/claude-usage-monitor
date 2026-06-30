import { useEffect, useRef } from "react";
import * as echarts from "echarts";

interface ChartProps {
  option: echarts.EChartsOption;
  height?: number;
}

/** Minimal ECharts wrapper (avoids echarts-for-react's React-version peer dep). */
export function Chart({ option, height = 320 }: ChartProps) {
  const ref = useRef<HTMLDivElement>(null);
  const inst = useRef<echarts.ECharts | null>(null);

  useEffect(() => {
    if (!ref.current) return;
    inst.current = echarts.init(ref.current);
    const onResize = () => inst.current?.resize();
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("resize", onResize);
      inst.current?.dispose();
      inst.current = null;
    };
  }, []);

  useEffect(() => {
    inst.current?.setOption(option, true);
  }, [option]);

  return <div ref={ref} style={{ width: "100%", height }} />;
}

/** Read the current theme palette from CSS variables (for canvas charts). */
export function readPalette() {
  const s = getComputedStyle(document.documentElement);
  const v = (n: string) => s.getPropertyValue(n).trim();
  return {
    accent: v("--c-accent"),
    accentHover: v("--c-accent-hover"),
    ink: v("--c-ink"),
    muted: v("--c-muted"),
    border: v("--c-border"),
    surface: v("--c-surface"),
    success: v("--c-success"),
    warn: v("--c-warn"),
    danger: v("--c-danger"),
  };
}
