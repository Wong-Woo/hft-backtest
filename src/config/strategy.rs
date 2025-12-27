// Market Making Strategy Configuration
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MarketMakerConfig {
    pub gamma: f64,
    pub initial_kappa: f64,
    pub max_inventory: f64,
    pub volatility_threshold: f64,
    pub order_size: f64,
    pub depth_levels: usize,
    pub order_layers: usize,
    pub fixed_spread_ticks: f64,
}

impl Default for MarketMakerConfig {
    fn default() -> Self {
        Self {
            gamma: 0.001,
            initial_kappa: 0.1,
            max_inventory: 5.0,
            volatility_threshold: 5.0,
            order_size: 0.01,
            depth_levels: 20,
            order_layers: 2,
            fixed_spread_ticks: 10.0,
        }
    }
}

pub const GAMMA: f64 = 0.001;
pub const INITIAL_KAPPA: f64 = 0.1;
pub const MAX_INVENTORY: f64 = 5.0;
pub const VOLATILITY_THRESHOLD: f64 = 5.0;
pub const ORDER_SIZE: f64 = 0.01;
pub const DEPTH_LEVELS: usize = 20;
pub const ORDER_LAYERS: usize = 2;
pub const FIXED_SPREAD_TICKS: f64 = 10.0;

// Momentum Strategy Configuration
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MomentumConfig {
    pub lookback_period: usize,
    pub momentum_threshold: f64,
    pub position_size: f64,
    pub stop_loss_pct: f64,
    pub take_profit_pct: f64,
}

impl Default for MomentumConfig {
    fn default() -> Self {
        Self {
            lookback_period: 100,
            momentum_threshold: 0.002,
            position_size: 0.05,
            stop_loss_pct: 0.01,
            take_profit_pct: 0.02,
        }
    }
}

pub const MOMENTUM_LOOKBACK_PERIOD: usize = 100;
pub const MOMENTUM_THRESHOLD: f64 = 0.002;
pub const MOMENTUM_POSITION_SIZE: f64 = 0.05;
pub const MOMENTUM_STOP_LOSS_PCT: f64 = 0.01;
pub const MOMENTUM_TAKE_PROFIT_PCT: f64 = 0.02;

// ML Prediction Strategy Configuration
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PredictionConfig {
    pub position_size: f64,
    pub stop_loss_pct: f64,
    pub take_profit_pct: f64,
    pub min_prediction_confidence: f64,
    pub learning_rate: f64,
}

impl Default for PredictionConfig {
    fn default() -> Self {
        Self {
            position_size: 0.05,
            stop_loss_pct: 0.005,
            take_profit_pct: 0.01,
            min_prediction_confidence: 0.001,
            learning_rate: 0.001,
        }
    }
}

pub const PREDICTION_POSITION_SIZE: f64 = 0.05;
pub const PREDICTION_STOP_LOSS_PCT: f64 = 0.005;
pub const PREDICTION_TAKE_PROFIT_PCT: f64 = 0.01;
pub const PREDICTION_CONFIDENCE_THRESHOLD: f64 = 0.001;
pub const PREDICTION_LEARNING_RATE: f64 = 0.001;
