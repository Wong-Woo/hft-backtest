pub struct SpreadCalculator {
    gamma: f64,
}

impl SpreadCalculator {
    pub fn new(gamma: f64) -> Self {
        Self { gamma }
    }

    pub fn calculate_reservation_price(
        &self,
        mid_price: f64,
        inventory: f64,
        volatility: f64,
    ) -> f64 {
        mid_price - inventory * self.gamma * volatility.powi(2)
    }
}
