use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints, Legend, Corner};
use crossbeam_channel::{Receiver, Sender};
use std::collections::VecDeque;
use crate::controller::{StrategyCommand, ControlResponse, ControlState};
use crate::config::PRICE_DECIMAL_PLACES;
use super::control_panel::ControlPanel;

/// Order book level data
#[derive(Debug, Clone)]
pub struct OrderBookLevel {
    pub price: f64,
    pub quantity: f64,
}

/// Performance data structure with extended metrics
#[derive(Debug, Clone)]
pub struct PerformanceData {
    pub timestamp: f64,
    pub equity: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub position: f64,
    pub mid_price: f64,
    pub strategy_name: String,
    // Extended metrics
    pub num_trades: usize,
    pub winning_trades: usize,
    pub total_fills: usize,
    pub total_orders: usize,
    pub position_hold_time: f64, // in seconds
    pub latency_micros: u64,
    // Order book data
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
}

/// GUI monitor application
pub struct PerformanceMonitor {
    data_receiver: Receiver<PerformanceData>,
    control_response_rx: Receiver<ControlResponse>,
    control_panel: ControlPanel,
    
    // Chart histories
    equity_history: VecDeque<(f64, f64)>,
    pnl_history: VecDeque<(f64, f64)>,
    position_history: VecDeque<(f64, f64)>,
    price_history: VecDeque<(f64, f64)>,
    
    // Extended metrics histories
    win_rate_history: VecDeque<(f64, f64)>,
    avg_profit_per_trade_history: VecDeque<(f64, f64)>,
    fill_ratio_history: VecDeque<(f64, f64)>,
    position_hold_time_history: VecDeque<(f64, f64)>,
    latency_history: VecDeque<(f64, f64)>,
    
    max_points: usize,
    current_data: Option<PerformanceData>,
    initial_equity: f64,
    show_settings: bool,
    orderbook_depth_levels: usize,
    data_updated: bool,
}

impl PerformanceMonitor {
    pub fn new(
        data_receiver: Receiver<PerformanceData>,
        control_response_rx: Receiver<ControlResponse>,
        command_tx: Sender<StrategyCommand>,
        initial_equity: f64,
        data_file: String,
    ) -> Self {
        Self {
            data_receiver,
            control_response_rx,
            control_panel: ControlPanel::new(command_tx, data_file),
            equity_history: VecDeque::new(),
            pnl_history: VecDeque::new(),
            position_history: VecDeque::new(),
            price_history: VecDeque::new(),
            win_rate_history: VecDeque::new(),
            avg_profit_per_trade_history: VecDeque::new(),
            fill_ratio_history: VecDeque::new(),
            position_hold_time_history: VecDeque::new(),
            latency_history: VecDeque::new(),
            max_points: 1000,
            current_data: None,
            initial_equity,
            show_settings: false,
            orderbook_depth_levels: 10,
            data_updated: false,
        }
    }

    fn update_data(&mut self) {
        self.data_updated = false;
        
        // Receive all pending data from channel
        while let Ok(data) = self.data_receiver.try_recv() {
            self.data_updated = true;
            let timestamp = data.timestamp;
            
            // Avoid processing zero/invalid data after backtest completion
            if data.equity == 0.0 && data.mid_price == 0.0 {
                continue;
            }
            
            // Update basic chart histories
            self.equity_history.push_back((timestamp, data.equity));
            self.pnl_history.push_back((timestamp, data.realized_pnl + data.unrealized_pnl));
            self.position_history.push_back((timestamp, data.position));
            self.price_history.push_back((timestamp, data.mid_price));
            
            // Update extended metrics
            let win_rate = if data.num_trades > 0 {
                (data.winning_trades as f64 / data.num_trades as f64) * 100.0
            } else {
                0.0
            };
            self.win_rate_history.push_back((timestamp, win_rate));
            
            let avg_profit = if data.num_trades > 0 {
                data.realized_pnl / data.num_trades as f64
            } else {
                0.0
            };
            self.avg_profit_per_trade_history.push_back((timestamp, avg_profit));
            
            let fill_ratio = if data.total_orders > 0 {
                (data.total_fills as f64 / data.total_orders as f64) * 100.0
            } else {
                0.0
            };
            self.fill_ratio_history.push_back((timestamp, fill_ratio));
            
            self.position_hold_time_history.push_back((timestamp, data.position_hold_time));
            self.latency_history.push_back((timestamp, data.latency_micros as f64));
            
            // Limit maximum number of points
            if self.equity_history.len() > self.max_points {
                self.equity_history.pop_front();
                self.pnl_history.pop_front();
                self.position_history.pop_front();
                self.price_history.pop_front();
                self.win_rate_history.pop_front();
                self.avg_profit_per_trade_history.pop_front();
                self.fill_ratio_history.pop_front();
                self.position_hold_time_history.pop_front();
                self.latency_history.pop_front();
            }
            
            self.current_data = Some(data);
        }
        
        // Process control responses
        while let Ok(response) = self.control_response_rx.try_recv() {
            match response {
                ControlResponse::StateChanged(state) => {
                    self.control_panel.update_state(state);
                }
                ControlResponse::SpeedChanged(speed) => {
                    self.control_panel.update_speed(speed);
                }
                ControlResponse::FilesChanged(files) => {
                    self.control_panel.update_files(files);
                    // Clear chart data when files change
                    self.clear_chart_data();
                }
                ControlResponse::Skipped => {
                    // File skipped, clear data for next file
                    self.clear_chart_data();
                }
                ControlResponse::Error(err) => {
                    eprintln!("Control error: {}", err);
                }
                ControlResponse::Completed => {
                    self.control_panel.update_state(ControlState::Completed);
                }
            }
        }
    }

