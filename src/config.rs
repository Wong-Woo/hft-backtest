// Data configuration
pub const DATA_FILE_PATH: &str = r"D:\quant-data\1000PEPEUSDT\1000PEPEUSDT_20240626.npz";
pub const TICK_SIZE: f64 = 0.00001;
pub const LOT_SIZE: f64 = 0.001;

// Market making parameters
pub const GAMMA: f64 = 0.001;
pub const INITIAL_KAPPA: f64 = 0.1;
pub const MAX_INVENTORY: f64 = 5.0;
pub const VOLATILITY_THRESHOLD: f64 = 5.0;
pub const ORDER_SIZE: f64 = 0.01;
pub const DEPTH_LEVELS: usize = 20;
pub const ORDER_LAYERS: usize = 2;
pub const FIXED_SPREAD_TICKS: f64 = 10.0;
pub const INITIAL_CAPITAL: f64 = 10000.0;

// Momentum strategy parameters
pub const MOMENTUM_LOOKBACK_PERIOD: usize = 100;  // 모멘텀 계산 기간
pub const MOMENTUM_THRESHOLD: f64 = 0.002;        // 신호 발생 임계값 (0.2%)
pub const MOMENTUM_POSITION_SIZE: f64 = 0.05;     // 포지션 크기
pub const MOMENTUM_STOP_LOSS_PCT: f64 = 0.01;     // 손절 퍼센트 (1%)
pub const MOMENTUM_TAKE_PROFIT_PCT: f64 = 0.02;   // 익절 퍼센트 (2%)

// Backtest execution parameters
pub const ELAPSE_DURATION_NS: i64 = 100_000_000;  // 100ms in nanoseconds
pub const UPDATE_INTERVAL: usize = 10;             // Update every 10 ticks
pub const PRINT_INTERVAL: usize = 100;             // Print status every 100 updates
