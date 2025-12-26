use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SignalType {
    Long,
    Short,
    Neutral,
}

/// Momentum indicator calculator
pub struct MomentumIndicator {
    lookback_period: usize,
    price_history: VecDeque<f64>,
    returns_history: VecDeque<f64>,
    momentum_threshold: f64,
}

impl MomentumIndicator {
    pub fn new(lookback_period: usize, momentum_threshold: f64) -> Self {
        Self {
            lookback_period,
            price_history: VecDeque::with_capacity(lookback_period + 1),
            returns_history: VecDeque::with_capacity(lookback_period),
            momentum_threshold,
        }
    }

    /// Update with new price
    pub fn update(&mut self, price: f64) {
        self.price_history.push_back(price);
        
        if self.price_history.len() > self.lookback_period + 1 {
            self.price_history.pop_front();
        }

        // Calculate returns
        if self.price_history.len() >= 2 {
            let prev_price = self.price_history[self.price_history.len() - 2];
            let current_price = self.price_history[self.price_history.len() - 1];
            let returns = (current_price - prev_price) / prev_price;
            
            self.returns_history.push_back(returns);
            
            if self.returns_history.len() > self.lookback_period {
                self.returns_history.pop_front();
            }
        }
    }

    /// Calculate momentum value (cumulative return)
    pub fn calculate_momentum(&self) -> Option<f64> {
        if self.price_history.len() < 2 {
            return None;
        }

        let first_price = self.price_history[0];
        let last_price = *self.price_history.back().unwrap();
        
        Some((last_price - first_price) / first_price)
    }

    /// Calculate average return
    #[allow(dead_code)]
    pub fn calculate_average_return(&self) -> Option<f64> {
        if self.returns_history.is_empty() {
            return None;
        }

        let sum: f64 = self.returns_history.iter().sum();
        Some(sum / self.returns_history.len() as f64)
    }

    /// Generate momentum signal
    pub fn generate_signal(&self) -> SignalType {
        let momentum = match self.calculate_momentum() {
            Some(m) => m,
            None => return SignalType::Neutral,
        };

        if momentum > self.momentum_threshold {
            SignalType::Long
        } else if momentum < -self.momentum_threshold {
            SignalType::Short
        } else {
            SignalType::Neutral
        }
    }

    /// Check if indicator is ready
    pub fn is_ready(&self) -> bool {
        self.price_history.len() >= self.lookback_period
    }

    /// Get current momentum value
    pub fn get_momentum(&self) -> f64 {
        self.calculate_momentum().unwrap_or(0.0)
    }

    /// Calculate price volatility (standard deviation)
    #[allow(dead_code)]
    pub fn calculate_volatility(&self) -> Option<f64> {
        if self.returns_history.len() < 2 {
            return None;
        }

        let mean = self.calculate_average_return()?;
        let variance: f64 = self.returns_history
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / self.returns_history.len() as f64;
        
        Some(variance.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_momentum_calculation() {
        let mut indicator = MomentumIndicator::new(5, 0.01);
        
        let prices = vec![100.0, 101.0, 102.0, 103.0, 104.0, 105.0];
        
        for price in prices {
            indicator.update(price);
        }

        let momentum = indicator.calculate_momentum().unwrap();
        assert!((momentum - 0.05).abs() < 0.0001); // (105-100)/100 = 0.05
    }

    #[test]
    fn test_signal_generation() {
        let mut indicator = MomentumIndicator::new(5, 0.01);
        
        // Upward trend
        for i in 0..6 {
            indicator.update(100.0 + i as f64 * 2.0);
        }

        let signal = indicator.generate_signal();
        assert_eq!(signal, SignalType::Long);
    }
}
