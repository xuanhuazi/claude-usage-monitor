import { statusColor } from "../lib/format";

interface GaugeProps {
  pct: number;
  size?: number;
  stroke?: number;
  label?: string;
  sublabel?: string;
}

/** Radial utilization ring. Fills clockwise; color shifts coral→amber→red. */
export function Gauge({ pct, size = 184, stroke = 15, label, sublabel }: GaugeProps) {
  const clamped = Math.max(0, Math.min(1, pct));
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  const dash = c * clamped;
  const color = `var(--c-${statusColor(pct)})`;
  const cxy = size / 2;

  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
      <circle cx={cxy} cy={cxy} r={r} fill="none" stroke="var(--c-border)" strokeWidth={stroke} />
      <circle
        cx={cxy}
        cy={cxy}
        r={r}
        fill="none"
        stroke={color}
        strokeWidth={stroke}
        strokeDasharray={`${dash} ${c - dash}`}
        strokeLinecap="round"
        transform={`rotate(-90 ${cxy} ${cxy})`}
        style={{ transition: "stroke-dasharray .6s ease, stroke .3s ease" }}
      />
      <text
        x="50%"
        y="47%"
        textAnchor="middle"
        dominantBaseline="middle"
        style={{ fontFamily: "var(--font-serif)", fontSize: size * 0.23, fill: "var(--c-ink)", fontWeight: 560 }}
      >
        {label}
      </text>
      {sublabel && (
        <text
          x="50%"
          y="63%"
          textAnchor="middle"
          style={{ fontSize: size * 0.072, fill: "var(--c-muted)", fontFamily: "var(--font-sans)" }}
        >
          {sublabel}
        </text>
      )}
    </svg>
  );
}
