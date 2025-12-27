#[derive(Debug, Clone)]
pub struct OrderBookLevel {
    pub price: f64,
    pub quantity: f64,
}

#[derive(Debug, Clone)]
pub struct PerformanceData {
    pub timestamp: f64,
    pub equity: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub position: f64,
    pub mid_price: f64,
    pub strategy_name: String,
    pub num_trades: usize,
    pub winning_trades: usize,
    pub total_fills: usize,
    pub total_orders: usize,
    pub position_hold_time: f64,
    pub latency_micros: u64,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
}
