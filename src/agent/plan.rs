use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

/// Shared plan mode state backed by an atomic boolean.
/// Cloning shares the same underlying state.
#[derive(Clone)]
pub struct PlanModeState {
    inner: Arc<AtomicBool>,
}

impl PlanModeState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.load(Ordering::SeqCst)
    }

    pub fn set(&self, enabled: bool) {
        self.inner.store(enabled, Ordering::SeqCst);
    }
}

impl Default for PlanModeState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_mode_default() {
        let state = PlanModeState::new();
        assert!(!state.is_enabled());
    }

    #[test]
    fn test_plan_mode_toggle() {
        let state = PlanModeState::new();
        state.set(true);
        assert!(state.is_enabled());
        state.set(false);
        assert!(!state.is_enabled());
    }

    #[test]
    fn test_plan_mode_clone_shares_state() {
        let state = PlanModeState::new();
        let clone = state.clone();
        state.set(true);
        assert!(clone.is_enabled());
    }
}
