mod app;
mod charts;
mod control_panel;
mod data;
mod orderbook;
mod stats_panel;

pub use app::PerformanceMonitor;
pub use data::{PerformanceData, OrderBookLevel};

use crate::strategy::StrategyType;

pub fn launch_monitor_with_respawn(
    strategy_type: StrategyType,
    initial_equity: f64,
    data_file: String,
) -> anyhow::Result<()> {
    let strategy_name = strategy_type.name();
    
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 900.0])
            .with_title(format!("HFT Backtest Monitor - {}", strategy_name)),
        ..Default::default()
    };
    
    eframe::run_native(
        "HFT Backtest Monitor",
        options,
        Box::new(move |_cc| Ok(Box::new(PerformanceMonitor::new(
            strategy_type,
            initial_equity,
            data_file,
        )))),
    ).map_err(|e| anyhow::anyhow!("Failed to run GUI: {}", e))
}
