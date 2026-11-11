//! Persisted `index_jobs.status` values and their lifecycle classification.

/// Every status stored in `index_jobs.status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Pending,
    Running,
    ExtractingPages,
    GeneratingThumbnails,
    Success,
    Partial,
    Failed,
    Cancelled,
}

impl JobState {
    pub const ALL: [JobState; 8] = [
        JobState::Pending,
        JobState::Running,
        JobState::ExtractingPages,
        JobState::GeneratingThumbnails,
        JobState::Success,
        JobState::Partial,
        JobState::Failed,
        JobState::Cancelled,
    ];

    /// Statuses where the job is actively doing work (claimed and running).
    pub const ACTIVE: [JobState; 3] = [
        JobState::Running,
        JobState::ExtractingPages,
        JobState::GeneratingThumbnails,
    ];

    /// Statuses where the job still holds a slot (queued or running).
    pub const IN_FLIGHT: [JobState; 4] = [
        JobState::Pending,
        JobState::Running,
        JobState::ExtractingPages,
        JobState::GeneratingThumbnails,
    ];

    /// Statuses where the job is over.
    pub const TERMINAL: [JobState; 4] = [
        JobState::Success,
        JobState::Partial,
        JobState::Failed,
        JobState::Cancelled,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            JobState::Pending => "pending",
            JobState::Running => "running",
            JobState::ExtractingPages => "extracting_pages",
            JobState::GeneratingThumbnails => "generating_thumbnails",
            JobState::Success => "success",
            JobState::Partial => "partial",
            JobState::Failed => "failed",
            JobState::Cancelled => "cancelled",
        }
    }

    pub const fn is_active(self) -> bool {
        matches!(
            self,
            JobState::Running | JobState::ExtractingPages | JobState::GeneratingThumbnails
        )
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            JobState::Success | JobState::Partial | JobState::Failed | JobState::Cancelled
        )
    }

    pub const fn is_in_flight(self) -> bool {
        matches!(
            self,
            JobState::Pending
                | JobState::Running
                | JobState::ExtractingPages
                | JobState::GeneratingThumbnails
        )
    }
}

impl std::fmt::Display for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownJobState;

impl std::fmt::Display for UnknownJobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("unknown job status")
    }
}

impl std::error::Error for UnknownJobState {}

impl std::str::FromStr for JobState {
    type Err = UnknownJobState;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(JobState::Pending),
            "running" => Ok(JobState::Running),
            "extracting_pages" => Ok(JobState::ExtractingPages),
            "generating_thumbnails" => Ok(JobState::GeneratingThumbnails),
            "success" => Ok(JobState::Success),
            "partial" => Ok(JobState::Partial),
            "failed" => Ok(JobState::Failed),
            "cancelled" => Ok(JobState::Cancelled),
            _ => Err(UnknownJobState),
        }
    }
}

/// SQL-friendly strings for a set of statuses.
pub(crate) fn as_strings(states: &[JobState]) -> Vec<String> {
    states
        .iter()
        .map(|state| state.as_str().to_string())
        .collect()
}