    fn render_stats(&self, ui: &mut egui::Ui) {
        if let Some(data) = &self.current_data {
            let return_pct = ((data.equity - self.initial_equity) / self.initial_equity) * 100.0;
            let total_pnl = data.realized_pnl + data.unrealized_pnl;
            
            ui.heading(format!("📊 {} Strategy Monitor", data.strategy_name));
            ui.separator();
            
            // Main statistics
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label("💰 Equity");
                    ui.label(egui::RichText::new(format!("${:.2}", data.equity))
                        .size(24.0)
                        .strong());
                });
                
                ui.separator();
                
                ui.vertical(|ui| {
                    ui.label("📈 Return");
                    let color = if return_pct >= 0.0 { egui::Color32::GREEN } else { egui::Color32::RED };
                    ui.label(egui::RichText::new(format!("{:+.2}%", return_pct))
                        .size(24.0)
                        .color(color)
                        .strong());
                });
                
                ui.separator();
                
                ui.vertical(|ui| {
                    ui.label("💵 Total PnL");
                    let color = if total_pnl >= 0.0 { egui::Color32::GREEN } else { egui::Color32::RED };
                    ui.label(egui::RichText::new(format!("${:+.2}", total_pnl))
                        .size(24.0)
                        .color(color)
                        .strong());
                });
            });
            
            ui.separator();
            
            // Detailed statistics
            ui.horizontal(|ui| {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Realized PnL:");
                        let color = if data.realized_pnl >= 0.0 { egui::Color32::GREEN } else { egui::Color32::RED };
                        ui.label(egui::RichText::new(format!("${:+.2}", data.realized_pnl))
                            .color(color));
                    });
                });
                
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Unrealized PnL:");
                        let color = if data.unrealized_pnl >= 0.0 { egui::Color32::GREEN } else { egui::Color32::RED };
                        ui.label(egui::RichText::new(format!("${:+.2}", data.unrealized_pnl))
                            .color(color));
                    });
                });
                
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Position:");
                        ui.label(format!("{:.4}", data.position));
                    });
                });
                
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Mid Price:");
                        ui.label(format!("${:.2}", data.mid_price));
                    });
                });
            });
        } else {
            ui.heading("📊 Strategy Monitor");
            ui.separator();
            ui.label("Waiting for data...");
        }
    }

    fn clear_chart_data(&mut self) {
        self.equity_history.clear();
        self.pnl_history.clear();
        self.position_history.clear();
        self.price_history.clear();
        self.win_rate_history.clear();
        self.avg_profit_per_trade_history.clear();
        self.fill_ratio_history.clear();
        self.position_hold_time_history.clear();
        self.latency_history.clear();
        self.data_updated = true; // Trigger repaint after clearing
    }

    fn render_equity_chart(&self, ui: &mut egui::Ui) {
        ui.heading("Equity Curve");
        
        if self.equity_history.is_empty() {
            ui.label("No data available");
            return;
        }
        
        let points: PlotPoints = self.equity_history.iter()
            .map(|(t, v)| [*t, *v])
            .collect();
        
        Plot::new("equity_plot")
            .legend(Legend::default().position(Corner::LeftTop))
            .height(200.0)
            .width(ui.available_width())
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(points)
                        .color(egui::Color32::from_rgb(0, 150, 255))
                        .name("Equity")
                        .width(2.0)
                );
                
                // Initial capital baseline
                if !self.equity_history.is_empty() {
                    let start = self.equity_history.front().unwrap().0;
                    let end = self.equity_history.back().unwrap().0;
                    let baseline: PlotPoints = vec![
                        [start, self.initial_equity],
                        [end, self.initial_equity]
                    ].into();
                    plot_ui.line(
                        Line::new(baseline)
                            .color(egui::Color32::GRAY)
                            .name("Initial Capital")
                            .style(egui_plot::LineStyle::Dashed { length: 10.0 })
                    );
                }
            });
    }

    fn render_pnl_chart(&self, ui: &mut egui::Ui) {
        ui.heading("PnL");
        
        let points: PlotPoints = self.pnl_history.iter()
            .map(|(t, v)| [*t, *v])
            .collect();
        
        Plot::new("pnl_plot")
            .legend(Legend::default().position(Corner::LeftTop))
            .height(200.0)
            .width(ui.available_width())
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(points)
                        .color(egui::Color32::from_rgb(0, 200, 100))
                        .name("Total PnL")
                        .width(2.0)
                );
                
                // Zero line
                if !self.pnl_history.is_empty() {
                    let start = self.pnl_history.front().unwrap().0;
                    let end = self.pnl_history.back().unwrap().0;
                    let zero_line: PlotPoints = vec![
                        [start, 0.0],
                        [end, 0.0]
                    ].into();
                    plot_ui.line(
                        Line::new(zero_line)
                            .color(egui::Color32::GRAY)
                            .style(egui_plot::LineStyle::Dashed { length: 10.0 })
                    );
                }
            });
    }

    fn render_position_chart(&self, ui: &mut egui::Ui) {
        ui.heading("Position");
        
        let points: PlotPoints = self.position_history.iter()
            .map(|(t, v)| [*t, *v])
            .collect();
        
        Plot::new("position_plot")
            .legend(Legend::default().position(Corner::LeftTop))
            .height(200.0)
            .width(ui.available_width())
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(points)
                        .color(egui::Color32::from_rgb(255, 150, 0))
                        .name("Position")
                        .width(2.0)
                );
                
                // Zero line
                if !self.position_history.is_empty() {
                    let start = self.position_history.front().unwrap().0;
                    let end = self.position_history.back().unwrap().0;
                    let zero_line: PlotPoints = vec![
                        [start, 0.0],
                        [end, 0.0]
                    ].into();
                    plot_ui.line(
                        Line::new(zero_line)
                            .color(egui::Color32::GRAY)
                            .style(egui_plot::LineStyle::Dashed { length: 10.0 })
                    );
                }
            });
    }

    fn render_price_chart(&self, ui: &mut egui::Ui) {
        ui.heading("Mid Price");
        
        let points: PlotPoints = self.price_history.iter()
            .map(|(t, v)| [*t, *v])
            .collect();
        
        Plot::new("price_plot")
            .legend(Legend::default().position(Corner::LeftTop))
            .height(200.0)
            .width(ui.available_width())
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(points)
                        .color(egui::Color32::from_rgb(200, 100, 255))
                        .name("Mid Price")
                        .width(2.0)
                );
            });
    }

    fn render_settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("⚙️ Settings");
            if ui.button("❌ Close").clicked() {
                self.show_settings = false;
            }
        });
        ui.separator();
        
        ui.horizontal(|ui| {
            ui.label("Max Data Points:");
            ui.add(egui::Slider::new(&mut self.max_points, 100..=10000)
                .text("points")
                .logarithmic(true));
        });
        
        ui.label(format!("Current: {} points", self.equity_history.len()));
        
        ui.separator();
        
        ui.horizontal(|ui| {
            ui.label("Order Book Depth:");
            ui.add(egui::Slider::new(&mut self.orderbook_depth_levels, 5..=20)
                .text("levels"));
        });
        
        ui.separator();
        
        ui.horizontal(|ui| {
            if ui.button("🗑️ Clear All Data").clicked() {
                self.equity_history.clear();
                self.pnl_history.clear();
                self.position_history.clear();
                self.price_history.clear();
                self.win_rate_history.clear();
                self.avg_profit_per_trade_history.clear();
                self.fill_ratio_history.clear();
                self.position_hold_time_history.clear();
                self.latency_history.clear();
            }
            
            if ui.button("🔄 Reset to 1000").clicked() {
                self.max_points = 1000;
            }
        });
    }
    
    fn render_orderbook(&self, ui: &mut egui::Ui) {
        ui.heading("📖 Order Book (Real-time)");
        
        if let Some(data) = &self.current_data {
            let depth = self.orderbook_depth_levels.min(data.asks.len().min(data.bids.len()));
            
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    egui::Grid::new("orderbook_grid")
                        .striped(true)
                        .spacing([10.0, 4.0])
                        .show(ui, |ui| {
                            // Header
                            ui.label(egui::RichText::new("Bid Qty").strong());
                            ui.label(egui::RichText::new("Bid Price").strong());
                            ui.label(egui::RichText::new("Ask Price").strong());
                            ui.label(egui::RichText::new("Ask Qty").strong());
                            ui.end_row();
                            
                            // Display order book levels
                            for i in 0..depth {
                                let bid_idx = i;
                                let ask_idx = i;
                                
                                // Bid side
                                if bid_idx < data.bids.len() {
                                    let bid = &data.bids[bid_idx];
                                    ui.label(
                                        egui::RichText::new(format!("{:.4}", bid.quantity))
                                            .color(egui::Color32::from_rgb(100, 200, 100))
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("${:.prec$}", bid.price, prec = PRICE_DECIMAL_PLACES))
                                            .color(egui::Color32::from_rgb(100, 200, 100))
                                            .strong()
                                    );
                                } else {
                                    ui.label("-");
                                    ui.label("-");
                                }
                                
                                // Ask side
                                if ask_idx < data.asks.len() {
                                    let ask = &data.asks[ask_idx];
                                    ui.label(
                                        egui::RichText::new(format!("${:.prec$}", ask.price, prec = PRICE_DECIMAL_PLACES))
                                            .color(egui::Color32::from_rgb(255, 100, 100))
                                            .strong()
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("{:.4}", ask.quantity))
                                            .color(egui::Color32::from_rgb(255, 100, 100))
                                    );
                                } else {
                                    ui.label("-");
                                    ui.label("-");
                                }
                                
                                ui.end_row();
                            }
                            
                            // Show spread
                            if !data.bids.is_empty() && !data.asks.is_empty() {
                                let spread = data.asks[0].price - data.bids[0].price;
                                let spread_bps = (spread / data.mid_price) * 10000.0;
                                
                                ui.label("");
                                ui.label(
                                    egui::RichText::new(format!("Spread: ${:.prec$}", spread, prec = PRICE_DECIMAL_PLACES))
                                        .small()
                                        .weak()
                                );
                                ui.label(
                                    egui::RichText::new(format!("({:.2} bps)", spread_bps))
                                        .small()
                                        .weak()
                                );
                                ui.label("");
                                ui.end_row();
                            }
                        });
                });
        } else {
            ui.label("Waiting for order book data...");
        }
    }
    
    fn render_win_rate_chart(&self, ui: &mut egui::Ui) {
        ui.heading("Win Rate");
        
        let points: PlotPoints = self.win_rate_history.iter()
            .map(|(t, v)| [*t, *v])
            .collect();
        
        Plot::new("win_rate_plot")
            .legend(Legend::default().position(Corner::LeftTop))
            .height(200.0)
            .width(ui.available_width())
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(points)
                        .color(egui::Color32::from_rgb(100, 150, 255))
                        .name("Win Rate %")
                        .width(2.0)
                );
            });
    }
    
    fn render_avg_profit_chart(&self, ui: &mut egui::Ui) {
        ui.heading("Avg Profit per Trade");
        
        let points: PlotPoints = self.avg_profit_per_trade_history.iter()
            .map(|(t, v)| [*t, *v])
            .collect();
        
        Plot::new("avg_profit_plot")
            .legend(Legend::default().position(Corner::LeftTop))
            .height(200.0)
            .width(ui.available_width())
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(points)
                        .color(egui::Color32::from_rgb(255, 180, 100))
                        .name("Avg Profit $")
                        .width(2.0)
                );
                
                // Zero line
                if !self.avg_profit_per_trade_history.is_empty() {
                    let start = self.avg_profit_per_trade_history.front().unwrap().0;
                    let end = self.avg_profit_per_trade_history.back().unwrap().0;
                    let zero_line: PlotPoints = vec![
                        [start, 0.0],
                        [end, 0.0]
                    ].into();
                    plot_ui.line(
                        Line::new(zero_line)
                            .color(egui::Color32::GRAY)
                            .style(egui_plot::LineStyle::Dashed { length: 10.0 })
                    );
                }
            });
    }
    
    fn render_fill_ratio_chart(&self, ui: &mut egui::Ui) {
        ui.heading("Order Fill Ratio");
        
        let points: PlotPoints = self.fill_ratio_history.iter()
            .map(|(t, v)| [*t, *v])
            .collect();
        
        Plot::new("fill_ratio_plot")
            .legend(Legend::default().position(Corner::LeftTop))
            .height(200.0)
            .width(ui.available_width())
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(points)
                        .color(egui::Color32::from_rgb(150, 100, 255))
                        .name("Fill Ratio %")
                        .width(2.0)
                );
            });
    }
    
    fn render_position_hold_time_chart(&self, ui: &mut egui::Ui) {
        ui.heading("Avg Position Hold Time");
        
        let points: PlotPoints = self.position_hold_time_history.iter()
            .map(|(t, v)| [*t, *v])
            .collect();
        
        Plot::new("position_hold_time_plot")
            .legend(Legend::default().position(Corner::LeftTop))
            .height(200.0)
            .width(ui.available_width())
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(points)
                        .color(egui::Color32::from_rgb(255, 150, 200))
                        .name("Hold Time (s)")
                        .width(2.0)
                );
            });
    }
    
    fn render_latency_chart(&self, ui: &mut egui::Ui) {
        ui.heading("Latency");
        
        let points: PlotPoints = self.latency_history.iter()
            .map(|(t, v)| [*t, *v])
            .collect();
        
        Plot::new("latency_plot")
            .legend(Legend::default().position(Corner::LeftTop))
            .height(200.0)
            .width(ui.available_width())
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(points)
                        .color(egui::Color32::from_rgb(200, 100, 150))
                        .name("Latency (μs)")
                        .width(2.0)
                );
            });
    }
}

