use eframe::egui;
use super::data::PerformanceData;

pub struct StatsPanel;

impl StatsPanel {
    /// Format elapsed time in human-readable format (T+XXm XXs or T+XXs)
    fn format_elapsed_time(seconds: f64) -> String {
        if seconds >= 3600.0 {
            let hours = (seconds / 3600.0).floor() as u64;
            let mins = ((seconds % 3600.0) / 60.0).floor() as u64;
            let secs = (seconds % 60.0).floor() as u64;
            format!("T+{}h {:02}m {:02}s", hours, mins, secs)
        } else if seconds >= 60.0 {
            let mins = (seconds / 60.0).floor() as u64;
            let secs = (seconds % 60.0).floor() as u64;
            format!("T+{}m {:02}s", mins, secs)
        } else {
            format!("T+{:.1}s", seconds)
        }
    }

    pub fn render(ui: &mut egui::Ui, data: Option<&PerformanceData>, initial_equity: f64) {
        ui.group(|ui| {
            if let Some(data) = data {
                let return_pct = ((data.equity - initial_equity) / initial_equity) * 100.0;
                let total_pnl = data.realized_pnl + data.unrealized_pnl;
                
                ui.horizontal(|ui| {
                    ui.heading(format!("📊 {} Monitor", data.strategy_name));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(Self::format_elapsed_time(data.timestamp))
                            .size(14.0)
                            .color(egui::Color32::LIGHT_GRAY)
                            .monospace());
                    });
                });
                ui.separator();
                
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("💰 Equity:");
                        ui.label(egui::RichText::new(format!("${:.2}", data.equity))
                            .size(18.0).strong());
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("📈 Return:");
                        let color = if return_pct >= 0.0 { egui::Color32::GREEN } else { egui::Color32::RED };
                        ui.label(egui::RichText::new(format!("{:+.2}%", return_pct))
                            .size(18.0).color(color).strong());
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("💵 Total PnL:");
                        let color = if total_pnl >= 0.0 { egui::Color32::GREEN } else { egui::Color32::RED };
                        ui.label(egui::RichText::new(format!("${:+.2}", total_pnl))
                            .size(18.0).color(color).strong());
                    });
                });
                
                ui.separator();
                
                egui::Grid::new("stats_grid")
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        Self::render_stat_row(ui, "Realized PnL:", data.realized_pnl, true);
                        Self::render_stat_row(ui, "Unrealized PnL:", data.unrealized_pnl, true);
                        
                        ui.label("Position:");
                        ui.label(format!("{:.4}", data.position));
                        ui.end_row();
                        
                        ui.label("Mid Price:");
                        ui.label(format!("${:.2}", data.mid_price));
                        ui.end_row();
                        
                        ui.label("Trades:");
                        ui.label(format!("{}", data.num_trades));
                        ui.end_row();
                        
                        ui.label("Win Rate:");
                        let win_rate = if data.num_trades > 0 {
                            (data.winning_trades as f64 / data.num_trades as f64) * 100.0
                        } else { 0.0 };
                        ui.label(format!("{:.1}%", win_rate));
                        ui.end_row();
                        
                        ui.label("Fill Ratio:");
                        let fill_ratio = if data.total_orders > 0 {
                            (data.total_fills as f64 / data.total_orders as f64) * 100.0
                        } else { 0.0 };
                        ui.label(format!("{:.1}%", fill_ratio));
                        ui.end_row();
                    });
            } else {
                ui.heading("📊 Strategy Monitor");
                ui.separator();
                ui.label("Waiting for data...");
            }
        });
    }

    fn render_stat_row(ui: &mut egui::Ui, label: &str, value: f64, is_pnl: bool) {
        ui.label(label);
        if is_pnl {
            let color = if value >= 0.0 { egui::Color32::GREEN } else { egui::Color32::RED };
            ui.label(egui::RichText::new(format!("${:+.2}", value)).color(color));
        } else {
            ui.label(format!("{:.4}", value));
        }
        ui.end_row();
    }
}
