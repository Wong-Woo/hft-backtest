use std::env;

// =============================================================================
// Data Configuration
// =============================================================================

/// Default data file path - can be overridden by DATA_FILE_PATH env var
const DEFAULT_DATA_FILE_PATH: &str = r"D:\quant-data\1000PEPEUSDT\1000PEPEUSDT_20240626.npz";

pub fn get_data_file_path() -> String {
    env::var("DATA_FILE_PATH").unwrap_or_else(|_| DEFAULT_DATA_FILE_PATH.to_string())
}

pub const TICK_SIZE: f64 = 0.00001;
pub const LOT_SIZE: f64 = 0.001;

/// Calculate decimal places needed for TICK_SIZE at compile time
/// This determines the price precision display (e.g., 0.00001 -> 5 decimal places)
pub const PRICE_DECIMAL_PLACES: usize = calculate_decimal_places(TICK_SIZE);

/// Calculate the number of decimal places needed for a given tick size
const fn calculate_decimal_places(tick_size: f64) -> usize {
    // Convert tick size to determine precision
    // 0.00001 -> 5, 0.0001 -> 4, 0.001 -> 3, 0.01 -> 2, 0.1 -> 1, 1.0 -> 0
    
    // For common tick sizes, use a simple lookup
    // This is a compile-time constant evaluation
    if (tick_size - 0.00001).abs() < 1e-10 { 5 }
    else if (tick_size - 0.0001).abs() < 1e-9 { 4 }
    else if (tick_size - 0.001).abs() < 1e-8 { 3 }
    else if (tick_size - 0.01).abs() < 1e-7 { 2 }
    else if (tick_size - 0.1).abs() < 1e-6 { 1 }
    else if (tick_size - 1.0).abs() < 1e-5 { 0 }
    else {
        // Fallback: calculate from the value
        // Count decimal places by checking order of magnitude
        let mut count = 0;
        let mut value = tick_size;
        while value < 1.0 && count < 10 {
            value *= 10.0;
            count += 1;
        }
        count
    }
}

// =============================================================================
// Backtest Execution Parameters
// =============================================================================

/// Time duration to elapse per iteration (100ms in nanoseconds)
pub const ELAPSE_DURATION_NS: i64 = 100_000_000;

/// Update strategy every N ticks
pub const UPDATE_INTERVAL: usize = 10;

/// Print status every N updates
pub const PRINT_INTERVAL: usize = 100;

/// Command polling timeout in microseconds
pub const COMMAND_POLL_TIMEOUT_MICROS: u64 = 1;

// =============================================================================
// Trading Parameters
// =============================================================================

/// Initial capital for backtesting
pub const INITIAL_CAPITAL: f64 = 10000.0;

// =============================================================================
// Market Making Strategy Configuration
// =============================================================================

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MarketMakerConfig {
    /// Inventory risk aversion parameter
    pub gamma: f64,
    /// Initial spread adjustment parameter (unused currently)
    pub initial_kappa: f64,
    /// Maximum allowed inventory position
    pub max_inventory: f64,
    /// Volatility threshold for risk management
    pub volatility_threshold: f64,
    /// Order size per layer
    pub order_size: f64,
    /// Number of order book depth levels to analyze
    pub depth_levels: usize,
    /// Number of order layers to place on each side
    pub order_layers: usize,
    /// Fixed spread in ticks
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

// Legacy constants for backward compatibility
pub const GAMMA: f64 = 0.001;
pub const INITIAL_KAPPA: f64 = 0.1;
pub const MAX_INVENTORY: f64 = 5.0;
pub const VOLATILITY_THRESHOLD: f64 = 5.0;
pub const ORDER_SIZE: f64 = 0.01;
pub const DEPTH_LEVELS: usize = 20;
pub const ORDER_LAYERS: usize = 2;
pub const FIXED_SPREAD_TICKS: f64 = 10.0;

// =============================================================================
// Momentum Strategy Configuration
// =============================================================================

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MomentumConfig {
    /// Lookback period for momentum calculation
    pub lookback_period: usize,
    /// Signal generation threshold (e.g., 0.002 = 0.2%)
    pub momentum_threshold: f64,
    /// Position size as fraction of capital
    pub position_size: f64,
    /// Stop loss percentage (e.g., 0.01 = 1%)
    pub stop_loss_pct: f64,
    /// Take profit percentage (e.g., 0.02 = 2%)
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

// Legacy constants for backward compatibility
pub const MOMENTUM_LOOKBACK_PERIOD: usize = 100;
pub const MOMENTUM_THRESHOLD: f64 = 0.002;
pub const MOMENTUM_POSITION_SIZE: f64 = 0.05;
pub const MOMENTUM_STOP_LOSS_PCT: f64 = 0.01;
pub const MOMENTUM_TAKE_PROFIT_PCT: f64 = 0.02;
