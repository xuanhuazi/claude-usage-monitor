//! SQLite persistence (via bundled rusqlite). Holds deduped usage events,
//! per-file read offsets, and app settings. All heavy aggregation runs here.

use crate::pricing;
use crate::types::{Breakdown, BreakdownItem, Event, HistPoint, RawSums, Settings};
use anyhow::Result;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

const FAM_CASE: &str = "CASE \
    WHEN model LIKE '%opus%' THEN 'opus' \
    WHEN model LIKE '%sonnet%' THEN 'sonnet' \
    WHEN model LIKE '%haiku%' THEN 'haiku' \
    ELSE 'other' END";

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS usage_events (
    uuid            TEXT PRIMARY KEY,
    ts_ms           INTEGER NOT NULL,
    model           TEXT NOT NULL,
    project         TEXT NOT NULL,
    session_id      TEXT NOT NULL,
    input_tokens    INTEGER NOT NULL DEFAULT 0,
    output_tokens   INTEGER NOT NULL DEFAULT 0,
    cache_creation  INTEGER NOT NULL DEFAULT 0,
    cache_read      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_usage_ts ON usage_events(ts_ms);
CREATE INDEX IF NOT EXISTS idx_usage_model ON usage_events(model);
CREATE INDEX IF NOT EXISTS idx_usage_project ON usage_events(project);

CREATE TABLE IF NOT EXISTS file_offsets (
    path     TEXT PRIMARY KEY,
    offset   INTEGER NOT NULL,
    size     INTEGER NOT NULL,
    mtime_ms INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Store {
            conn: Mutex::new(conn),
        })
    }

    // ---- file offsets ----

    pub fn get_offset(&self, path: &str) -> (i64, i64) {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT offset, size FROM file_offsets WHERE path = ?1",
            params![path],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )
        .unwrap_or((0, 0))
    }

    pub fn set_offset(&self, path: &str, offset: i64, size: i64, mtime_ms: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO file_offsets(path, offset, size, mtime_ms) VALUES(?1,?2,?3,?4)
             ON CONFLICT(path) DO UPDATE SET offset=?2, size=?3, mtime_ms=?4",
            params![path, offset, size, mtime_ms],
        )?;
        Ok(())
    }

    // ---- events ----

    pub fn insert_events(&self, events: &[Event]) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let mut inserted = 0usize;
        {
            // Dedup by uuid (= message.id:requestId). When the same response is
            // re-logged with different token counts (e.g. a partial streamed copy
            // vs the final one), keep the copy with the largest total so we never
            // undercount — matching ccusage's effective behavior.
            let mut stmt = tx.prepare(
                "INSERT INTO usage_events
                 (uuid, ts_ms, model, project, session_id,
                  input_tokens, output_tokens, cache_creation, cache_read)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
                 ON CONFLICT(uuid) DO UPDATE SET
                   ts_ms=excluded.ts_ms, model=excluded.model, project=excluded.project,
                   session_id=excluded.session_id, input_tokens=excluded.input_tokens,
                   output_tokens=excluded.output_tokens, cache_creation=excluded.cache_creation,
                   cache_read=excluded.cache_read
                 WHERE (excluded.input_tokens+excluded.output_tokens+excluded.cache_creation+excluded.cache_read)
                     > (input_tokens+output_tokens+cache_creation+cache_read)",
            )?;
            for e in events {
                inserted += stmt.execute(params![
                    e.uuid,
                    e.ts_ms,
                    e.model,
                    e.project,
                    e.session_id,
                    e.input,
                    e.output,
                    e.cache_creation,
                    e.cache_read
                ])?;
            }
        }
        tx.commit()?;
        Ok(inserted)
    }

    pub fn total_events(&self) -> i64 {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM usage_events", [], |r| r.get(0))
            .unwrap_or(0)
    }

    pub fn first_ts(&self) -> Option<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT MIN(ts_ms) FROM usage_events", [], |r| {
            r.get::<_, Option<i64>>(0)
        })
        .ok()
        .flatten()
    }

    /// Raw token sums over [from_ms, to_ms).
    pub fn raw_sum(&self, from_ms: i64, to_ms: i64) -> RawSums {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                    COALESCE(SUM(cache_creation),0), COALESCE(SUM(cache_read),0), COUNT(*)
             FROM usage_events WHERE ts_ms >= ?1 AND ts_ms < ?2",
            params![from_ms, to_ms],
            |r| {
                Ok(RawSums {
                    input: r.get(0)?,
                    output: r.get(1)?,
                    cc: r.get(2)?,
                    cr: r.get(3)?,
                    count: r.get(4)?,
                })
            },
        )
        .unwrap_or_default()
    }

    /// Per-event rows (ts + raw tokens) at or after `from_ms`, ascending.
    pub fn recent_events(&self, from_ms: i64) -> Vec<(i64, i64, i64, i64, i64)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT ts_ms, input_tokens, output_tokens, cache_creation, cache_read
             FROM usage_events WHERE ts_ms >= ?1 ORDER BY ts_ms ASC",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt
            .query_map(params![from_ms], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })
            .map(|it| it.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        rows
    }

    /// Highest weighted usage in any fixed `bucket_ms`-sized bin since `from_ms`.
    pub fn observed_max(&self, from_ms: i64, bucket_ms: i64, w_cc: f64, w_cr: f64) -> i64 {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                    COALESCE(SUM(cache_creation),0), COALESCE(SUM(cache_read),0)
             FROM usage_events WHERE ts_ms >= ?2 GROUP BY (ts_ms / ?1)",
        ) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        let mut max = 0i64;
        let it = stmt.query_map(params![bucket_ms, from_ms], |r| {
            Ok(RawSums {
                input: r.get(0)?,
                output: r.get(1)?,
                cc: r.get(2)?,
                cr: r.get(3)?,
                count: 0,
            })
        });
        if let Ok(it) = it {
            for s in it.flatten() {
                max = max.max(s.weighted(w_cc, w_cr));
            }
        }
        max
    }

    /// Time-series buckets with per-family weighted totals.
    pub fn history(&self, from_ms: i64, bucket_ms: i64, w_cc: f64, w_cr: f64) -> Vec<HistPoint> {
        let conn = self.conn.lock().unwrap();
        let sql = format!(
            "SELECT (ts_ms / ?1) AS b, {fam} AS fam,
                    COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                    COALESCE(SUM(cache_creation),0), COALESCE(SUM(cache_read),0)
             FROM usage_events WHERE ts_ms >= ?2
             GROUP BY b, fam ORDER BY b ASC",
            fam = FAM_CASE
        );
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map(params![bucket_ms, from_ms], |r| {
            let b: i64 = r.get(0)?;
            let fam: String = r.get(1)?;
            let sums = RawSums {
                input: r.get(2)?,
                output: r.get(3)?,
                cc: r.get(4)?,
                cr: r.get(5)?,
                count: 0,
            };
            Ok((b, fam, sums.weighted(w_cc, w_cr)))
        });

        let mut out: Vec<HistPoint> = Vec::new();
        if let Ok(rows) = rows {
            for (b, fam, w) in rows.flatten() {
                let bucket_ms_start = b * bucket_ms;
                let point = match out.last_mut() {
                    Some(p) if p.bucket_ms == bucket_ms_start => p,
                    _ => {
                        out.push(HistPoint {
                            bucket_ms: bucket_ms_start,
                            weighted: 0,
                            opus: 0,
                            sonnet: 0,
                            haiku: 0,
                            other: 0,
                        });
                        out.last_mut().unwrap()
                    }
                };
                point.weighted += w;
                match fam.as_str() {
                    "opus" => point.opus += w,
                    "sonnet" => point.sonnet += w,
                    "haiku" => point.haiku += w,
                    _ => point.other += w,
                }
            }
        }
        out
    }

    /// Breakdown by model family and by project over [from_ms, to_ms), with cost.
    pub fn breakdown(&self, from_ms: i64, to_ms: i64, w_cc: f64, w_cr: f64) -> Breakdown {
        Breakdown {
            by_model: self.model_breakdown(from_ms, to_ms, w_cc, w_cr),
            by_project: self.project_breakdown(from_ms, to_ms, w_cc, w_cr),
        }
    }

    fn model_breakdown(&self, from_ms: i64, to_ms: i64, w_cc: f64, w_cr: f64) -> Vec<BreakdownItem> {
        let conn = self.conn.lock().unwrap();
        // Group by family so opus-4-8 / opus-4-7 / opus-4-6 fold into one "Opus".
        let sql = format!(
            "SELECT {fam} AS k,
                    COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                    COALESCE(SUM(cache_creation),0), COALESCE(SUM(cache_read),0), COUNT(*)
             FROM usage_events WHERE ts_ms >= ?1 AND ts_ms < ?2
             GROUP BY k",
            fam = FAM_CASE
        );
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map(params![from_ms, to_ms], |r| {
            let key: String = r.get(0)?;
            let sums = RawSums {
                input: r.get(1)?,
                output: r.get(2)?,
                cc: r.get(3)?,
                cr: r.get(4)?,
                count: r.get(5)?,
            };
            Ok((key, sums))
        });
        let mut items: Vec<BreakdownItem> = Vec::new();
        if let Ok(rows) = rows {
            for (key, sums) in rows.flatten() {
                let cost = pricing::cost_usd(&key, sums.input, sums.output, sums.cc, sums.cr);
                items.push(BreakdownItem {
                    label: pretty_family(&key),
                    key,
                    weighted: sums.weighted(w_cc, w_cr),
                    raw: sums.to_raw_tokens(),
                    count: sums.count,
                    cost_usd: cost,
                });
            }
        }
        items.sort_by(|a, b| b.weighted.cmp(&a.weighted));
        items
    }

    fn project_breakdown(&self, from_ms: i64, to_ms: i64, w_cc: f64, w_cr: f64) -> Vec<BreakdownItem> {
        let conn = self.conn.lock().unwrap();
        // Group by project AND family so each project's cost is priced correctly.
        let sql = format!(
            "SELECT project, {fam} AS fam,
                    COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                    COALESCE(SUM(cache_creation),0), COALESCE(SUM(cache_read),0), COUNT(*)
             FROM usage_events WHERE ts_ms >= ?1 AND ts_ms < ?2
             GROUP BY project, fam",
            fam = FAM_CASE
        );
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let rows = stmt.query_map(params![from_ms, to_ms], |r| {
            let project: String = r.get(0)?;
            let fam: String = r.get(1)?;
            let sums = RawSums {
                input: r.get(2)?,
                output: r.get(3)?,
                cc: r.get(4)?,
                cr: r.get(5)?,
                count: r.get(6)?,
            };
            Ok((project, fam, sums))
        });
        let mut acc: HashMap<String, (RawSums, f64)> = HashMap::new();
        if let Ok(rows) = rows {
            for (project, fam, sums) in rows.flatten() {
                let cost = pricing::cost_usd(&fam, sums.input, sums.output, sums.cc, sums.cr);
                let e = acc.entry(project).or_insert((RawSums::default(), 0.0));
                e.0.add(&sums);
                e.1 += cost;
            }
        }
        let mut items: Vec<BreakdownItem> = acc
            .into_iter()
            .map(|(project, (sums, cost))| BreakdownItem {
                label: pretty_project(&project),
                key: project,
                weighted: sums.weighted(w_cc, w_cr),
                raw: sums.to_raw_tokens(),
                count: sums.count,
                cost_usd: cost,
            })
            .collect();
        items.sort_by(|a, b| b.weighted.cmp(&a.weighted));
        items
    }

    /// Total equivalent API cost (USD) over [from_ms, to_ms), priced per family.
    pub fn window_cost(&self, from_ms: i64, to_ms: i64) -> f64 {
        let conn = self.conn.lock().unwrap();
        let sql = format!(
            "SELECT {fam} AS k,
                    COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                    COALESCE(SUM(cache_creation),0), COALESCE(SUM(cache_read),0)
             FROM usage_events WHERE ts_ms >= ?1 AND ts_ms < ?2
             GROUP BY k",
            fam = FAM_CASE
        );
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return 0.0,
        };
        let mut total = 0.0;
        let it = stmt.query_map(params![from_ms, to_ms], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
            ))
        });
        if let Ok(it) = it {
            for (key, i, o, cc, cr) in it.flatten() {
                total += pricing::cost_usd(&key, i, o, cc, cr);
            }
        }
        total
    }

    /// Latest events for the request log (newest first).
    pub fn recent_event_rows(&self, limit: i64) -> Vec<(i64, String, String, i64, i64, i64, i64)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT ts_ms, model, project, input_tokens, output_tokens, cache_creation, cache_read
             FROM usage_events ORDER BY ts_ms DESC LIMIT ?1",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![limit], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
            ))
        })
        .map(|it| it.filter_map(|x| x.ok()).collect())
        .unwrap_or_default()
    }

    // ---- settings ----

    pub fn load_settings(&self) -> Settings {
        let conn = self.conn.lock().unwrap();
        let raw: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'app'",
                [],
                |r| r.get(0),
            )
            .ok();
        raw.and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, s: &Settings) -> Result<()> {
        let json = serde_json::to_string(s)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings(key, value) VALUES('app', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1",
            params![json],
        )?;
        Ok(())
    }
}

fn pretty_family(k: &str) -> String {
    match k {
        "opus" => "Opus",
        "sonnet" => "Sonnet",
        "haiku" => "Haiku",
        _ => "其他",
    }
    .to_string()
}

fn pretty_project(p: &str) -> String {
    if p.is_empty() {
        "未知项目".to_string()
    } else {
        p.to_string()
    }
}
