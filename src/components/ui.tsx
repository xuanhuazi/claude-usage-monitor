import type { ReactNode } from "react";
import { statusColor } from "../lib/format";

export function Card({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <div className={`card p-5 ${className}`}>{children}</div>;
}

export function ProgressBar({ pct, height = 10 }: { pct: number; height?: number }) {
  const clamped = Math.min(1, Math.max(0, pct));
  const color = `var(--c-${statusColor(pct)})`;
  return (
    <div
      style={{ height, borderRadius: 999, background: "var(--c-border)", overflow: "hidden" }}
    >
      <div
        style={{
          height: "100%",
          width: `${clamped * 100}%`,
          background: color,
          borderRadius: 999,
          transition: "width .6s ease, background .3s ease",
        }}
      />
    </div>
  );
}

export function StatCard({
  label,
  value,
  sub,
}: {
  label: string;
  value: ReactNode;
  sub?: ReactNode;
}) {
  return (
    <Card>
      <div className="text-muted text-[0.7rem] font-medium uppercase tracking-wider">{label}</div>
      <div className="mt-1.5 text-2xl font-serif text-ink">{value}</div>
      {sub != null && <div className="text-muted text-xs mt-1">{sub}</div>}
    </Card>
  );
}

export function SectionTitle({ children }: { children: ReactNode }) {
  return <h2 className="text-lg text-ink mb-3">{children}</h2>;
}
