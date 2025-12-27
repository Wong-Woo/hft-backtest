use eframe::egui;
use crossbeam_channel::Sender;
use crate::controller::{StrategyCommand, ControlState};
use std::path::PathBuf;

/// Control panel for strategy execution
pub struct ControlPanel {
    command_tx: Sender<StrategyCommand>,
    current_state: ControlState,
    speed_multiplier: f64,
    file_paths: Vec<String>,
    pending_file_change: bool,
    can_start_new: bool,        // Whether a new backtest can be started
    start_new_requested: bool,  // Flag to signal start new backtest to monitor
}

impl ControlPanel {
    pub fn new(command_tx: Sender<StrategyCommand>, initial_file: String) -> Self {
        Self {
            command_tx,
            current_state: ControlState::Paused,
            speed_multiplier: 1.0,
            file_paths: vec![initial_file],
            pending_file_change: false,
            can_start_new: true,
            start_new_requested: false,
        }
    }

    pub fn update_state(&mut self, state: ControlState) {
        self.current_state = state;
        if state == ControlState::Running {
            self.pending_file_change = false;
        }
    }

    pub fn update_speed(&mut self, speed: f64) {
        self.speed_multiplier = speed;
    }

    pub fn update_files(&mut self, files: Vec<String>) {
        self.file_paths = files;
        self.pending_file_change = true;
    }
    
    pub fn update_command_sender(&mut self, new_tx: Sender<StrategyCommand>) {
        self.command_tx = new_tx;
    }
    
    pub fn set_can_start_new(&mut self, can_start: bool) {
        self.can_start_new = can_start;
    }
    
    /// Mark that new files must be selected before starting
    pub fn mark_needs_new_files(&mut self) {
        self.pending_file_change = false;
    }
    
    pub fn should_start_new_backtest(&mut self) -> bool {
        let requested = self.start_new_requested;
        self.start_new_requested = false;
        requested
    }
    
    /// Get all selected file paths
    pub fn get_selected_files(&self) -> Vec<String> {
        self.file_paths.clone()
    }

    fn select_files(&mut self) {
        if let Some(files) = rfd::FileDialog::new()
            .add_filter("NPZ Data Files", &["npz"])
            .add_filter("CSV Data Files", &["csv"])
            .add_filter("All Files", &["*"])
            .set_title("Select Backtest Data Files")
            .pick_files()
        {
            let file_paths: Vec<String> = files
                .iter()
                .filter_map(|p| p.to_str().map(|s| s.to_string()))
                .collect();
            
            if !file_paths.is_empty() {
                self.file_paths = file_paths;
                self.pending_file_change = true;
            }
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.heading("🎮 Strategy Control");
                
                // State indicator
                let (color, emoji, status_text) = match self.current_state {
                    ControlState::Running => (egui::Color32::GREEN, "▶", "Running"),
                    ControlState::Paused => (egui::Color32::YELLOW, "⏸", "Paused"),
                    ControlState::Stopped => (egui::Color32::RED, "⏹", "Stopped"),
                    ControlState::Completed => (egui::Color32::LIGHT_BLUE, "✓", "Completed"),
                };
                
                ui.label(
                    egui::RichText::new(format!("{} {}", emoji, status_text))
                        .color(color)
                        .strong()
                );
            });
            
