use anyhow::Result;
use hftbacktest::{
    backtest::{Backtest, BacktestError},
    prelude::{HashMapMarketDepth, Bot},
    depth::MarketDepth,
};
use crate::ui::{PerformanceData, OrderBookLevel};

/// Core strategy state that all strategies share
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct StrategyState {
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub position: f64,
    pub entry_price: f64,
    pub mid_price: f64,
    pub update_count: u64,
    pub num_trades: usize,
    pub winning_trades: usize,
    pub total_orders: usize,
    pub total_fills: usize,
    pub avg_hold_time: f64,
}

impl StrategyState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn equity(&self, initial_capital: f64) -> f64 {
        initial_capital + self.realized_pnl + self.unrealized_pnl
    }

    pub fn win_rate(&self) -> f64 {
        if self.num_trades > 0 {
            self.winning_trades as f64 / self.num_trades as f64 * 100.0
        } else {
            0.0
        }
    }

    pub fn fill_ratio(&self) -> f64 {
        if self.total_orders > 0 {
            self.total_fills as f64 / self.total_orders as f64 * 100.0
        } else {
            0.0
        }
    }
}

/// Main trait that all strategies must implement
/// 
/// # Example
/// ```ignore
/// struct MyStrategy {
///     // Your strategy-specific fields
///     position_size: f64,
///     threshold: f64,
/// }
/// 
/// impl Strategy for MyStrategy {
///     fn name(&self) -> &str { "My Strategy" }
///     fn initial_capital(&self) -> f64 { 100_000.0 }
///     
///     fn on_tick(&mut self, ctx: &mut TickContext<'_>, state: &mut StrategyState) -> Result<(), BacktestError> {
///         // Your core strategy logic here
///         let mid_price = ctx.mid_price();
///         
///         if self.should_buy(mid_price) {
///             ctx.submit_buy_order(mid_price - 0.5, self.position_size)?;
///         }
///         
///         Ok(())
///     }
/// }
/// ```
pub trait Strategy: Send {
    /// Strategy name for display
    fn name(&self) -> &str;
    
    /// Initial capital
    fn initial_capital(&self) -> f64;
    
    /// Called on each tick with market data
    /// This is where your core strategy logic goes
    fn on_tick(
        &mut self,
        ctx: &mut TickContext<'_>,
        state: &mut StrategyState,
    ) -> Result<(), BacktestError>;
    
    /// Called at the start of each file (optional)
    fn on_file_start(&mut self, _file_path: &str) {
        // Default: do nothing
    }
    
    /// Called at the end of each file (optional)
    fn on_file_end(&mut self, _state: &StrategyState) {
        // Default: do nothing
    }
    
    /// Called when strategy is completed (optional)
    fn on_completed(&mut self, state: &StrategyState) {
        println!("\n=== {} Results ===", self.name());
        println!("Total PnL: ${:.2}", state.realized_pnl);
        println!("Trades: {} (Win rate: {:.1}%)", state.num_trades, state.win_rate());
    }
    
    /// How often to run strategy logic (in update counts)
    /// Default: every tick
    fn update_interval(&self) -> u64 {
        1
    }
    
    /// Order book depth levels to extract for GUI
    fn orderbook_depth(&self) -> usize {
        10
    }
}

/// Context passed to strategy on each tick
/// Provides convenient access to market data and order submission
#[allow(dead_code)]
pub struct TickContext<'a> {
    pub hbt: &'a mut Backtest<HashMapMarketDepth>,
    depth_cache: Option<DepthSnapshot>,
}

#[allow(dead_code)]
#[derive(Clone)]
struct DepthSnapshot {
    best_bid: f64,
    best_ask: f64,
    mid_price: f64,
    spread: f64,
}

impl<'a> TickContext<'a> {
    pub fn new(hbt: &'a mut Backtest<HashMapMarketDepth>) -> Self {
        Self {
            hbt,
            depth_cache: None,
        }
    }

    fn ensure_depth_cache(&mut self) {
        if self.depth_cache.is_none() {
            let depth = self.hbt.depth(0);
            let tick_size = depth.tick_size();
            let best_bid = depth.best_bid_tick() as f64 * tick_size;
            let best_ask = depth.best_ask_tick() as f64 * tick_size;
            self.depth_cache = Some(DepthSnapshot {
                best_bid,
                best_ask,
                mid_price: (best_bid + best_ask) / 2.0,
                spread: best_ask - best_bid,
            });
        }
    }

    /// Get current mid price
    pub fn mid_price(&mut self) -> f64 {
        self.ensure_depth_cache();
        self.depth_cache.as_ref().unwrap().mid_price
    }

