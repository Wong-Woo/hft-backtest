//! Example Strategy Template
//! 
//! Copy this file to create a new strategy.
//! You only need to implement the `on_tick` method for your core logic.
//!
//! ## Quick Start
//! 1. Copy this file to `src/strategy/my_strategy/my_strategy.rs`
//! 2. Implement your logic in `on_tick()`
//! 3. Add to `StrategyType` enum
//! 4. Run!

use anyhow::Result;
use hftbacktest::backtest::BacktestError;
use crate::strategy::base::{Strategy, StrategyState, TickContext, StrategyRunner};
use crate::ui::PerformanceData;
use crate::controller::StrategyController;
use crossbeam_channel::Sender;
use std::sync::Arc;

/// Your strategy struct - add your own fields here
#[allow(dead_code)]
pub struct ExampleStrategy {
    // Configuration
    position_size: f64,
    threshold: f64,
    initial_capital: f64,
    
    // Internal state (optional)
    last_signal: f64,
    order_id: u64,
}

impl ExampleStrategy {
    pub fn new(position_size: f64, threshold: f64, initial_capital: f64) -> Self {
        Self {
            position_size,
            threshold,
            initial_capital,
            last_signal: 0.0,
            order_id: 0,
        }
    }
    
    /// Helper: Run with files (call this from StrategyType)
    pub fn run_with_files(
        files: Vec<String>,
        position_size: f64,
        threshold: f64,
        initial_capital: f64,
        sender: Sender<PerformanceData>,
        controller: Arc<StrategyController>,
    ) -> Result<()> {
        let strategy = Self::new(position_size, threshold, initial_capital);
        let runner = StrategyRunner::new(strategy, files)?;
        runner.run_with_controller(sender, controller)
    }
    
    fn next_order_id(&mut self) -> u64 {
        self.order_id += 1;
        self.order_id
    }
}

impl Strategy for ExampleStrategy {
    fn name(&self) -> &str {
        "Example Strategy"
    }
    
    fn initial_capital(&self) -> f64 {
        self.initial_capital
    }
    
    /// MAIN LOGIC: Called on each market tick
    /// 
    /// Available in `ctx`:
    /// - ctx.mid_price(), ctx.best_bid(), ctx.best_ask(), ctx.spread()
    /// - ctx.bid_qty(level), ctx.ask_qty(level)
    /// - ctx.submit_buy_order(price, qty, id), ctx.submit_sell_order(price, qty, id)
    /// - ctx.cancel_order(id), ctx.clear_inactive_orders()
    /// 
    /// Available in `state`:
    /// - state.position, state.realized_pnl, state.unrealized_pnl
    /// - state.num_trades, state.winning_trades, state.total_orders, state.total_fills
    fn on_tick(
        &mut self,
        ctx: &mut TickContext<'_>,
        state: &mut StrategyState,
    ) -> Result<(), BacktestError> {
        let mid_price = ctx.mid_price();
        let _spread = ctx.spread();
        
        // Example: Simple mean reversion logic
        let signal = mid_price - self.last_signal;
        self.last_signal = mid_price;
        
        // Only trade if we have enough signal
        if signal.abs() < self.threshold {
            return Ok(());
        }
        
        // If price dropped significantly, buy
        if signal < -self.threshold && state.position <= 0.0 {
            let order_id = self.next_order_id();
            let buy_price = ctx.best_bid();
            
            ctx.submit_buy_order(buy_price, self.position_size, order_id)?;
            state.total_orders += 1;
        }
        
        // If price rose significantly, sell
        if signal > self.threshold && state.position >= 0.0 {
            let order_id = self.next_order_id();
            let sell_price = ctx.best_ask();
            
            ctx.submit_sell_order(sell_price, self.position_size, order_id)?;
            state.total_orders += 1;
        }
        
        // Update unrealized PnL
        if state.position != 0.0 {
            state.unrealized_pnl = state.position * (mid_price - state.entry_price);
        }
        
        Ok(())
    }
    
    fn on_file_start(&mut self, file_path: &str) {
        println!("📂 Starting: {}", file_path);
        self.last_signal = 0.0;
    }
    
    fn on_file_end(&mut self, state: &StrategyState) {
        println!("📊 File completed - Trades: {}, PnL: ${:.2}", 
                 state.num_trades, state.realized_pnl);
    }
    
    fn on_completed(&mut self, state: &StrategyState) {
        println!("\n╔══════════════════════════════════════╗");
        println!("║     {} Results      ║", self.name());
        println!("╠══════════════════════════════════════╣");
        println!("║ Total PnL:     ${:>18.2} ║", state.realized_pnl);
        println!("║ Trades:        {:>18}   ║", state.num_trades);
        println!("║ Win Rate:      {:>17.1}%  ║", state.win_rate());
        println!("║ Fill Ratio:    {:>17.1}%  ║", state.fill_ratio());
        println!("╚══════════════════════════════════════╝");
    }
    
    fn update_interval(&self) -> u64 {
        // Run logic every N updates (1 = every tick)
        100
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_strategy_creation() {
        let strategy = ExampleStrategy::new(0.01, 0.5, 100_000.0);
        assert_eq!(strategy.name(), "Example Strategy");
        assert_eq!(strategy.initial_capital(), 100_000.0);
    }
}
