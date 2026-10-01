//! Discover resource limits (`--with-limits` / `RGCTL_WITH_LIMITS`).

use anyhow::{bail, Context, Result};
use tracing::info;

/// Opt-in discover constraints. Default discover leaves these unset.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoverLimits {
    /// Soft RSS budget in mebibytes (`max-mem-mb`).
    pub max_mem_mb: Option<u64>,
    /// Cap Rayon / pipeline worker threads (`threads`).
    pub threads: Option<usize>,
}

impl DiscoverLimits {
    /// Parse `max-mem-mb=4096,threads=1` (comma-separated `key=value`).
    ///
    /// Empty string ⇒ enabled with no overrides (placeholder for future cgroup defaults).
    pub fn parse(spec: &str) -> Result<Self> {
        let mut out = Self::default();
        let trimmed = spec.trim();
        if trimmed.is_empty() {
            return Ok(out);
        }
        for part in trimmed.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (key, value) = part
                .split_once('=')
                .with_context(|| format!("invalid --with-limits entry `{part}` (want key=value)"))?;
            let key = key.trim().to_ascii_lowercase().replace('_', "-");
            let value = value.trim();
            match key.as_str() {
                "max-mem-mb" | "max-mem" | "memory-mb" => {
                    let mb: u64 = value
                        .parse()
                        .with_context(|| format!("invalid max-mem-mb `{value}`"))?;
                    if mb == 0 {
                        bail!("max-mem-mb must be > 0");
                    }
                    out.max_mem_mb = Some(mb);
                }
                "threads" | "thread" | "max-thread" | "max-threads" => {
                    let n: usize = value
                        .parse()
                        .with_context(|| format!("invalid threads `{value}`"))?;
                    if n == 0 {
                        bail!("threads must be > 0");
                    }
                    out.threads = Some(n);
                }
                other => bail!(
                    "unknown --with-limits key `{other}` (supported: max-mem-mb, threads)"
                ),
            }
        }
        Ok(out)
    }

    /// Resolve CLI `--with-limits [SPEC]` or `RGCTL_WITH_LIMITS` when the flag is omitted.
    pub fn from_cli(spec: Option<&str>) -> Result<Option<Self>> {
        let resolved = match spec {
            Some(s) => Some(s.to_owned()),
            None => std::env::var("RGCTL_WITH_LIMITS").ok(),
        };
        match resolved.as_deref() {
            None => Ok(None),
            Some(s) => Ok(Some(Self::parse(s)?)),
        }
    }

    /// Whether any constraint was requested (flag present with values).
    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.max_mem_mb.is_some() || self.threads.is_some()
    }

    /// Stream channel capacity when limits are active (else leave pipeline default).
    pub fn stream_channel_capacity(&self) -> Option<usize> {
        let budget = self.max_mem_mb? as usize;
        // ~10 MB assumed per in-flight extraction; clamp to [32, 1024].
        Some((budget / 10).clamp(32, 1024))
    }

    /// Spill external-sort run size when limits are active.
    pub fn sort_run_bytes(&self) -> Option<usize> {
        match self.max_mem_mb? {
            mb if mb <= 2048 => Some(32 * 1024 * 1024),
            mb if mb <= 8192 => Some(64 * 1024 * 1024),
            _ => None, // keep default 256 MiB on large budgets
        }
    }

    pub fn log_active(&self) {
        info!(
            max_mem_mb = ?self.max_mem_mb,
            threads = ?self.threads,
            stream_channel = ?self.stream_channel_capacity(),
            sort_run_mb = self.sort_run_bytes().map(|b| b / (1024 * 1024)),
            "discover --with-limits active"
        );
    }
}

/// Soft memory budget check (warn ≥90%, error ≥95%).
pub fn check_memory_budget(peak_mb: f64, limit_mb: u64) -> Result<()> {
    let limit = limit_mb as f64;
    if peak_mb >= limit * 0.95 {
        bail!(
            "Memory limit of {limit_mb} MB approached (peak RSS: {peak_mb:.0} MB). \
             Discovery aborted to avoid container OOM. \
             Raise max-mem-mb, set threads lower, or filter with -l."
        );
    }
    if peak_mb >= limit * 0.90 {
        tracing::warn!(
            peak_mb,
            limit_mb,
            "RSS within 90% of --with-limits max-mem-mb"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kv_pair() {
        let l = DiscoverLimits::parse("max-mem-mb=4096,threads=1").unwrap();
        assert_eq!(l.max_mem_mb, Some(4096));
        assert_eq!(l.threads, Some(1));
        assert_eq!(l.stream_channel_capacity(), Some(409));
        assert_eq!(l.sort_run_bytes(), Some(64 * 1024 * 1024));
    }

    #[test]
    fn parse_empty() {
        let l = DiscoverLimits::parse("").unwrap();
        assert!(!l.is_active());
    }

    #[test]
    fn reject_unknown_key() {
        assert!(DiscoverLimits::parse("foo=1").is_err());
    }

    #[test]
    fn budget_tripwire() {
        // 90% of 4096 ≈ 3686; 95% ≈ 3891
        assert!(check_memory_budget(3700.0, 4096).is_ok());
        assert!(check_memory_budget(4000.0, 4096).is_err());
    }

    #[test]
    fn from_cli_reads_env_when_flag_absent() {
        // SAFETY: test process; key is test-scoped.
        unsafe { std::env::set_var("RGCTL_WITH_LIMITS", "threads=2") };
        let l = DiscoverLimits::from_cli(None).unwrap().unwrap();
        assert_eq!(l.threads, Some(2));
        // Flag wins over env.
        let l = DiscoverLimits::from_cli(Some("threads=1")).unwrap().unwrap();
        assert_eq!(l.threads, Some(1));
        unsafe { std::env::remove_var("RGCTL_WITH_LIMITS") };
    }
}
