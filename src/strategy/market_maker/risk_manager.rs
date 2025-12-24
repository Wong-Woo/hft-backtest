use std::collections::VecDeque;

pub struct RiskManager {
    pub max_inventory: f64,
    volatility_threshold: f64,
    price_history: VecDeque<f64>,
    volatility_window: usize,
}

impl RiskManager {
    pub fn new(max_inventory: f64, volatility_threshold: f64, volatility_window: usize) -> Self {
        Self {
            max_inventory,
            volatility_threshold,
            price_history: VecDeque::with_capacity(volatility_window),
            volatility_window,
        }
    }

    pub fn is_position_safe(&self, inventory: f64) -> bool {
        inventory.abs() < self.max_inventory
    }

    pub fn update_price(&mut self, price: f64) {
        if self.price_history.len() >= self.volatility_window {
            self.price_history.pop_front();
        }
        self.price_history.push_back(price);
    }

    pub fn calculate_volatility(&self) -> f64 {
        if self.price_history.len() < 2 {
            return 0.0;
        }

        let mean = self.price_history.iter().sum::<f64>() / self.price_history.len() as f64;
        let variance = self.price_history
            .iter()
            .map(|&price| (price - mean).powi(2))
            .sum::<f64>() / self.price_history.len() as f64;
        
        variance.sqrt()
    }

    pub fn detect_toxic_flow(&self) -> bool {
        let volatility = self.calculate_volatility();
        volatility > self.volatility_threshold
    }

    pub fn adjust_order_size(&self, base_size: f64, inventory: f64) -> f64 {
        if self.max_inventory == 0.0 {
            return base_size;
        }

        let inventory_ratio = (inventory.abs() / self.max_inventory).min(1.0);
        base_size * (1.0 - inventory_ratio * 0.5)
    }
}
