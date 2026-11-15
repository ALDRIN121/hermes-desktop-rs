//! Port ledger: which of the shipped IPC channels this shell answers.
//!
//! The inventory itself is generated (see `channels.rs`). This module records
//! the honest status of each channel so a partial port cannot be mistaken for a
//! complete one: `Ported` runs real Rust, `Stubbed` returns a typed
//! "unimplemented" error, and anything else in the inventory falls through to
//! the same refusal.

use crate::channels::CHANNELS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelStatus {
    /// Implemented in Rust: the shell answers with real behavior.
    Ported,
    /// Enumerated and refused with `unimplemented:<channel>`.
    Stubbed,
}

/// Channels the Rust shell actually answers. Keep in sync with
/// `src-tauri/src/commands.rs`.
pub const PORTED: &[&str] = &[
    "hermes:api",
    "hermes:boot-progress:get",
    "hermes:bootstrap:get",
    "hermes:bootstrap:repair",
    "hermes:connection",
    "hermes:connection:for",
    "hermes:connection:revalidate",
    "hermes:data-url-read-max:get",
    "hermes:data-url-read-max:set",
    "hermes:embed-host:origin",
    "hermes:feature-flags",
    "hermes:fs:desktopPluginsRoot",
    "hermes:fs:gitRoot",
    "hermes:fs:logsRoot",
    "hermes:fs:openDir",
    "hermes:fs:readDir",
    "hermes:fs:reconcileDesktopPlugins",
    "hermes:fs:rename",
    "hermes:fs:reveal",
    "hermes:fs:trash",
    "hermes:fs:writeText",
    "hermes:gateway:ws-url",
    "hermes:gateway:ws-url-for",
    "hermes:git:baseBranchList",
    "hermes:git:branchList",
    "hermes:git:branchSwitch",
    "hermes:git:fileDiff",
    "hermes:git:repoStatus",
    "hermes:git:review:commit",
    "hermes:git:review:commitContext",
    "hermes:git:review:createPr",
    "hermes:git:review:diff",
    "hermes:git:review:list",
    "hermes:git:review:prList",
    "hermes:git:review:push",
    "hermes:git:review:revParse",
    "hermes:git:review:revert",
    "hermes:git:review:shipInfo",
    "hermes:git:review:stage",
    "hermes:git:review:unstage",
    "hermes:git:scanRepos",
    "hermes:git:worktreeAdd",
    "hermes:git:worktreeList",
    "hermes:git:worktreeRemove",
    "hermes:hud:windowing",
    "hermes:keep-awake",
    "hermes:local-backend:probe",
    "hermes:logs:recent",
    "hermes:logs:renderer-error",
    "hermes:logs:renderer-line",
    "hermes:logs:reveal",
    "hermes:machine:profile",
    "hermes:normalizePreviewTarget",
    "hermes:notify",
    "hermes:openExternal",
    "hermes:prepared-submissions:read",
    "hermes:readClipboard",
    "hermes:readFileDataUrl",
    "hermes:readFileDataUrlForAttach",
    "hermes:readFileText",
    "hermes:readPluginSource",
    "hermes:saveGatewayFile",
    "hermes:saveImageBuffer",
    "hermes:saveImageFromUrl",
    "hermes:savePastedText",
    "hermes:saveTextFileAs",
    "hermes:selectPaths",
    "hermes:selectSavePath",
    "hermes:setting:defaultProjectDir:get",
    "hermes:setting:defaultProjectDir:pick",
    "hermes:setting:defaultProjectDir:set",
    "hermes:skin:local",
    "hermes:ssh-config:resolve",
    "hermes:terminal:attach",
    "hermes:terminal:cwd",
    "hermes:terminal:dispose",
    "hermes:terminal:resize",
    "hermes:terminal:start",
    "hermes:terminal:write",
    "hermes:translucency:support",
    "hermes:ui-preferences:read",
    "hermes:ui-preferences:snapshot",
    "hermes:ui-preferences:write",
    "hermes:version",
    "hermes:watchPreviewFile",
    "hermes:window-control",
    "hermes:writeClipboard",
    "hermes:zoom:get",
    "hermes:zoom:set-percent",
];

pub fn status(channel: &str) -> ChannelStatus {
    if PORTED.contains(&channel) {
        ChannelStatus::Ported
    } else {
        ChannelStatus::Stubbed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortLedger {
    pub total: usize,
    pub ported: usize,
    pub stubbed: usize,
    /// Request channels (invoke → handler). Push-only events are excluded: they
    /// are not refusable requests, so counting them as "unported work" would
    /// overstate the gap.
    pub requests: usize,
    pub requests_ported: usize,
    pub events: usize,
    pub categories: Vec<(&'static str, usize)>,
}

impl PortLedger {
    pub fn measure() -> Self {
        let total = CHANNELS.len();
        let events = crate::channels::EVENT_CHANNELS.len();
        let requests = total - events;
        let ported = PORTED.iter().filter(|c| CHANNELS.contains(c)).count();
        let requests_ported = PORTED
            .iter()
            .filter(|c| CHANNELS.contains(c) && !crate::channels::EVENT_CHANNELS.contains(c))
            .count();
        PortLedger {
            total,
            ported,
            stubbed: total - ported,
            requests,
            requests_ported,
            events,
            categories: crate::channels::CATEGORY_COUNTS.to_vec(),
        }
    }

    pub fn percent_ported(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.ported as f64 / self.total as f64) * 100.0
    }

    /// Share of the request surface the shell actually answers.
    pub fn percent_requests_ported(&self) -> f64 {
        if self.requests == 0 {
            return 0.0;
        }
        (self.requests_ported as f64 / self.requests as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ported_channel_exists_in_the_inventory() {
        for channel in PORTED {
            assert!(
                CHANNELS.contains(channel),
                "{channel} is claimed as ported but is not a shipped channel"
            );
        }
    }

    #[test]
    fn inventory_matches_the_shipped_surface() {
        // REA read 281 literal IPC channels out of the shipped app.asar; the
        // generated inventory must not silently drift below that.
        assert_eq!(CHANNELS.len(), 281);
        assert!(CHANNELS.contains(&"hermes:api"));
        assert!(CHANNELS.contains(&"hermes:ui-preferences:snapshot"));
    }

    #[test]
    fn ledger_counts_add_up() {
        let ledger = PortLedger::measure();
        assert_eq!(ledger.ported + ledger.stubbed, ledger.total);
        assert_eq!(ledger.ported, PORTED.len());
        assert_eq!(ledger.requests + ledger.events, ledger.total);
        // A partial port must never read as complete; the exact number moves as
        // channels land, the ceiling does not.
        assert!(ledger.percent_ported() < 100.0);
        assert!(ledger.percent_requests_ported() < 100.0);
        assert!(ledger.ported > 0);
    }

    #[test]
    fn events_are_never_counted_as_ported_requests() {
        let ledger = PortLedger::measure();
        for channel in PORTED {
            assert!(
                !crate::channels::EVENT_CHANNELS.contains(channel),
                "{channel} is a push-only event and cannot be a ported request"
            );
        }
        assert_eq!(ledger.requests_ported, PORTED.len());
    }
}