impl eframe::App for PerformanceMonitor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update data
        self.update_data();
        
        // Only request repaint if data was updated or UI interaction is needed
        if self.data_updated {
            ctx.request_repaint();
        }
        
        // Top panel - Title and settings button
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("📊 HFT Backtest Monitor");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⚙️ Settings").clicked() {
                        self.show_settings = !self.show_settings;
                    }
                });
            });
        });
        
        // Right side panel - Control panel (like settings panel)
        egui::SidePanel::right("control_panel")
            .default_width(320.0)
            .min_width(280.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.control_panel.render(ui);
                });
            });
        
        // Settings panel (another right panel, shown conditionally)
        if self.show_settings {
            egui::SidePanel::right("settings_panel")
                .default_width(300.0)
                .show(ctx, |ui| {
                    self.render_settings_panel(ui);
                });
        }
        
        // Central panel with charts and orderbook
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Add horizontal margin to charts
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space(100.0); // Left margin
                    ui.vertical(|ui| {
                        // Order book at the top
                        self.render_orderbook(ui);
                        
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(10.0);
                        
                        // Statistics panel
                        self.render_stats(ui);
                        
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(10.0);
                        
                        // Charts in grid layout
                        ui.heading("📈 Performance Charts");
                        ui.add_space(5.0);
                        
                        // Row 1: Equity and PnL
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                self.render_equity_chart(ui);
                            });
                            ui.add_space(20.0);
                            ui.vertical(|ui| {
                                self.render_pnl_chart(ui);
                            });
                        });
                        
                        ui.add_space(15.0);
                        
                        // Row 2: Win Rate and Avg Profit per Trade
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                self.render_win_rate_chart(ui);
                            });
                            ui.add_space(20.0);
                            ui.vertical(|ui| {
                                self.render_avg_profit_chart(ui);
                            });
                        });
                        
                        ui.add_space(15.0);
                        
                        // Row 3: Fill Ratio and Position Hold Time
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                self.render_fill_ratio_chart(ui);
                            });
                            ui.add_space(20.0);
                            ui.vertical(|ui| {
                                self.render_position_hold_time_chart(ui);
                            });
                        });
                        
                        ui.add_space(15.0);
                        
                        // Row 4: Latency and Position
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                self.render_latency_chart(ui);
                            });
                            ui.add_space(20.0);
                            ui.vertical(|ui| {
                                self.render_position_chart(ui);
                            });
                        });
                        
                        ui.add_space(15.0);
                        
                        // Row 5: Price
                        self.render_price_chart(ui);
                    });
                    ui.add_space(100.0); // Right margin
                });
            });
        });
    }
}

/// Launch monitor window function
pub fn launch_monitor(
    data_receiver: Receiver<PerformanceData>,
    control_response_rx: Receiver<ControlResponse>,
    command_tx: Sender<StrategyCommand>,
    initial_equity: f64,
    strategy_name: &str,
    data_file: String,
) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 900.0])
            .with_title(format!("HFT Backtest Monitor - {}", strategy_name)),
        ..Default::default()
    };
    
    eframe::run_native(
        "HFT Backtest Monitor",
        options,
        Box::new(|_cc| Ok(Box::new(PerformanceMonitor::new(
            data_receiver,
            control_response_rx,
            command_tx,
            initial_equity,
            data_file,
        )))),
    ).map_err(|e| anyhow::anyhow!("Failed to run GUI: {}", e))
}