    /// Get best bid price
    pub fn best_bid(&mut self) -> f64 {
        self.ensure_depth_cache();
        self.depth_cache.as_ref().unwrap().best_bid
    }

    /// Get best ask price
    pub fn best_ask(&mut self) -> f64 {
        self.ensure_depth_cache();
        self.depth_cache.as_ref().unwrap().best_ask
    }

    /// Get bid-ask spread
    pub fn spread(&mut self) -> f64 {
        self.ensure_depth_cache();
        self.depth_cache.as_ref().unwrap().spread
    }

    /// Get raw market depth reference
    pub fn depth(&self) -> &HashMapMarketDepth {
        self.hbt.depth(0)
    }

    /// Get bid quantity at price level (0 = best bid)
    pub fn bid_qty(&self, level: usize) -> f64 {
        let depth = self.hbt.depth(0);
        let tick = depth.best_bid_tick() - level as i64;
        depth.bid_qty_at_tick(tick)
    }

    /// Get ask quantity at price level (0 = best ask)
    pub fn ask_qty(&self, level: usize) -> f64 {
        let depth = self.hbt.depth(0);
        let tick = depth.best_ask_tick() + level as i64;
        depth.ask_qty_at_tick(tick)
    }

    /// Get current timestamp in nanoseconds
    pub fn timestamp_ns(&self) -> i64 {
        self.hbt.current_timestamp()
    }

    /// Submit a buy limit order
    pub fn submit_buy_order(
        &mut self,
        price: f64,
        qty: f64,
        order_id: u64,
    ) -> Result<(), BacktestError> {
        use hftbacktest::prelude::TimeInForce;
        use hftbacktest::types::OrdType;
        self.hbt.submit_buy_order(
            0, order_id, price, qty,
            TimeInForce::GTC, OrdType::Limit, false
        )?;
        Ok(())
    }

    /// Submit a sell limit order
    pub fn submit_sell_order(
        &mut self,
        price: f64,
        qty: f64,
        order_id: u64,
    ) -> Result<(), BacktestError> {
        use hftbacktest::prelude::TimeInForce;
        use hftbacktest::types::OrdType;
        self.hbt.submit_sell_order(
            0, order_id, price, qty,
            TimeInForce::GTC, OrdType::Limit, false
        )?;
        Ok(())
    }

    /// Cancel an order
    pub fn cancel_order(&mut self, order_id: u64) -> Result<(), BacktestError> {
        self.hbt.cancel(0, order_id, false)?;
        Ok(())
    }

    /// Clear all inactive orders
    pub fn clear_inactive_orders(&mut self) {
        self.hbt.clear_inactive_orders(Some(0));
    }
}

/// Helper to build PerformanceData for GUI
#[allow(dead_code)]
pub fn build_performance_data(
    state: &StrategyState,
    initial_capital: f64,
    strategy_name: &str,
    bids: Vec<OrderBookLevel>,
    asks: Vec<OrderBookLevel>,
    sim_time_secs: f64,
) -> PerformanceData {
    PerformanceData {
        timestamp: sim_time_secs,
        equity: state.equity(initial_capital),
        realized_pnl: state.realized_pnl,
        unrealized_pnl: state.unrealized_pnl,
        position: state.position,
        mid_price: state.mid_price,
        strategy_name: strategy_name.to_string(),
        num_trades: state.num_trades,
        winning_trades: state.winning_trades,
        total_fills: state.total_fills,
        total_orders: state.total_orders,
        position_hold_time: state.avg_hold_time,
        latency_micros: 100,
        bids,
        asks,
    }
}

/// Extract orderbook levels from depth
#[allow(dead_code)]
pub fn extract_orderbook<MD: MarketDepth>(
    depth: &MD,
    levels: usize,
) -> (Vec<OrderBookLevel>, Vec<OrderBookLevel>) {
    let mut bids = Vec::with_capacity(levels);
    let mut asks = Vec::with_capacity(levels);
    
    let best_bid_tick = depth.best_bid_tick();
    let best_ask_tick = depth.best_ask_tick();
    let tick_size = depth.tick_size();
    
    if best_bid_tick != i64::MIN {
        for i in 0..levels {
            let tick = best_bid_tick - i as i64;
            let qty = depth.bid_qty_at_tick(tick);
            if qty > 0.0 {
                bids.push(OrderBookLevel {
                    price: tick as f64 * tick_size,
                    quantity: qty,
                });
            }
        }
    }
    
    if best_ask_tick != i64::MAX {
        for i in 0..levels {
            let tick = best_ask_tick + i as i64;
            let qty = depth.ask_qty_at_tick(tick);
            if qty > 0.0 {
                asks.push(OrderBookLevel {
                    price: tick as f64 * tick_size,
                    quantity: qty,
                });
            }
        }
    }
    
    (bids, asks)
}