            // Show hint when can start new backtest
            if self.can_start_new && matches!(self.current_state, ControlState::Stopped | ControlState::Completed | ControlState::Paused) {
                if (self.current_state == ControlState::Paused || self.current_state == ControlState::Completed) && self.pending_file_change {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("📋 New files selected. Click 'Start New' to begin.")
                                .small()
                                .color(egui::Color32::GOLD)
                        );
                    });
                } else if self.current_state == ControlState::Completed {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("✅ Backtest completed. Select new files to run again.")
                                .small()
                                .color(egui::Color32::LIGHT_GREEN)
                        );
                    });
                } else if self.current_state == ControlState::Stopped {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("⏹ Stopped. Select files and click 'Start New'.")
                                .small()
                                .color(egui::Color32::LIGHT_GREEN)
                        );
                    });
                }
            }
            
            ui.separator();
            
            // Control buttons with improved logic
            ui.horizontal(|ui| {
                // Start New: Available when can_start_new and:
                // - Stopped: can start immediately
                // - Completed: requires new file selection (pending_file_change)
                // - Paused with pending files
                let can_start_new = self.can_start_new && (
                    self.current_state == ControlState::Stopped ||
                    (self.current_state == ControlState::Completed && self.pending_file_change) ||
                    (self.current_state == ControlState::Paused && self.pending_file_change)
                );
                
                // Resume: Only when Paused and no pending file change
                let can_resume = self.current_state == ControlState::Paused && !self.pending_file_change;
                
                // Pause: Only when Running
                let can_pause = self.current_state == ControlState::Running;
                
                // Stop: When Running or Paused
                let can_stop = matches!(
                    self.current_state, 
                    ControlState::Running | ControlState::Paused
                );
                
                // Skip: Only when Running and multiple files
                let can_skip = self.current_state == ControlState::Running && self.file_paths.len() > 1;
                
                // Start New button - spawn new thread
                if ui.add_enabled(can_start_new, egui::Button::new("🚀 Start New")).clicked() {
                    self.start_new_requested = true;
                    self.pending_file_change = false;
                    self.current_state = ControlState::Running; // Optimistic update
                }
                
                // Resume button
                if ui.add_enabled(can_resume, egui::Button::new("▶ Resume")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::Start);
                }
                
                if ui.add_enabled(can_pause, egui::Button::new("⏸ Pause")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::Pause);
                }
                
                if ui.add_enabled(can_stop, egui::Button::new("⏹ Stop")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::Stop);
                }
                
                if ui.add_enabled(can_skip, egui::Button::new("⏭ Skip")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::Skip);
                }
            });
            
            ui.separator();
            
            // Speed control - only enabled when Running or Paused
            let speed_enabled = matches!(
                self.current_state, 
                ControlState::Running | ControlState::Paused
            );
            
            ui.horizontal(|ui| {
                ui.label("⚡ Speed:");
                
                if ui.add_enabled(speed_enabled, egui::Button::new("0.1x")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::SetSpeed(0.1));
                }
                if ui.add_enabled(speed_enabled, egui::Button::new("0.5x")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::SetSpeed(0.5));
                }
                if ui.add_enabled(speed_enabled, egui::Button::new("1x")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::SetSpeed(1.0));
                }
                if ui.add_enabled(speed_enabled, egui::Button::new("2x")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::SetSpeed(2.0));
                }
                if ui.add_enabled(speed_enabled, egui::Button::new("10x")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::SetSpeed(10.0));
                }
                
                ui.label(format!("Current: {:.2}x", self.speed_multiplier));
            });
            
            ui.separator();
            
            // Custom speed slider
            ui.add_enabled_ui(speed_enabled, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Custom Speed:");
                    let mut temp_speed = self.speed_multiplier;
                    if ui.add(
                        egui::Slider::new(&mut temp_speed, 0.01..=100.0)
                            .logarithmic(true)
                            .text("x")
                    ).changed() {
                        let _ = self.command_tx.send(StrategyCommand::SetSpeed(temp_speed));
                    }
                });
            });
            
            ui.separator();
            
            // File selection - only enabled when not Running
            let file_select_enabled = self.current_state != ControlState::Running;
            
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label("📁 Data Files:");
                    
                    if ui.add_enabled(
                        file_select_enabled, 
                        egui::Button::new("📂 Select Files...")
                    ).clicked() {
                        self.select_files();
                    }
                    
                    if !file_select_enabled {
                        ui.label(
                            egui::RichText::new("(Pause to change)")
                                .small()
                                .weak()
                        );
                    }
                });
                
                // Display selected files
                if self.file_paths.is_empty() {
                    ui.label(egui::RichText::new("No files selected").italics().weak());
                } else {
                    ui.group(|ui| {
                        egui::ScrollArea::vertical()
                            .max_height(100.0)
                            .show(ui, |ui| {
                                for (idx, file_path) in self.file_paths.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        ui.label(format!("{}.", idx + 1));
                                        ui.label(
                                            egui::RichText::new(
                                                PathBuf::from(file_path)
                                                    .file_name()
                                                    .and_then(|n| n.to_str())
                                                    .unwrap_or(file_path)
                                            )
                                            .small()
                                            .monospace()
                                        );
                                    });
                                }
                            });
                        ui.label(
                            egui::RichText::new(format!("Total: {} file(s)", self.file_paths.len()))
                                .small()
                                .weak()
                        );
                    });
                }
                
                // Show restart hint for file change
                if self.pending_file_change && !file_select_enabled {
                    ui.label(
                        egui::RichText::new("⚠ Pause or Stop to apply file changes")
                            .small()
                            .color(egui::Color32::GOLD)
                    );
                }
            });
        });
    }
}
