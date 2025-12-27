use eframe::egui;
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
            
            egui::Frame::none().show(ui, |ui| {
                ui.set_min_height(orderbook_height);
                ui.set_max_height(orderbook_height);
                
                egui::ScrollArea::vertical()
                    .max_height(orderbook_height - 20.0)
                    .show(ui, |ui| {
                        egui::Grid::new("orderbook_grid")
                            .striped(true)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("Bid Qty").strong());
                                ui.label(egui::RichText::new("Bid Price").strong());
                                ui.label(egui::RichText::new("Ask Price").strong());
                                ui.label(egui::RichText::new("Ask Qty").strong());
                                ui.end_row();
                                
                                for i in 0..depth {
                                    if i < data.bids.len() {
                                        let bid = &data.bids[i];
                                        ui.label(egui::RichText::new(format!("{:.4}", bid.quantity))
                                            .color(egui::Color32::from_rgb(100, 200, 100)));
                                        ui.label(egui::RichText::new(format!("${:.prec$}", bid.price, prec = PRICE_DECIMAL_PLACES))
                                            .color(egui::Color32::from_rgb(100, 200, 100)).strong());
                                    } else {
                                        ui.label("-");
                                        ui.label("-");
                                    }
                                    
                                    if i < data.asks.len() {
                                        let ask = &data.asks[i];
                                        ui.label(egui::RichText::new(format!("${:.prec$}", ask.price, prec = PRICE_DECIMAL_PLACES))
                                            .color(egui::Color32::from_rgb(255, 100, 100)).strong());
                                        ui.label(egui::RichText::new(format!("{:.4}", ask.quantity))
                                            .color(egui::Color32::from_rgb(255, 100, 100)));
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
                                    ui.label(egui::RichText::new(format!("Spread: ${:.prec$}", spread, prec = PRICE_DECIMAL_PLACES))
                                        .small().weak());
                                    ui.label(egui::RichText::new(format!("({:.2} bps)", spread_bps))
                                        .small().weak());
                                    ui.label("");
                                    ui.end_row();
                                }
                            });
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
}
