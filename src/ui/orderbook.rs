use eframe::egui;
use egui_plot::{Plot, PlotPoints, Line, Legend, Corner, VLine};
use super::data::PerformanceData;
use crate::config::PRICE_DECIMAL_PLACES;

pub struct OrderbookView {
    depth_levels: usize,
}

impl OrderbookView {
    pub fn new(depth_levels: usize) -> Self {
        Self { depth_levels }
    }

    pub fn set_depth_levels(&mut self, levels: usize) {
        self.depth_levels = levels;
    }

    pub fn depth_levels(&self) -> usize {
        self.depth_levels
    }

    pub fn render(&self, ui: &mut egui::Ui, data: Option<&PerformanceData>) {
        ui.heading("📖 Order Book (Real-time)");
        
        if let Some(data) = data {
            let depth = self.depth_levels.min(data.asks.len().min(data.bids.len()));
            let orderbook_height = 250.0;
            
            ui.columns(2, |columns| {
                // Left column: Order Book Table
                columns[0].vertical(|ui| {
                    ui.label(egui::RichText::new("📋 Order Book").strong().size(13.0));
                    egui::Frame::none().show(ui, |ui| {
                        ui.set_min_height(orderbook_height - 20.0);
                        ui.set_max_height(orderbook_height - 20.0);
                        
                        egui::ScrollArea::vertical()
                            .max_height(orderbook_height - 40.0)
                            .show(ui, |ui| {
                                egui::Grid::new("orderbook_grid")
                                    .striped(true)
                                    .spacing([8.0, 3.0])
                                    .show(ui, |ui| {
                                        ui.label(egui::RichText::new("Bid Qty").strong().size(11.0));
                                        ui.label(egui::RichText::new("Bid").strong().size(11.0));
                                        ui.label(egui::RichText::new("Ask").strong().size(11.0));
                                        ui.label(egui::RichText::new("Ask Qty").strong().size(11.0));
                                        ui.end_row();
                                        
                                        for i in 0..depth {
                                            if i < data.bids.len() {
                                                let bid = &data.bids[i];
                                                ui.label(egui::RichText::new(format!("{:.4}", bid.quantity))
                                                    .color(egui::Color32::from_rgb(100, 200, 100)).size(10.0));
                                                ui.label(egui::RichText::new(format!("{:.prec$}", bid.price, prec = PRICE_DECIMAL_PLACES))
                                                    .color(egui::Color32::from_rgb(100, 200, 100)).strong().size(10.0));
                                            } else {
                                                ui.label("-");
                                                ui.label("-");
                                            }
                                            
                                            if i < data.asks.len() {
                                                let ask = &data.asks[i];
                                                ui.label(egui::RichText::new(format!("{:.prec$}", ask.price, prec = PRICE_DECIMAL_PLACES))
                                                    .color(egui::Color32::from_rgb(255, 100, 100)).strong().size(10.0));
                                                ui.label(egui::RichText::new(format!("{:.4}", ask.quantity))
                                                    .color(egui::Color32::from_rgb(255, 100, 100)).size(10.0));
                                            } else {
                                                ui.label("-");
                                                ui.label("-");
                                            }
                                            ui.end_row();
                                        }
                                        
                                        if !data.bids.is_empty() && !data.asks.is_empty() {
                                            let spread = data.asks[0].price - data.bids[0].price;
                                            let spread_bps = (spread / data.mid_price) * 10000.0;
                                            
                                            ui.label("");
                                            ui.label(egui::RichText::new(format!("Spread: {:.prec$}", spread, prec = PRICE_DECIMAL_PLACES))
                                                .small().weak());
                                            ui.label(egui::RichText::new(format!("({:.2}bps)", spread_bps))
                                                .small().weak());
                                            ui.label("");
                                            ui.end_row();
                                        }
                                    });
                            });
                    });
                });
                
                // Right column: Depth Chart
                columns[1].vertical(|ui| {
                    ui.label(egui::RichText::new("📊 Depth Chart").strong().size(13.0));
                    self.render_depth_chart(ui, data, depth, orderbook_height - 20.0);
                });
            });
        } else {
            egui::Frame::none().show(ui, |ui| {
                ui.set_min_height(250.0);
                ui.set_max_height(250.0);
                ui.centered_and_justified(|ui| {
                    ui.label("Waiting for order book data...");
                });
            });
        }
    }
    
