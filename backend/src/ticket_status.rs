use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketStatus {
    Open,
    Pending,
    Acknowledged,
    Closed,
}

impl TicketStatus {
    fn parse(s: &str) -> AppResult<Self> {
        match s {
            "open" => Ok(Self::Open),
            "pending" => Ok(Self::Pending),
            "acknowledged" => Ok(Self::Acknowledged),
            "closed" => Ok(Self::Closed),
            other => Err(AppError::Internal(format!(
                "unknown ticket status: {other}"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Pending => "pending",
            Self::Acknowledged => "acknowledged",
            Self::Closed => "closed",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TransitionAction {
    Acknowledge,
    Pend,
    Close,
}

impl TransitionAction {
    fn target_str(self) -> &'static str {
        match self {
            Self::Acknowledge => "acknowledged",
            Self::Pend => "pending",
            Self::Close => "closed",
        }
    }
}

/// The single place in the codebase where ticket status transitions are validated.
/// Returns the resulting status or `AppError::InvalidTransition`.
///
/// Valid transitions:
///   open → acknowledged    (desk acknowledges immediately)
///   open → pending         (desk needs more info from client)
///   pending → acknowledged (desk has what they need)
///   acknowledged → closed  (desk resolves)
///
/// pending → open is intentionally not allowed — use acknowledge, then close.
pub fn transition(current: &str, action: TransitionAction) -> AppResult<TicketStatus> {
    let from = TicketStatus::parse(current)?;
    match (from, action) {
        (TicketStatus::Open, TransitionAction::Acknowledge) => Ok(TicketStatus::Acknowledged),
        (TicketStatus::Open, TransitionAction::Pend) => Ok(TicketStatus::Pending),
        (TicketStatus::Pending, TransitionAction::Acknowledge) => Ok(TicketStatus::Acknowledged),
        (TicketStatus::Acknowledged, TransitionAction::Close) => Ok(TicketStatus::Closed),
        _ => Err(AppError::InvalidTransition {
            from: from.as_str().to_owned(),
            to: action.target_str().to_owned(),
        }),
    }
}

// ── T4 — state machine unit tests ────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_to_acknowledged() {
        assert_eq!(
            transition("open", TransitionAction::Acknowledge)
                .unwrap()
                .as_str(),
            "acknowledged"
        );
    }

    #[test]
    fn open_to_pending() {
        assert_eq!(
            transition("open", TransitionAction::Pend).unwrap().as_str(),
            "pending"
        );
    }

    #[test]
    fn pending_to_acknowledged() {
        assert_eq!(
            transition("pending", TransitionAction::Acknowledge)
                .unwrap()
                .as_str(),
            "acknowledged"
        );
    }

    #[test]
    fn acknowledged_to_closed() {
        assert_eq!(
            transition("acknowledged", TransitionAction::Close)
                .unwrap()
                .as_str(),
            "closed"
        );
    }

    #[test]
    fn open_to_closed_rejected() {
        assert!(matches!(
            transition("open", TransitionAction::Close),
            Err(AppError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn pending_to_closed_rejected() {
        assert!(matches!(
            transition("pending", TransitionAction::Close),
            Err(AppError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn pending_to_pend_rejected() {
        assert!(matches!(
            transition("pending", TransitionAction::Pend),
            Err(AppError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn acknowledged_to_open_rejected() {
        assert!(matches!(
            transition("acknowledged", TransitionAction::Pend),
            Err(AppError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn closed_to_acknowledge_rejected() {
        assert!(matches!(
            transition("closed", TransitionAction::Acknowledge),
            Err(AppError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn closed_to_pend_rejected() {
        assert!(matches!(
            transition("closed", TransitionAction::Pend),
            Err(AppError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn closed_to_close_rejected() {
        assert!(matches!(
            transition("closed", TransitionAction::Close),
            Err(AppError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn unknown_status_returns_error() {
        assert!(transition("flying", TransitionAction::Acknowledge).is_err());
    }
}
