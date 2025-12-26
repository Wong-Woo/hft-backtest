use crossbeam_channel::{Sender, Receiver, select};
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::time::Duration;
use super::commands::{StrategyCommand, ControlResponse, ControlState};

/// Strategy controller that manages execution flow
/// Follows Single Responsibility Principle - only handles control logic
pub struct StrategyController {
    /// Command receiver from GUI
    command_rx: Receiver<StrategyCommand>,
    /// Response sender to GUI
    response_tx: Sender<ControlResponse>,
    /// Current state
    state: Arc<AtomicU64>, // Using u64 to store ControlState as integer
    /// Should stop flag
    should_stop: Arc<AtomicBool>,
    /// Should skip flag
    should_skip: Arc<AtomicBool>,
    /// Speed multiplier (stored as f64 bits in u64)
    speed_multiplier: Arc<AtomicU64>,
}

impl StrategyController {
    pub fn new(
        command_rx: Receiver<StrategyCommand>,
        response_tx: Sender<ControlResponse>,
    ) -> Self {
        Self {
            command_rx,
            response_tx,
            state: Arc::new(AtomicU64::new(ControlState::Paused as u64)),
            should_stop: Arc::new(AtomicBool::new(false)),
            should_skip: Arc::new(AtomicBool::new(false)),
            speed_multiplier: Arc::new(AtomicU64::new(1.0f64.to_bits())),
        }
    }

    /// Get current state
    pub fn state(&self) -> ControlState {
        let state_val = self.state.load(Ordering::Relaxed);
        match state_val {
            0 => ControlState::Running,
            1 => ControlState::Paused,
            2 => ControlState::Stopped,
            3 => ControlState::Completed,
            _ => ControlState::Stopped,
        }
    }

    /// Get speed multiplier
    pub fn speed_multiplier(&self) -> f64 {
        f64::from_bits(self.speed_multiplier.load(Ordering::Relaxed))
    }

    /// Check if should stop
    pub fn should_stop(&self) -> bool {
        self.should_stop.load(Ordering::Relaxed)
    }

    /// Signal to stop execution
    pub fn stop(&self) {
        self.state.store(ControlState::Stopped as u64, Ordering::Relaxed);
        self.should_stop.store(true, Ordering::Relaxed);
    }

    /// Check if should skip current file
    #[allow(dead_code)]
    pub fn should_skip(&self) -> bool {
        self.should_skip.load(Ordering::Relaxed)
    }

    /// Reset skip flag
    #[allow(dead_code)]
    pub fn reset_skip(&self) {
        self.should_skip.store(false, Ordering::Relaxed);
    }

    /// Check if currently running
    pub fn is_running(&self) -> bool {
        self.state() == ControlState::Running
    }

    /// Process commands with timeout
    pub fn process_commands(&self, timeout: Duration) -> bool {
        select! {
            recv(self.command_rx) -> msg => {
                if let Ok(cmd) = msg {
                    self.handle_command(cmd);
                    true
                } else {
                    false
                }
            }
            default(timeout) => false,
        }
    }

    /// Handle a single command
    fn handle_command(&self, command: StrategyCommand) {
        match command {
            StrategyCommand::Start => {
                self.state.store(ControlState::Running as u64, Ordering::Relaxed);
                self.should_stop.store(false, Ordering::Relaxed);
                let _ = self.response_tx.send(ControlResponse::StateChanged(ControlState::Running));
            }
            StrategyCommand::Pause => {
                self.state.store(ControlState::Paused as u64, Ordering::Relaxed);
                let _ = self.response_tx.send(ControlResponse::StateChanged(ControlState::Paused));
            }
            StrategyCommand::Stop => {
                self.state.store(ControlState::Stopped as u64, Ordering::Relaxed);
                self.should_stop.store(true, Ordering::Relaxed);
                let _ = self.response_tx.send(ControlResponse::StateChanged(ControlState::Stopped));
            }
            StrategyCommand::SetSpeed(speed) => {
                let clamped_speed = speed.clamp(0.01, 100.0);
                self.speed_multiplier.store(clamped_speed.to_bits(), Ordering::Relaxed);
                let _ = self.response_tx.send(ControlResponse::SpeedChanged(clamped_speed));
            }
            StrategyCommand::ChangeFiles(files) => {
                // For now, just notify. Actual file change would require restarting
                let _ = self.response_tx.send(ControlResponse::FilesChanged(files));
            }
            StrategyCommand::Skip => {
                self.should_skip.store(true, Ordering::Relaxed);
                let _ = self.response_tx.send(ControlResponse::Skipped);
            }
            StrategyCommand::Reset => {
                self.state.store(ControlState::Paused as u64, Ordering::Relaxed);
                self.should_stop.store(false, Ordering::Relaxed);
                self.should_skip.store(false, Ordering::Relaxed);
                self.speed_multiplier.store(1.0f64.to_bits(), Ordering::Relaxed);
                let _ = self.response_tx.send(ControlResponse::StateChanged(ControlState::Paused));
            }
        }
    }

    /// Mark as completed
    pub fn mark_completed(&self) {
        self.state.store(ControlState::Completed as u64, Ordering::Relaxed);
        let _ = self.response_tx.send(ControlResponse::Completed);
    }

    /// Wait while paused, checking for commands
    pub fn wait_while_paused(&self) {
        while self.state() == ControlState::Paused && !self.should_stop() {
            self.process_commands(Duration::from_millis(100));
        }
    }

    /// Get clones for sharing with strategy thread
    #[allow(dead_code)]
    pub fn get_shared_handles(&self) -> (Arc<AtomicBool>, Arc<AtomicU64>, Arc<AtomicU64>) {
        (
            Arc::clone(&self.should_stop),
            Arc::clone(&self.state),
            Arc::clone(&self.speed_multiplier),
        )
    }
}