    fn render_depth_chart(&self, ui: &mut egui::Ui, data: &PerformanceData, depth: usize, height: f32) {
        if data.bids.is_empty() || data.asks.is_empty() {
            ui.add_sized([ui.available_width(), height], egui::Label::new("No depth data"));
            return;
        }
        
        let mid_price = data.mid_price;
        
        // Calculate cumulative quantities for bids (sorted by price descending, so reverse for cumulative)
        let mut bid_cumulative: Vec<[f64; 2]> = Vec::new();
        let mut cumulative_qty = 0.0;
        
        // Build bid depth from mid price going left (lower prices)
        // Bids are typically sorted highest to lowest, so we iterate and accumulate
        for i in 0..depth.min(data.bids.len()) {
            let bid = &data.bids[i];
            cumulative_qty += bid.quantity;
            bid_cumulative.push([bid.price, cumulative_qty]);
        }
        // Reverse to have ascending price order for proper line drawing
        bid_cumulative.reverse();
        
        // Add starting point at mid price with 0 cumulative
        let mut bid_points: Vec<[f64; 2]> = vec![[mid_price, 0.0]];
        // Add step-like points for bids (going from mid price to lower prices)
        for i in 0..bid_cumulative.len() {
            let [price, qty] = bid_cumulative[i];
            // Add horizontal line to this price level
            if i == 0 {
                bid_points.push([price, 0.0]);
            }
            bid_points.push([price, qty]);
            // Add vertical step
            if i + 1 < bid_cumulative.len() {
                bid_points.push([bid_cumulative[i + 1][0], qty]);
            }
        }
        
        // Calculate cumulative quantities for asks (sorted by price ascending)
        let mut ask_points: Vec<[f64; 2]> = vec![[mid_price, 0.0]];
        cumulative_qty = 0.0;
        
        for i in 0..depth.min(data.asks.len()) {
            let ask = &data.asks[i];
            // Add step-like points
            if i == 0 {
                ask_points.push([ask.price, 0.0]);
            }
            cumulative_qty += ask.quantity;
            ask_points.push([ask.price, cumulative_qty]);
            // Add horizontal step to next price
            if i + 1 < depth.min(data.asks.len()) {
                ask_points.push([data.asks[i + 1].price, cumulative_qty]);
            }
        }
        
        let bid_line: PlotPoints = bid_points.into_iter().collect();
        let ask_line: PlotPoints = ask_points.into_iter().collect();
        
        let chart_width = ui.available_width();
        
        Plot::new("depth_chart")
            .legend(Legend::default().position(Corner::RightTop))
            .height(height)
            .width(chart_width)
            .show_axes([true, true])
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .show(ui, |plot_ui| {
                // Draw bid depth (green, filled area effect with line)
                plot_ui.line(
                    Line::new(bid_line)
                        .color(egui::Color32::from_rgb(100, 200, 100))
                        .name("Bids")
                        .width(2.0)
                        .fill(0.0)
                );
                
                // Draw ask depth (red, filled area effect with line)
                plot_ui.line(
                    Line::new(ask_line)
                        .color(egui::Color32::from_rgb(255, 100, 100))
                        .name("Asks")
                        .width(2.0)
                        .fill(0.0)
                );
                
                // Draw mid price vertical line
                plot_ui.vline(
                    VLine::new(mid_price)
                        .color(egui::Color32::from_rgb(255, 255, 100))
                        .style(egui_plot::LineStyle::Dashed { length: 8.0 })
                        .name(format!("Mid: {:.prec$}", mid_price, prec = PRICE_DECIMAL_PLACES))
                );
            });
    }
}
