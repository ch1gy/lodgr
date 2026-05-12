use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketStatus {
    Open,
    Acknowledged,
    Closed,
}

impl TicketStatus {
    fn from_str(s: &str) -> AppResult<Self> {
        match s {
            "open" => Ok(Self::Open),
            "acknowledged" => Ok(Self::Acknowledged),
            "closed" => Ok(Self::Closed),
            other => Err(AppError::Internal(format!("unknown ticket status: {other}"))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Acknowledged => "acknowledged",
            Self::Closed => "closed",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TransitionAction {
    Acknowledge,
    Close,
}

impl TransitionAction {
    fn target_str(self) -> &'static str {
        match self {
            Self::Acknowledge => "acknowledged",
            Self::Close => "closed",
        }
    }
}

/// The single place in the codebase where ticket status transitions are validated.
/// Returns the resulting status or `AppError::InvalidTransition`.
pub fn transition(current: &str, action: TransitionAction) -> AppResult<TicketStatus> {
    let from = TicketStatus::from_str(current)?;
    match (from, action) {
        (TicketStatus::Open, TransitionAction::Acknowledge) => Ok(TicketStatus::Acknowledged),
        (TicketStatus::Acknowledged, TransitionAction::Close) => Ok(TicketStatus::Closed),
        _ => Err(AppError::InvalidTransition {
            from: from.as_str().to_owned(),
            to: action.target_str().to_owned(),
        }),
    }
}
