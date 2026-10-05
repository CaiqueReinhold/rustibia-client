use std::collections::{HashMap, HashSet};
use std::time::Duration;

use bevy::prelude::*;

use crate::core::SpriteSheet;

const SUMMARY_PERIOD: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(super) struct SheetMetrics {
    resident: HashSet<String>,
    evicted_at: HashMap<String, Duration>,
    loads: u32,
    evictions: u32,
    reload_gaps: Vec<Duration>,
    peak_bytes: u64,
    last_summary: Duration,
}

impl SheetMetrics {
    pub(super) fn observe(
        &mut self,
        resident: &[(&str, &SpriteSheet)],
        users: &HashSet<&str>,
        now: Duration,
    ) {
        let total: u64 = resident.iter().map(|(_, sheet)| sheet_bytes(sheet)).sum();
        self.peak_bytes = self.peak_bytes.max(total);

        for (group, _) in resident {
            if !self.resident.insert((*group).to_owned()) {
                continue;
            }
            self.loads += 1;
            if let Some(evicted) = self.evicted_at.remove(*group) {
                let gap = now.saturating_sub(evicted);
                info!("sheet {group} reloaded {} after eviction", minutes(gap));
                self.reload_gaps.push(gap);
            }
        }

        if now.saturating_sub(self.last_summary) >= SUMMARY_PERIOD {
            self.last_summary = now;
            info!("{}", self.summary(resident, users, total));
        }
    }

    pub(super) fn evicted(&mut self, group: &str, sheet: &SpriteSheet, now: Duration) {
        self.resident.remove(group);
        self.evicted_at.insert(group.to_owned(), now);
        self.evictions += 1;
        info!("sheet {group} evicted ({})", mebibytes(sheet_bytes(sheet)));
    }

    fn summary(
        &self,
        resident: &[(&str, &SpriteSheet)],
        users: &HashSet<&str>,
        total: u64,
    ) -> String {
        let idle: Vec<u64> = resident
            .iter()
            .filter(|(group, _)| !users.contains(group))
            .map(|(_, sheet)| sheet_bytes(sheet))
            .collect();
        let mut line = format!(
            "sheets: {} resident / {} ({} idle, {}) · peak {} · {} loads, {} evictions, {} reloads",
            resident.len(),
            mebibytes(total),
            idle.len(),
            mebibytes(idle.iter().sum()),
            mebibytes(self.peak_bytes),
            self.loads,
            self.evictions,
            self.reload_gaps.len(),
        );
        if let Some(median) = median(&self.reload_gaps) {
            line.push_str(&format!(" (median {} after eviction)", minutes(median)));
        }
        line
    }
}

/// Decoded RGBA8 size of the whole sheet.
fn sheet_bytes(sheet: &SpriteSheet) -> u64 {
    (sheet.grid_size * sheet.sprite_size).element_product() as u64 * 4
}

fn median(gaps: &[Duration]) -> Option<Duration> {
    let mut sorted = gaps.to_vec();
    sorted.sort();
    sorted.get(sorted.len() / 2).copied()
}

fn mebibytes(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
}

fn minutes(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!("{}m{:02}s", secs / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> SpriteSheet {
        SpriteSheet::for_test("outfit-a.png", Vec2::splat(32.0), Vec2::splat(64.0))
    }

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    #[test]
    fn a_sheet_resident_again_after_eviction_is_a_reload() {
        let sheet = sheet();
        let nobody = HashSet::new();
        let mut metrics = SheetMetrics::default();

        metrics.observe(&[("outfit-a", &sheet)], &nobody, secs(0));
        metrics.evicted("outfit-a", &sheet, secs(300));
        metrics.observe(&[], &nobody, secs(301));
        metrics.observe(&[("outfit-a", &sheet)], &nobody, secs(402));

        assert_eq!(metrics.loads, 2);
        assert_eq!(metrics.evictions, 1);
        assert_eq!(metrics.reload_gaps, vec![secs(102)]);
    }

    #[test]
    fn the_summary_reports_residency_idleness_and_reloads() {
        let used = sheet();
        let idle = SpriteSheet::for_test("item-a.png", Vec2::new(32.0, 16.0), Vec2::splat(32.0));
        let resident = [("outfit-a", &used), ("item-a", &idle)];
        let users = HashSet::from(["outfit-a"]);
        let mut metrics = SheetMetrics::default();

        metrics.observe(&resident, &users, secs(0));
        metrics.reload_gaps = vec![secs(130), secs(10), secs(200)];

        assert_eq!(
            metrics.summary(&resident, &users, 18 * 1024 * 1024),
            "sheets: 2 resident / 18.0 MiB (1 idle, 2.0 MiB) · peak 18.0 MiB · \
             2 loads, 0 evictions, 3 reloads (median 2m10s after eviction)"
        );
    }
}
