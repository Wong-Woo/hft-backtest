use anyhow::Result;
use hftbacktest::{
    backtest::{Backtest, BacktestError, ExchangeKind, L2AssetBuilder, assettype::LinearAsset,
        data::DataSource, models::{CommonFees, ConstantLatency, ProbQueueModel, 
        PowerProbQueueFunc3, TradingValueFeeModel}},
    prelude::{Bot, HashMapMarketDepth, Status, TimeInForce, OrdType},
    depth::MarketDepth,
    types::ElapseResult,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use crossbeam_channel::Sender;
use crate::common::{calculate_mid_price, is_valid_depth};
use crate::config::{TICK_SIZE, LOT_SIZE, ELAPSE_DURATION_NS, UPDATE_INTERVAL, COMMAND_POLL_TIMEOUT_MICROS};
use crate::ui::{PerformanceData, OrderBookLevel};
use crate::controller::StrategyController;
use super::{MomentumIndicator, SignalType};

#[derive(Debug, Clone, Copy, PartialEq)]
enum PositionState {
    Flat,
    Long,
    Short,
}

pub struct MomentumRunner {
    data_files: Vec<PathBuf>,
    momentum_indicator: MomentumIndicator,
    #[allow(dead_code)]
    lookback_period: usize,
    #[allow(dead_code)]
    momentum_threshold: f64,
    position_size: f64,
    stop_loss_pct: f64,
    take_profit_pct: f64,
    initial_capital: f64,
    position_state: PositionState,
    entry_price: f64,
    position_qty: f64,
    // Metrics tracking
    num_trades: usize,
    winning_trades: usize,
    total_orders: usize,
    total_fills: usize,
    #[allow(dead_code)]
    position_entry_time: Option<Instant>,
    total_hold_time: Duration,
    next_order_id: u64,
}

impl MomentumRunner {
    pub fn new_with_files(
        files: Vec<String>,
        lookback_period: usize,
        momentum_threshold: f64,
        position_size: f64,
        stop_loss_pct: f64,
        take_profit_pct: f64,
        initial_capital: f64,
    ) -> Result<Self> {
        let data_files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
        if data_files.is_empty() {
            anyhow::bail!("No data files provided");
        }
        println!("Using {} file(s):", data_files.len());
        for (i, f) in data_files.iter().enumerate() {
            println!("  [{}] {}", i + 1, f.display());
        }
        Self::create_runner(data_files, lookback_period, momentum_threshold, position_size, stop_loss_pct, take_profit_pct, initial_capital)
    }
    
    fn create_runner(
        data_files: Vec<PathBuf>,
        lookback_period: usize,
        momentum_threshold: f64,
        position_size: f64,
        stop_loss_pct: f64,
        take_profit_pct: f64,
        initial_capital: f64,
    ) -> Result<Self> {
        Ok(Self {
            data_files,
            momentum_indicator: MomentumIndicator::new(lookback_period, momentum_threshold),
            lookback_period,
            momentum_threshold,
            position_size,
            stop_loss_pct,
            take_profit_pct,
            initial_capital,
            position_state: PositionState::Flat,
            entry_price: 0.0,
            position_qty: 0.0,
            num_trades: 0,
            winning_trades: 0,
            total_orders: 0,
            total_fills: 0,
            position_entry_time: None,
            total_hold_time: Duration::ZERO,
            next_order_id: 1,
        })
    }
    
    /// Extract order book levels from market depth
    fn extract_orderbook<MD>(&self, depth: &MD, levels: usize) -> (Vec<OrderBookLevel>, Vec<OrderBookLevel>)
    where
        MD: MarketDepth,
    {
        let mut bids = Vec::new();
        let mut asks = Vec::new();
        
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

    /// Run strategy with GUI monitor and Controller
    pub fn run_with_controller(
        &mut self,
        sender: Sender<PerformanceData>,
        controller: Arc<StrategyController>,
    ) -> Result<()> {
        let file_count = self.data_files.len();
        
        for file_idx in 0..file_count {
            // Wait for start signal if in paused or stopped state
            while !controller.is_running() && !controller.should_stop() {
                controller.process_commands(Duration::from_millis(100));
            }
            
            if controller.should_stop() {
                println!("\n⏹ Strategy stopped by user");
                break;
            }
            
            let data_file = self.data_files[file_idx].clone();
            
            // Notify GUI to clear chart data for new file (except first file)
            if file_idx > 0 {
                controller.notify_new_file();
            }
            
            println!("\n{}", "=".repeat(60));
            println!("Running momentum strategy on file [{}/{}]: {}", 
                     file_idx + 1, 
                     file_count, 
                     data_file.display());
            println!("{}\n", "=".repeat(60));
            
            self.run_strategy_with_control(
                data_file.to_str().unwrap(),
                &sender,
                &controller,
            )?;
        }
        
        if !controller.should_stop() {
            controller.mark_completed();
            println!("\n✅ All files processed successfully!");
        }
        
        // Keep thread alive until GUI closes
        self.keep_alive_until_close(&controller);
        
        Ok(())
    }
    
    /// Keep thread alive until GUI window closes
    fn keep_alive_until_close(&self, controller: &StrategyController) {
        println!("Backtest finished. Close the window to exit.");
        
        loop {
            if !controller.process_commands(Duration::from_millis(200)) {
                std::thread::sleep(Duration::from_millis(100));
                if !controller.process_commands(Duration::from_millis(100)) {
                    break;
                }
            }
        }
    }

    fn run_strategy_with_control(
        &mut self,
        data_file: &str,
        sender: &Sender<PerformanceData>,
        controller: &StrategyController,
    ) -> Result<()> {
        println!("Loading data from: {}", data_file);

        let mut hbt = self.create_backtest(data_file)?;
        
        println!("Momentum strategy started...\n");

        let mut realized_pnl = 0.0;
        let cash = self.initial_capital;
        let mut update_count = 0;

        println!("Waiting for market data...\n");

        // Initialize position state
        self.position_state = PositionState::Flat;
        self.entry_price = 0.0;
        self.position_qty = 0.0;

        let mut last_gui_update = Instant::now();
        let mut last_command_check = Instant::now();
        let command_check_interval = Duration::from_millis(16); // ~60Hz command polling
        let mut data_ended = false;

        loop {
            // Check if data has ended
            if data_ended {
                println!("\nEnd of data reached!");
                if self.position_state != PositionState::Flat {
                    println!("Closing remaining position...");
                    let _ = self.close_position(&mut hbt, &mut realized_pnl)?;
                }
                let final_depth = hbt.depth(0);
                self.print_final_stats(realized_pnl, cash, final_depth);
                return Ok(());
            }
            
            // Check pause/stop state (always, regardless of timing)
            if !controller.is_running() {
                // Process commands while paused
                controller.process_commands(Duration::from_millis(50));
                
                if controller.should_stop() {
                    println!("\n⏹ Strategy stopped by user");
                    break;
                }
                continue;
            }
            
            // Process commands at fixed interval when running
            if last_command_check.elapsed() >= command_check_interval {
                controller.process_commands(Duration::from_micros(COMMAND_POLL_TIMEOUT_MICROS));
                last_command_check = Instant::now();
                
                if controller.should_stop() {
                    println!("\n⏹ Strategy stopped by user");
                    break;
                }
            }
            
            // Speed adjustment - affects simulation time
            let speed = controller.speed_multiplier();
            
            // Calculate iterations and delay based on speed
            let (iterations_per_loop, loop_delay_ms) = if speed >= 100.0 {
                (100, 0u64)
            } else if speed >= 10.0 {
                ((speed / 10.0).ceil() as usize, 1)
            } else if speed >= 1.0 {
                (1, (10.0 / speed) as u64)
            } else {
                (1, (10.0 / speed) as u64)
            };
            
            for _ in 0..iterations_per_loop {
                match hbt.elapse(ELAPSE_DURATION_NS) {
                    Ok(ElapseResult::EndOfData) => {
                        data_ended = true;
                        break;
                    }
                    Ok(_) => {
                        let depth = hbt.depth(0);
                        
                        if !is_valid_depth(depth) {
                            continue;
                        }
                        
                        update_count += 1;
                        
                        let mid_price = calculate_mid_price(depth);
                        
                        // Update momentum indicator
                        self.momentum_indicator.update(mid_price);

                        if update_count % UPDATE_INTERVAL == 0 {
                            // Execute strategy logic
                            self.execute_strategy(&mut hbt, &mut realized_pnl)?;
                        }
                    }
                    Err(_) => {
                        data_ended = true;
                        break;
                    }
                }
            }
            
            // Send data to GUI (throttled to ~30 FPS)
            if last_gui_update.elapsed() >= Duration::from_millis(33) {
                let depth_for_data = hbt.depth(0);
                if is_valid_depth(depth_for_data) {
                    let mid_price = calculate_mid_price(depth_for_data);
                    
                    let (position_value, unrealized_pnl) = self.calculate_position_metrics(mid_price);
                    let (bids, asks) = self.extract_orderbook(depth_for_data, 10);
                    let avg_hold_time = if self.num_trades > 0 {
                        self.total_hold_time.as_secs_f64() / self.num_trades as f64
                    } else {
                        0.0
                    };
                    
                    // Use try_send to avoid blocking GUI
                    // timestamp = simulation time in seconds
                    let sim_time_secs = update_count as f64 * (ELAPSE_DURATION_NS as f64 / 1_000_000_000.0);
                    let _ = sender.try_send(PerformanceData {
                        timestamp: sim_time_secs,
                        equity: cash + realized_pnl + position_value,
                        realized_pnl,
                        unrealized_pnl,
                        position: self.position_qty,
                        mid_price,
                        strategy_name: "Momentum".to_string(),
                        num_trades: self.num_trades,
                        winning_trades: self.winning_trades,
                        total_fills: self.total_fills,
                        total_orders: self.total_orders,
                        position_hold_time: avg_hold_time,
                        latency_micros: 100,
                        bids,
                        asks,
                    });
                }
                last_gui_update = Instant::now();
            }
            
            // Apply speed-based delay
            if loop_delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(loop_delay_ms));
            } else {
                std::thread::yield_now();
            }
        }

        // Close remaining position
        if self.position_state != PositionState::Flat {
            println!("\nClosing remaining position...");
            let _ = self.close_position(&mut hbt, &mut realized_pnl)?;
        }

        let final_depth = hbt.depth(0);
        self.print_final_stats(realized_pnl, cash, final_depth);

        Ok(())
    }

    fn execute_strategy<MD>(
        &mut self,
        hbt: &mut Backtest<MD>,
        realized_pnl: &mut f64,
    ) -> Result<(), BacktestError>
    where
        MD: MarketDepth,
    {
        if !self.momentum_indicator.is_ready() {
            return Ok(());
        }

        let depth = hbt.depth(0);
        let mid_price = calculate_mid_price(depth);

        // Check exit conditions (stop-loss or take-profit)
        if self.position_state != PositionState::Flat {
            if self.should_close_position(mid_price) {
                println!("  Closing position due to stop loss or take profit");
                return self.close_position(hbt, realized_pnl);
            }
        }

        // Generate signals based on momentum
        let signal = self.momentum_indicator.generate_signal();
        let momentum_value = self.momentum_indicator.get_momentum();

        match self.position_state {
            PositionState::Flat => {
                // Enter new position based on signal
                match signal {
                    SignalType::Long => {
                        println!("  🟢 LONG signal detected | Momentum: {:.4}", momentum_value);
                        self.open_long_position(hbt)?;
                    }
                    SignalType::Short => {
                        println!("  🔴 SHORT signal detected | Momentum: {:.4}", momentum_value);
                        self.open_short_position(hbt)?;
                    }
                    SignalType::Neutral => {}
                }
            }
            PositionState::Long => {
                // Close long position on opposite signal
                if signal == SignalType::Short {
                    println!("  ⚠️  Reverse signal detected, closing LONG position");
                    self.close_position(hbt, realized_pnl)?;
                }
            }
            PositionState::Short => {
                // Close short position on opposite signal
                if signal == SignalType::Long {
                    println!("  ⚠️  Reverse signal detected, closing SHORT position");
                    self.close_position(hbt, realized_pnl)?;
                }
            }
        }

        Ok(())
    }

    fn open_long_position<MD>(
        &mut self,
        hbt: &mut Backtest<MD>,
    ) -> Result<(), BacktestError>
    where
        MD: MarketDepth,
    {
        // Clear any pending orders first
        hbt.clear_inactive_orders(Some(0));
        
        let depth = hbt.depth(0);
        let tick_size = depth.tick_size();
        let best_ask_tick = depth.best_ask_tick();
        let best_ask_price = best_ask_tick as f64 * tick_size;
        
        let order_id = self.next_order_id;
        self.next_order_id += 1;
        
        hbt.submit_buy_order(
            0,
            order_id,
            best_ask_price,
            self.position_size,
            TimeInForce::GTC,
            OrdType::Limit,
            false,
        )?;
        self.total_orders += 1;

        // Short timeout to avoid blocking - 100ms
        let _ = hbt.wait_order_response(0, order_id, 100_000_000);

        let orders = hbt.orders(0);
        if let Some(order) = orders.get(&order_id) {
            if order.status == Status::Filled {
                self.entry_price = order.price_tick as f64 * tick_size;
                self.position_qty = order.qty;
                self.position_state = PositionState::Long;                self.total_fills += 1;                
                println!("    ✓ Opened LONG @ {:.2} qty {:.4}", self.entry_price, self.position_qty);
            }
        }

        Ok(())
    }

    fn open_short_position<MD>(
        &mut self,
        hbt: &mut Backtest<MD>,
    ) -> Result<(), BacktestError>
    where
        MD: MarketDepth,
    {
        // Clear any pending orders first
        hbt.clear_inactive_orders(Some(0));
        
        let depth = hbt.depth(0);
        let tick_size = depth.tick_size();
        let best_bid_tick = depth.best_bid_tick();
        let best_bid_price = best_bid_tick as f64 * tick_size;
        
        let order_id = self.next_order_id;
        self.next_order_id += 1;
        
        hbt.submit_sell_order(
            0,
            order_id,
            best_bid_price,
            self.position_size,
            TimeInForce::GTC,
            OrdType::Limit,
            false,
        )?;
        self.total_orders += 1;

        // Short timeout to avoid blocking - 100ms
        let _ = hbt.wait_order_response(0, order_id, 100_000_000);

        let orders = hbt.orders(0);
        if let Some(order) = orders.get(&order_id) {
            if order.status == Status::Filled {
                self.entry_price = order.price_tick as f64 * tick_size;
                self.position_qty = order.qty;
                self.position_state = PositionState::Short;
                self.total_fills += 1;
                
                println!("    ✓ Opened SHORT @ {:.2} qty {:.4}", self.entry_price, self.position_qty);
            }
        }

        Ok(())
    }

    fn close_position<MD>(
        &mut self,
        hbt: &mut Backtest<MD>,
        realized_pnl: &mut f64,
    ) -> Result<(), BacktestError>
    where
        MD: MarketDepth,
    {
        // Clear any pending orders first
        hbt.clear_inactive_orders(Some(0));
        
        let depth = hbt.depth(0);
        let tick_size = depth.tick_size();

        match self.position_state {
            PositionState::Long => {
                let best_bid_tick = depth.best_bid_tick();
                let best_bid_price = best_bid_tick as f64 * tick_size;
                
                let order_id = self.next_order_id;
                self.next_order_id += 1;
                
                hbt.submit_sell_order(
                    0,
                    order_id,
                    best_bid_price,
                    self.position_qty,
                    TimeInForce::GTC,
                    OrdType::Limit,
                    false,
                )?;
                self.total_orders += 1;

                // Short timeout to avoid blocking - 100ms
                let _ = hbt.wait_order_response(0, order_id, 100_000_000);

                let orders = hbt.orders(0);
                if let Some(order) = orders.get(&order_id) {
                    if order.status == Status::Filled {
                        let exit_price = order.price_tick as f64 * tick_size;
                        let pnl = (exit_price - self.entry_price) * self.position_qty;
                        let fee = (exit_price * self.position_qty + self.entry_price * self.position_qty) * 0.0001;
                        *realized_pnl += pnl - fee;
                        self.total_fills += 1;
                        
                        println!("    ✓ Closed LONG @ {:.2} | PnL: {:.2} | Fee: {:.2}", 
                                 exit_price, pnl, fee);
                    }
                }
            }
            PositionState::Short => {
                let best_ask_tick = depth.best_ask_tick();
                let best_ask_price = best_ask_tick as f64 * tick_size;
                
                let order_id = self.next_order_id;
                self.next_order_id += 1;
                
                hbt.submit_buy_order(
                    0,
                    order_id,
                    best_ask_price,
                    self.position_qty,
                    TimeInForce::GTC,
                    OrdType::Limit,
                    false,
                )?;
                self.total_orders += 1;

                // Short timeout to avoid blocking - 100ms
                let _ = hbt.wait_order_response(0, order_id, 100_000_000);

                let orders = hbt.orders(0);
                if let Some(order) = orders.get(&order_id) {
                    if order.status == Status::Filled {
                        let exit_price = order.price_tick as f64 * tick_size;
                        let pnl = (self.entry_price - exit_price) * self.position_qty;
                        let fee = (exit_price * self.position_qty + self.entry_price * self.position_qty) * 0.0001;
                        *realized_pnl += pnl - fee;
                        self.total_fills += 1;
                        
                        println!("    ✓ Closed SHORT @ {:.2} | PnL: {:.2} | Fee: {:.2}", 
                                 exit_price, pnl, fee);
                    }
                }
            }
            PositionState::Flat => {}
        }

        self.position_state = PositionState::Flat;
        self.entry_price = 0.0;
        self.position_qty = 0.0;

        Ok(())
    }

    /// Calculate position metrics (position_value, unrealized_pnl)
    fn calculate_position_metrics(&self, mid_price: f64) -> (f64, f64) {
        match self.position_state {
            PositionState::Long => {
                let position_value = self.position_qty * mid_price;
                let unrealized_pnl = (mid_price - self.entry_price) * self.position_qty;
                (position_value, unrealized_pnl)
            }
            PositionState::Short => {
                let position_value = -self.position_qty * mid_price;
                let unrealized_pnl = (self.entry_price - mid_price) * self.position_qty;
                (position_value, unrealized_pnl)
            }
            PositionState::Flat => (0.0, 0.0),
        }
    }

    fn should_close_position(&self, current_price: f64) -> bool {
        if self.entry_price == 0.0 {
            return false;
        }

        match self.position_state {
            PositionState::Long => {
                let pnl_pct = (current_price - self.entry_price) / self.entry_price;
                pnl_pct <= -self.stop_loss_pct || pnl_pct >= self.take_profit_pct
            }
            PositionState::Short => {
                let pnl_pct = (self.entry_price - current_price) / self.entry_price;
                pnl_pct <= -self.stop_loss_pct || pnl_pct >= self.take_profit_pct
            }
            PositionState::Flat => false,
        }
    }

    fn create_backtest(&self, data_file: &str) -> Result<Backtest<HashMapMarketDepth>> {
        let latency_model = ConstantLatency::new(0, 0);
        let queue_model = ProbQueueModel::new(PowerProbQueueFunc3::new(3.0));
        let asset_type = LinearAsset::new(1.0);
        let fee_model = TradingValueFeeModel::new(CommonFees::new(-0.00005, 0.0007));

        let hbt = Backtest::builder()
            .add_asset(
                L2AssetBuilder::new()
                    .data(vec![
                        DataSource::File(data_file.to_string())
                    ])
                    .latency_model(latency_model)
                    .queue_model(queue_model)
                    .asset_type(asset_type)
                    .fee_model(fee_model)
                    .exchange(ExchangeKind::NoPartialFillExchange)
                    .depth(|| HashMapMarketDepth::new(TICK_SIZE, LOT_SIZE))
                    .build()?,
            )
            .build()?;

        Ok(hbt)
    }

    fn print_final_stats<MD>(&self, realized_pnl: f64, cash: f64, depth: &MD)
    where
        MD: MarketDepth,
    {
        let tick_size = depth.tick_size();
        let best_bid = depth.best_bid_tick() as f64 * tick_size;
        let best_ask = depth.best_ask_tick() as f64 * tick_size;
        let mid_price = (best_bid + best_ask) / 2.0;

        let position_value = match self.position_state {
            PositionState::Long => self.position_qty * mid_price,
            PositionState::Short => -self.position_qty * mid_price,
            PositionState::Flat => 0.0,
        };

        let total_equity = cash + realized_pnl + position_value;

        println!("\n{}", "=".repeat(60));
        println!("Final Statistics:");
        println!("{}", "=".repeat(60));
        println!("Initial Capital: ${:.2}", cash);
        println!("Realized PnL: ${:.2}", realized_pnl);
        println!("Final Position Value: ${:.2}", position_value);
        println!("Total Equity: ${:.2}", total_equity);
        println!("Total Return: {:.2}%", (total_equity - cash) / cash * 100.0);
        println!("{}", "=".repeat(60));
    }
}
