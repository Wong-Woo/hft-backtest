use eframe::egui;
use crossbeam_channel::{Sender, Receiver, unbounded};
use crate::controller::{StrategyCommand, ControlResponse, ControlState, StrategyController};
use crate::strategy::StrategyType;
use super::charts::{ChartHistory, ChartRenderer};
use super::control_panel::ControlPanel;
use super::data::PerformanceData;
use super::orderbook::OrderbookView;
use super::stats_panel::StatsPanel;

use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub struct PerformanceMonitor {
    data_receiver: Receiver<PerformanceData>,
    control_response_rx: Receiver<ControlResponse>,
    control_panel: ControlPanel,
    chart_history: ChartHistory,
    orderbook_view: OrderbookView,
    current_data: Option<PerformanceData>,
    initial_equity: f64,
    show_settings: bool,
    data_updated: bool,
    
    // Thread management
    strategy_type: StrategyType,
    strategy_thread: Option<JoinHandle<anyhow::Result<()>>>,
    controller: Option<Arc<StrategyController>>,
    #[allow(dead_code)]
    data_tx: Option<Sender<PerformanceData>>,
    cmd_tx: Sender<StrategyCommand>,
    #[allow(dead_code)]
    cmd_rx_holder: Option<Receiver<StrategyCommand>>,
    response_tx: Sender<ControlResponse>,
    can_start_new: bool,
}

impl PerformanceMonitor {
    pub fn new(strategy_type: StrategyType, initial_equity: f64, data_file: String) -> Self {
        let (data_tx, data_rx) = unbounded();
        let (cmd_tx, cmd_rx) = unbounded();
        let (response_tx, response_rx) = unbounded();
        
        Self {
            data_receiver: data_rx,
            control_response_rx: response_rx,
            control_panel: ControlPanel::new(cmd_tx.clone(), data_file),
            chart_history: ChartHistory::new(1000),
            orderbook_view: OrderbookView::new(10),
            current_data: None,
            initial_equity,
            show_settings: false,
            data_updated: false,
            strategy_type,
            strategy_thread: None,
            controller: None,
            data_tx: Some(data_tx),
            cmd_tx,
            cmd_rx_holder: Some(cmd_rx),
            response_tx,
            can_start_new: true,
        }
    }

    fn spawn_strategy_thread(&mut self) {
        let file_paths = self.control_panel.get_selected_files();
        if file_paths.is_empty() {
            eprintln!("No data file selected");
            return;
        }
        
        let (data_tx, data_rx) = unbounded();
        let (cmd_tx, cmd_rx) = unbounded();
        
        self.data_receiver = data_rx;
        self.cmd_rx_holder = None;
        self.control_panel.update_command_sender(cmd_tx.clone());
        self.cmd_tx = cmd_tx.clone();
        
        let controller = Arc::new(StrategyController::new(cmd_rx, self.response_tx.clone()));
        let controller_clone = Arc::clone(&controller);
        self.controller = Some(controller);
        
        let strategy_type = self.strategy_type.clone();
        
        let handle = thread::spawn(move || {
            strategy_type.run(file_paths, data_tx, controller_clone)
        });
        
        self.strategy_thread = Some(handle);
        self.can_start_new = false;
        self.chart_history.clear();
        
        // Send Start command immediately after spawning
        let _ = cmd_tx.send(StrategyCommand::Start);
    }

    fn check_thread_status(&mut self) {
        if let Some(handle) = self.strategy_thread.take() {
            if handle.is_finished() {
                let _ = handle.join();
                self.can_start_new = true;
                self.strategy_thread = None;
            } else {
                self.strategy_thread = Some(handle);
            }
        }
    }

    fn update_data(&mut self) {
        self.data_updated = false;
        
        while let Ok(data) = self.data_receiver.try_recv() {
            if data.equity == 0.0 && data.mid_price == 0.0 { continue; }
            self.data_updated = true;
            self.chart_history.push(&data);
            self.current_data = Some(data);
        }
        
        self.check_thread_status();
        self.control_panel.set_can_start_new(self.can_start_new);
        
        while let Ok(response) = self.control_response_rx.try_recv() {
            match response {
                ControlResponse::StateChanged(state) => self.control_panel.update_state(state),
                ControlResponse::SpeedChanged(speed) => self.control_panel.update_speed(speed),
                ControlResponse::FilesChanged(files) => {
                    self.control_panel.update_files(files);
                    self.chart_history.clear();
                }
                ControlResponse::Skipped => self.chart_history.clear(),
                ControlResponse::Error(err) => eprintln!("Control error: {}", err),
                ControlResponse::Completed => self.control_panel.update_state(ControlState::Completed),
                ControlResponse::ThreadTerminated => self.can_start_new = true,
            }
        }
        
        if self.control_panel.should_start_new_backtest() {
            self.spawn_strategy_thread();
        }
    }

