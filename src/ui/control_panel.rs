use eframe::egui;
use crossbeam_channel::Sender;
use crate::controller::{StrategyCommand, ControlState};

/// Control panel for strategy execution
pub struct ControlPanel {
    command_tx: Sender<StrategyCommand>,
    current_state: ControlState,
    speed_multiplier: f64,
    file_path: String,
}

impl ControlPanel {
    pub fn new(command_tx: Sender<StrategyCommand>, initial_file: String) -> Self {
        Self {
            command_tx,
            current_state: ControlState::Paused,
            speed_multiplier: 1.0,
            file_path: initial_file,
        }
    }

    pub fn update_state(&mut self, state: ControlState) {
        self.current_state = state;
    }

    pub fn update_speed(&mut self, speed: f64) {
        self.speed_multiplier = speed;
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
                
                if ui.add_enabled(can_start, egui::Button::new("▶ Start")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::Start);
                }
                
                if ui.add_enabled(can_pause, egui::Button::new("⏸ Pause")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::Pause);
                }
                
                if ui.add_enabled(can_stop, egui::Button::new("⏹ Stop")).clicked() {
                    let _ = self.command_tx.send(StrategyCommand::Stop);
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
            
            // File info
            ui.horizontal(|ui| {
                ui.label("📁 Data File:");
                ui.label(
                    egui::RichText::new(&self.file_path)
                        .small()
                        .monospace()
                );
            });
        });
    }
}
