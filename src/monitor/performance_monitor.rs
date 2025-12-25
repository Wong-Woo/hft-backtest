use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints, Legend, Corner};
use crossbeam_channel::Receiver;
use std::collections::VecDeque;

/// 성능 데이터 구조체
#[derive(Debug, Clone)]
pub struct PerformanceData {
    pub timestamp: f64,
    pub equity: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub position: f64,
    pub mid_price: f64,
    pub strategy_name: String,
}

/// GUI 모니터 애플리케이션
pub struct PerformanceMonitor {
    receiver: Receiver<PerformanceData>,
    equity_history: VecDeque<(f64, f64)>,
    pnl_history: VecDeque<(f64, f64)>,
    position_history: VecDeque<(f64, f64)>,
    price_history: VecDeque<(f64, f64)>,
    max_points: usize,
    current_data: Option<PerformanceData>,
    initial_equity: f64,
    show_settings: bool,
}

impl PerformanceMonitor {
    pub fn new(receiver: Receiver<PerformanceData>, initial_equity: f64) -> Self {
        Self {
            receiver,
            equity_history: VecDeque::new(),
            pnl_history: VecDeque::new(),
            position_history: VecDeque::new(),
            price_history: VecDeque::new(),
            max_points: 1000,
            current_data: None,
            initial_equity,
            show_settings: false,
        }
    }

    fn update_data(&mut self) {
        // 채널에서 모든 대기 중인 데이터 수신
        while let Ok(data) = self.receiver.try_recv() {
            let timestamp = data.timestamp;
            
            // 데이터 히스토리 업데이트
            self.equity_history.push_back((timestamp, data.equity));
            self.pnl_history.push_back((timestamp, data.realized_pnl + data.unrealized_pnl));
            self.position_history.push_back((timestamp, data.position));
            self.price_history.push_back((timestamp, data.mid_price));
            
            // 최대 포인트 수 제한
            if self.equity_history.len() > self.max_points {
                self.equity_history.pop_front();
                self.pnl_history.pop_front();
                self.position_history.pop_front();
                self.price_history.pop_front();
            }
            
            self.current_data = Some(data);
        }
    }

    fn render_stats(&self, ui: &mut egui::Ui) {
        if let Some(data) = &self.current_data {
            let return_pct = ((data.equity - self.initial_equity) / self.initial_equity) * 100.0;
            let total_pnl = data.realized_pnl + data.unrealized_pnl;
            
            ui.heading(format!("📊 {} Strategy Monitor", data.strategy_name));
            ui.separator();
            
            // 메인 통계
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
            
            // 상세 통계
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

    fn render_equity_chart(&self, ui: &mut egui::Ui) {
        ui.heading("Equity Curve");
        
        let points: PlotPoints = self.equity_history.iter()
            .map(|(t, v)| [*t, *v])
            .collect();
        
        Plot::new("equity_plot")
            .legend(Legend::default().position(Corner::LeftTop))
            .height(200.0)
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(points)
                        .color(egui::Color32::from_rgb(0, 150, 255))
                        .name("Equity")
                        .width(2.0)
                );
                
                // 초기 자본 기준선
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
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(points)
                        .color(egui::Color32::from_rgb(0, 200, 100))
                        .name("Total PnL")
                        .width(2.0)
                );
                
                // 제로 라인
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
            .height(150.0)
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(points)
                        .color(egui::Color32::from_rgb(255, 150, 0))
                        .name("Position")
                        .width(2.0)
                );
                
                // 제로 라인
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
            .height(150.0)
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
            if ui.button("🗑️ Clear All Data").clicked() {
                self.equity_history.clear();
                self.pnl_history.clear();
                self.position_history.clear();
                self.price_history.clear();
            }
            
            if ui.button("🔄 Reset to 1000").clicked() {
                self.max_points = 1000;
            }
        });
    }
}

impl eframe::App for PerformanceMonitor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 데이터 업데이트
        self.update_data();
        
        // UI 지속적으로 갱신
        ctx.request_repaint();
        
        // 상단 패널 - 설정 버튼
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
        
        // 설정 패널 (사이드 패널)
        if self.show_settings {
            egui::SidePanel::right("settings_panel")
                .default_width(300.0)
                .show(ctx, |ui| {
                    self.render_settings_panel(ui);
                });
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // 통계 패널
                self.render_stats(ui);
                
                ui.separator();
                
                // Equity 차트
                self.render_equity_chart(ui);
                
                ui.separator();
                
                // PnL 차트
                self.render_pnl_chart(ui);
                
                ui.separator();
                
                // Position 차트
                self.render_position_chart(ui);
                
                ui.separator();
                
                // Price 차트
                self.render_price_chart(ui);
            });
        });
    }
}

/// 모니터 윈도우 실행 함수
pub fn launch_monitor(
    receiver: Receiver<PerformanceData>,
    initial_equity: f64,
    strategy_name: &str,
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
        Box::new(|_cc| Ok(Box::new(PerformanceMonitor::new(receiver, initial_equity)))),
    ).map_err(|e| anyhow::anyhow!("Failed to run GUI: {}", e))
}
