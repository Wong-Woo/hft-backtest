/// Strategy control commands using Command Pattern
#[derive(Debug, Clone)]
pub enum StrategyCommand {
    /// Start or resume strategy execution
    Start,
    /// Pause strategy execution
    Pause,
    /// Stop strategy execution completely
    Stop,
    /// Change execution speed (multiplier: 0.1 = 10x slower, 10.0 = 10x faster)
    SetSpeed(f64),
    /// Change data file
    ChangeFile(String),
    /// Reset strategy state
    Reset,
}

/// Control responses sent back to GUI
#[derive(Debug, Clone)]
pub enum ControlResponse {
    /// Strategy state changed successfully
    StateChanged(ControlState),
    /// Speed changed
    SpeedChanged(f64),
    /// File changed
    FileChanged(String),
    /// Error occurred
    Error(String),
    /// Strategy completed
    Completed,
}

/// Current control state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlState {
    Running,
    Paused,
    Stopped,
    Completed,
}

impl std::fmt::Display for ControlState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlState::Running => write!(f, "Running"),
            ControlState::Paused => write!(f, "Paused"),
            ControlState::Stopped => write!(f, "Stopped"),
            ControlState::Completed => write!(f, "Completed"),
        }
    }
}
