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
}

impl ControlPanel {
    pub fn new(command_tx: Sender<StrategyCommand>, initial_file: String) -> Self {
        Self {
            command_tx,
            current_state: ControlState::Paused,
            speed_multiplier: 1.0,
            file_paths: vec![initial_file],
        }
    }

    pub fn update_state(&mut self, state: ControlState) {
        self.current_state = state;
    }

    pub fn update_speed(&mut self, speed: f64) {
        self.speed_multiplier = speed;
    }

    pub fn update_files(&mut self, files: Vec<String>) {
        self.file_paths = files;
    }

    fn select_files(&mut self) {
        // Open file dialog in a separate thread to avoid blocking UI
        let command_tx = self.command_tx.clone();
        
        std::thread::spawn(move || {
            if let Some(files) = rfd::FileDialog::new()
                .add_filter("NPZ Data Files", &["npz"])
                .add_filter("CSV Data Files", &["csv"])
                .add_filter("All Files", &["*"])
                .set_title("Select Backtest Data Files (Multiple Selection Supported)")
                .pick_files()
            {
                let file_paths: Vec<String> = files
                    .iter()
                    .filter_map(|p| p.to_str().map(|s| s.to_string()))
                    .collect();
                
                if !file_paths.is_empty() {
                    let _ = command_tx.send(StrategyCommand::ChangeFiles(file_paths));
                }
            }
        });
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.heading("🎮 Strategy Control");
                
                // State indicator
                let (color, emoji) = match self.current_state {
                    ControlState::Running => (egui::Color32::GREEN, "▶"),
                    ControlState::Paused => (egui::Color32::YELLOW, "⏸"),
                    ControlState::Stopped => (egui::Color32::RED, "⏹"),
                    ControlState::Completed => (egui::Color32::BLUE, "✓"),
                };
                
                ui.label(
                    egui::RichText::new(format!("{} {}", emoji, self.current_state))
                        .color(color)
                        .strong()
                );
            });
            
            ui.separator();
            
            // Control buttons
            ui.horizontal(|ui| {
                let can_start = matches!(self.current_state, ControlState::Paused | ControlState::Stopped);
                let can_pause = self.current_state == ControlState::Running;
                let can_stop = !matches!(self.current_state, ControlState::Stopped | ControlState::Completed);
                let can_skip = self.current_state == ControlState::Running && self.file_paths.len() > 1;
                
                if ui.add_enabled(can_start, egui::Button::new("▶ Start")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::Start);
                }
                
                if ui.add_enabled(can_pause, egui::Button::new("⏸ Pause")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::Pause);
                }
                
                if ui.add_enabled(can_stop, egui::Button::new("⏹ Stop")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::Stop);
                }
                
                if ui.add_enabled(can_skip, egui::Button::new("⏭️ Skip")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::Skip);
                }
                
                if ui.button("🔄 Reset").clicked() {
                    let _ = self.command_tx.send(StrategyCommand::Reset);
                }
            });
            
            ui.separator();
            
            // Speed control
            ui.horizontal(|ui| {
                ui.label("⚡ Speed:");
                
                if ui.button("0.1x").clicked() {
                    let _ = self.command_tx.send(StrategyCommand::SetSpeed(0.1));
                }
                if ui.button("0.5x").clicked() {
                    let _ = self.command_tx.send(StrategyCommand::SetSpeed(0.5));
                }
                if ui.button("1x").clicked() {
                    let _ = self.command_tx.send(StrategyCommand::SetSpeed(1.0));
                }
                if ui.button("2x").clicked() {
                    let _ = self.command_tx.send(StrategyCommand::SetSpeed(2.0));
                }
                if ui.button("10x").clicked() {
                    let _ = self.command_tx.send(StrategyCommand::SetSpeed(10.0));
                }
                
                ui.label(format!("Current: {:.2}x", self.speed_multiplier));
            });
            
            ui.separator();
            
            // Custom speed slider
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
            
            ui.separator();
            
            // File selection and info
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label("📁 Data Files:");
                    if ui.button("📂 Select Files...").clicked() {
                        self.select_files();
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
            });
        });
    }
}
