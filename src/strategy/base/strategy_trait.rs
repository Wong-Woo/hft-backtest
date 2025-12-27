use hftbacktest::{
    backtest::{Backtest, BacktestError},
    prelude::{HashMapMarketDepth, Bot},
    depth::MarketDepth,
};
use crate::ui::{PerformanceData, OrderBookLevel};

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

#[allow(dead_code)]
pub trait Strategy: Send {
    fn name(&self) -> &str;
    fn initial_capital(&self) -> f64;
    
    fn on_tick(
        &mut self,
        ctx: &mut TickContext<'_>,
        state: &mut StrategyState,
    ) -> Result<(), BacktestError>;
    
    fn on_file_start(&mut self, _file_path: &str) {}
    
    fn on_file_end(&mut self, _state: &StrategyState) {}
    
    fn on_completed(&mut self, state: &StrategyState) {
        println!("\n=== {} Results ===", self.name());
        println!("Total PnL: ${:.2}", state.realized_pnl);
        println!("Trades: {} (Win rate: {:.1}%)", state.num_trades, state.win_rate());
    }
    
    fn update_interval(&self) -> u64 { 1 }
    
    fn orderbook_depth(&self) -> usize { 10 }
}

pub struct TickContext<'a> {
    pub hbt: &'a mut Backtest<HashMapMarketDepth>,
    depth_cache: Option<DepthSnapshot>,
}

#[derive(Clone)]
struct DepthSnapshot {
    best_bid: f64,
    best_ask: f64,
    mid_price: f64,
    spread: f64,
}

#[allow(dead_code)]
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

    pub fn mid_price(&mut self) -> f64 {
        self.ensure_depth_cache();
        self.depth_cache.as_ref().unwrap().mid_price
    }

    pub fn best_bid(&mut self) -> f64 {
        self.ensure_depth_cache();
        self.depth_cache.as_ref().unwrap().best_bid
    }

    pub fn best_ask(&mut self) -> f64 {
        self.ensure_depth_cache();
        self.depth_cache.as_ref().unwrap().best_ask
    }

    pub fn spread(&mut self) -> f64 {
        self.ensure_depth_cache();
        self.depth_cache.as_ref().unwrap().spread
    }

    pub fn depth(&self) -> &HashMapMarketDepth {
        self.hbt.depth(0)
    }

    pub fn bid_qty(&self, level: usize) -> f64 {
        let depth = self.hbt.depth(0);
        let tick = depth.best_bid_tick() - level as i64;
        depth.bid_qty_at_tick(tick)
    }

    pub fn ask_qty(&self, level: usize) -> f64 {
        let depth = self.hbt.depth(0);
        let tick = depth.best_ask_tick() + level as i64;
        depth.ask_qty_at_tick(tick)
    }

    pub fn timestamp_ns(&self) -> i64 {
        self.hbt.current_timestamp()
    }

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

    pub fn cancel_order(&mut self, order_id: u64) -> Result<(), BacktestError> {
        self.hbt.cancel(0, order_id, false)?;
        Ok(())
    }

    pub fn clear_inactive_orders(&mut self) {
        self.hbt.clear_inactive_orders(Some(0));
    }
}

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
