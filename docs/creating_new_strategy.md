# Creating a New Strategy

This guide explains how to create a new trading strategy using the template framework.

## Quick Start (5 minutes)

### Step 1: Copy the Template

```bash
# Create your strategy folder
mkdir src/strategy/my_strategy
```

Copy this minimal template to `src/strategy/my_strategy/my_strategy.rs`:

```rust
use anyhow::Result;
use hftbacktest::backtest::BacktestError;
use crate::strategy::base::{Strategy, StrategyState, TickContext, StrategyRunner};
use crate::ui::PerformanceData;
use crate::controller::StrategyController;
use crossbeam_channel::Sender;
use std::sync::Arc;

pub struct MyStrategy {
    position_size: f64,
    initial_capital: f64,
    order_id: u64,
}

impl MyStrategy {
    pub fn new(position_size: f64, initial_capital: f64) -> Self {
        Self { position_size, initial_capital, order_id: 0 }
    }
    
    pub fn run_with_files(
        files: Vec<String>,
        position_size: f64,
        initial_capital: f64,
        sender: Sender<PerformanceData>,
        controller: Arc<StrategyController>,
    ) -> Result<()> {
        let strategy = Self::new(position_size, initial_capital);
        let runner = StrategyRunner::new(strategy, files)?;
        runner.run_with_controller(sender, controller)
    }
    
    fn next_order_id(&mut self) -> u64 {
        self.order_id += 1;
        self.order_id
    }
}

impl Strategy for MyStrategy {
    fn name(&self) -> &str { "My Strategy" }
    fn initial_capital(&self) -> f64 { self.initial_capital }
    
    fn on_tick(
        &mut self,
        ctx: &mut TickContext<'_>,
        state: &mut StrategyState,
    ) -> Result<(), BacktestError> {
        // ========================================
        // YOUR STRATEGY LOGIC GOES HERE
        // ========================================
        
        let mid_price = ctx.mid_price();
        let best_bid = ctx.best_bid();
        let best_ask = ctx.best_ask();
        
        // Example: Buy if no position
        if state.position == 0.0 {
            let order_id = self.next_order_id();
            ctx.submit_buy_order(best_bid, self.position_size, order_id)?;
            state.total_orders += 1;
        }
        
        Ok(())
    }
    
    fn update_interval(&self) -> u64 { 100 } // Run every 100 ticks
}
```

### Step 2: Create mod.rs

Create `src/strategy/my_strategy/mod.rs`:

```rust
mod my_strategy;
pub use my_strategy::MyStrategy;
```

### Step 3: Register in strategy/mod.rs

Add to `src/strategy/mod.rs`:

```rust
pub mod my_strategy;
pub use my_strategy::MyStrategy;
```

### Step 4: Add to StrategyType

Add to `src/strategy/strategy_type.rs`:

```rust
pub enum StrategyType {
    // ... existing variants
    MyStrategy {
        position_size: f64,
        initial_capital: f64,
    },
}

impl StrategyType {
    pub fn run(...) -> Result<()> {
        match self {
            // ... existing matches
            StrategyType::MyStrategy { position_size, initial_capital } => {
                MyStrategy::run_with_files(
                    data_files, *position_size, *initial_capital,
                    sender, controller
                )
            }
        }
    }
}
```

Done! Your strategy is ready to run.

---

## Available APIs

### TickContext (Market Data & Orders)

```rust
// Price Data
ctx.mid_price()     // Current mid price
ctx.best_bid()      // Best bid price
ctx.best_ask()      // Best ask price
ctx.spread()        // Bid-ask spread

// Order Book
ctx.bid_qty(0)      // Quantity at best bid (level 0)
ctx.ask_qty(2)      // Quantity at 3rd ask level
ctx.depth()         // Raw MarketDepth reference

// Time
ctx.timestamp_ns()  // Current simulation time (nanoseconds)

// Order Submission
ctx.submit_buy_order(price, qty, order_id)?;
ctx.submit_sell_order(price, qty, order_id)?;
ctx.cancel_order(order_id)?;
ctx.clear_inactive_orders();
```

