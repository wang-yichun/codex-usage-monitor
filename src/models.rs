use std::time::SystemTime;

#[derive(Clone, Debug, Default)]
pub struct UsageSection {
    pub percentage: f64,
    pub resets_at: Option<SystemTime>,
}

#[derive(Clone, Debug, Default)]
pub struct UsageData {
    pub session: UsageSection,
    pub weekly: UsageSection,
    pub reset_credits: Option<ResetCreditsInfo>,
}

#[derive(Clone, Debug, Default)]
pub struct ResetCreditsInfo {
    /// Authoritative server count. None means the server did not provide it.
    pub available_count: Option<usize>,
    /// Available detail rows only; the backend may return fewer details than the count.
    pub credits: Vec<ResetCredit>,
    pub details_available: bool,
}

#[derive(Clone, Debug)]
pub struct ResetCredit {
    pub expires_at: Option<SystemTime>,
}

#[derive(Clone, Debug, Default)]
pub struct AppUsageData {
    pub claude_code: Option<UsageData>,
    pub codex: Option<UsageData>,
    pub antigravity: Option<UsageData>,
}