    fn render_charts(&self, ui: &mut egui::Ui, chart_width: f32, content_width: f32) {
        ui.heading("📈 Performance Charts");
        ui.add_space(10.0);
        
        let chart_spacing = 15.0;
        
        ui.columns(2, |columns| {
            columns[0].vertical(|ui| {
                ChartRenderer::render_line_chart(ui, "equity_plot", "Equity Curve", 
                    &self.chart_history.equity, chart_width, 
                    egui::Color32::from_rgb(0, 150, 255), "Equity", false, Some(self.initial_equity));
            });
            columns[1].vertical(|ui| {
                ChartRenderer::render_line_chart(ui, "pnl_plot", "PnL", 
                    &self.chart_history.pnl, chart_width,
                    egui::Color32::from_rgb(0, 200, 100), "Total PnL", true, None);
            });
        });
        
        ui.add_space(chart_spacing);
        
        ui.columns(2, |columns| {
            columns[0].vertical(|ui| {
                ChartRenderer::render_line_chart(ui, "win_rate_plot", "Win Rate",
                    &self.chart_history.win_rate, chart_width,
                    egui::Color32::from_rgb(100, 150, 255), "Win Rate %", false, None);
            });
            columns[1].vertical(|ui| {
                ChartRenderer::render_line_chart(ui, "avg_profit_plot", "Avg Profit per Trade",
                    &self.chart_history.avg_profit, chart_width,
                    egui::Color32::from_rgb(255, 180, 100), "Avg Profit $", true, None);
            });
        });
        
        ui.add_space(chart_spacing);
        
        ui.columns(2, |columns| {
            columns[0].vertical(|ui| {
                ChartRenderer::render_line_chart(ui, "fill_ratio_plot", "Order Fill Ratio",
                    &self.chart_history.fill_ratio, chart_width,
                    egui::Color32::from_rgb(150, 100, 255), "Fill Ratio %", false, None);
            });
            columns[1].vertical(|ui| {
                ChartRenderer::render_line_chart(ui, "position_hold_time_plot", "Avg Position Hold Time",
                    &self.chart_history.position_hold_time, chart_width,
                    egui::Color32::from_rgb(255, 150, 200), "Hold Time (s)", false, None);
            });
        });
        
        ui.add_space(chart_spacing);
        
        ui.columns(2, |columns| {
            columns[0].vertical(|ui| {
                ChartRenderer::render_line_chart(ui, "latency_plot", "Latency",
                    &self.chart_history.latency, chart_width,
                    egui::Color32::from_rgb(200, 100, 150), "Latency (μs)", false, None);
            });
            columns[1].vertical(|ui| {
                ChartRenderer::render_line_chart(ui, "position_plot", "Position",
                    &self.chart_history.position, chart_width,
                    egui::Color32::from_rgb(255, 150, 0), "Position", true, None);
            });
        });
        
        ui.add_space(chart_spacing);
        
        ChartRenderer::render_line_chart(ui, "price_plot", "Mid Price",
            &self.chart_history.price, content_width,
            egui::Color32::from_rgb(200, 100, 255), "Mid Price", false, None);
    }

    fn render_settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.heading("⚙️ Display Settings");
            ui.separator();
            
            ui.horizontal(|ui| {
                ui.label("Max Data Points:");
                let mut max_points = self.chart_history.max_points();
                if ui.add(egui::Slider::new(&mut max_points, 100..=10000)
                    .text("points").logarithmic(true)).changed() {
                    self.chart_history.set_max_points(max_points);
                }
            });
            
            ui.label(format!("Current: {} points", self.chart_history.len()));
            ui.separator();
            
            ui.horizontal(|ui| {
                ui.label("Order Book Depth:");
                let mut depth = self.orderbook_view.depth_levels();
                if ui.add(egui::Slider::new(&mut depth, 5..=20).text("levels")).changed() {
                    self.orderbook_view.set_depth_levels(depth);
                }
            });
            
            ui.separator();
            
            ui.horizontal(|ui| {
                if ui.button("🗑️ Clear All Data").clicked() {
                    self.chart_history.clear();
                    self.data_updated = true;
                }
                if ui.button("🔄 Reset to 1000").clicked() {
                    self.chart_history.set_max_points(1000);
                }
            });
        });
    }
}

impl eframe::App for PerformanceMonitor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_data();
        
        if self.data_updated {
            ctx.request_repaint();
        }
        
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("📊 HFT Backtest Monitor");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn_text = if self.show_settings { "❌ Close Panel" } else { "⚙️ Control Panel" };
                    if ui.button(btn_text).clicked() {
                        self.show_settings = !self.show_settings;
                    }
                });
            });
        });
        
        if self.show_settings {
            egui::SidePanel::right("combined_panel")
                .default_width(340.0)
                .min_width(300.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        StatsPanel::render(ui, self.current_data.as_ref(), self.initial_equity);
                        ui.add_space(10.0);
                        self.control_panel.render(ui);
                        ui.add_space(10.0);
                        self.render_settings_panel(ui);
                    });
                });
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let available_width = ui.available_width();
                let margin = 20.0;
                let content_width = available_width - margin * 2.0;
                let chart_width = (content_width - 15.0) / 2.0;
                
                ui.add_space(10.0);
                
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(margin, 0.0))
                    .show(ui, |ui| {
                        self.orderbook_view.render(ui, self.current_data.as_ref());
                        ui.add_space(15.0);
                        ui.separator();
                        ui.add_space(15.0);
                        self.render_charts(ui, chart_width, content_width);
                        ui.add_space(20.0);
                    });
            });
        });
    }
}