### StrategyState (Your Position & Stats)

```rust
state.position        // Current position (+ long, - short)
state.entry_price     // Entry price of current position
state.realized_pnl    // Realized PnL
state.unrealized_pnl  // Unrealized PnL
state.mid_price       // Latest mid price

state.num_trades      // Total number of trades
state.winning_trades  // Number of winning trades
state.total_orders    // Total orders submitted
state.total_fills     // Total orders filled
state.avg_hold_time   // Average position hold time

state.win_rate()      // Win rate percentage
state.fill_ratio()    // Fill ratio percentage
state.equity(capital) // Total equity
```

### Strategy Trait (Optional Overrides)

```rust
impl Strategy for MyStrategy {
    // Required
    fn name(&self) -> &str;
    fn initial_capital(&self) -> f64;
    fn on_tick(&mut self, ctx, state) -> Result<(), BacktestError>;
    
    // Optional
    fn on_file_start(&mut self, file_path: &str) { }
    fn on_file_end(&mut self, state: &StrategyState) { }
    fn on_completed(&mut self, state: &StrategyState) { }
    fn update_interval(&self) -> u64 { 1 }      // How often to run
    fn orderbook_depth(&self) -> usize { 10 }   // GUI orderbook levels
}
```

---

## Example Strategies

### Mean Reversion

```rust
fn on_tick(&mut self, ctx: &mut TickContext<'_>, state: &mut StrategyState) -> Result<(), BacktestError> {
    let mid = ctx.mid_price();
    
    // Track moving average
    self.prices.push(mid);
    if self.prices.len() > 100 { self.prices.remove(0); }
    let ma = self.prices.iter().sum::<f64>() / self.prices.len() as f64;
    
    // Mean reversion signals
    let deviation = (mid - ma) / ma;
    
    if deviation < -0.001 && state.position <= 0.0 {
        ctx.submit_buy_order(ctx.best_bid(), 0.01, self.next_id())?;
    } else if deviation > 0.001 && state.position >= 0.0 {
        ctx.submit_sell_order(ctx.best_ask(), 0.01, self.next_id())?;
    }
    
    Ok(())
}
```

### Momentum

```rust
fn on_tick(&mut self, ctx: &mut TickContext<'_>, state: &mut StrategyState) -> Result<(), BacktestError> {
    let mid = ctx.mid_price();
    let prev = self.prev_price;
    self.prev_price = mid;
    
    if prev == 0.0 { return Ok(()); }
    
    let momentum = (mid - prev) / prev;
    
    // Follow the trend
    if momentum > 0.0005 && state.position == 0.0 {
        ctx.submit_buy_order(mid, 0.01, self.next_id())?;
    } else if momentum < -0.0005 && state.position == 0.0 {
        ctx.submit_sell_order(mid, 0.01, self.next_id())?;
    }
    
    Ok(())
}
```

### Market Making

```rust
fn on_tick(&mut self, ctx: &mut TickContext<'_>, state: &mut StrategyState) -> Result<(), BacktestError> {
    ctx.clear_inactive_orders();
    
    let mid = ctx.mid_price();
    let spread = 0.5; // Half spread offset
    
    // Quote both sides
    if state.position.abs() < self.max_position {
        ctx.submit_buy_order(mid - spread, 0.01, self.next_id())?;
        ctx.submit_sell_order(mid + spread, 0.01, self.next_id())?;
        state.total_orders += 2;
    }
    
    Ok(())
}
```

---

## Tips

1. **Keep on_tick() fast** - This runs on every tick
2. **Use update_interval()** - Run expensive logic less frequently
3. **Track your own state** - Add fields to your strategy struct
4. **Handle fills manually** - The framework doesn't auto-update position
5. **Test with small position sizes first**
