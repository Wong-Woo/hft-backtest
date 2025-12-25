use anyhow::Result;
use hftbacktest::{
    backtest::{Backtest, BacktestError, ExchangeKind, L2AssetBuilder, assettype::LinearAsset,
        data::DataSource, models::{CommonFees, ConstantLatency, ProbQueueModel, 
        PowerProbQueueFunc3, TradingValueFeeModel}},
    prelude::{Bot, HashMapMarketDepth, Status, TimeInForce, OrdType},
    depth::MarketDepth,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use crossbeam_channel::Sender;
use crate::common::{DataLoader, calculate_mid_price, is_valid_depth};
use crate::config::{TICK_SIZE, LOT_SIZE, ELAPSE_DURATION_NS, UPDATE_INTERVAL, PRINT_INTERVAL, COMMAND_POLL_TIMEOUT_MICROS};
use crate::ui::PerformanceData;
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
}

impl MomentumRunner {
    pub fn new(
        data_pattern: String,
        lookback_period: usize,
        momentum_threshold: f64,
        position_size: f64,
        stop_loss_pct: f64,
        take_profit_pct: f64,
        initial_capital: f64,
    ) -> Result<Self> {
        let data_files = DataLoader::load_files(&data_pattern)?;

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
        })
    }

    /// Run strategy with GUI monitor and Controller
    pub fn run_with_controller(
        &mut self,
        sender: Sender<PerformanceData>,
        controller: Arc<StrategyController>,
    ) -> Result<()> {
        let file_count = self.data_files.len();
        
        for file_idx in 0..file_count {
            if controller.should_stop() {
                println!("\n⏹ Strategy stopped by user");
                break;
            }
            
            // Reset skip flag for new file
            controller.reset_skip();
            
            let data_file = self.data_files[file_idx].clone();
            
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
            
            // Check if skipped to next file
            if controller.should_skip() {
                println!("\n⏭️  Skipping to next file...");
                continue;
            }
        }
        
        if !controller.should_stop() {
            controller.mark_completed();
            println!("\n✅ All files processed successfully!");
        }
        
        Ok(())
    }

    /// Run strategy with GUI monitor (legacy version, for backward compatibility)
    #[allow(dead_code)]
    pub fn run_with_monitor(&mut self, sender: Sender<PerformanceData>) -> Result<()> {
        let file_count = self.data_files.len();
        
        for file_idx in 0..file_count {
            let data_file = self.data_files[file_idx].clone();
            
            println!("\n{}", "=".repeat(60));
            println!("Running momentum strategy on file [{}/{}]: {}", 
                     file_idx + 1, 
                     file_count, 
                     data_file.display());
            println!("{}\n", "=".repeat(60));
            
            self.run_strategy(data_file.to_str().unwrap(), Some(&sender))?;
        }
        
        println!("\n✅ All files processed successfully!");
        Ok(())
    }

    /// Run strategy on a single file (with Controller)
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

        loop {
            // Process commands (non-blocking)
            controller.process_commands(Duration::from_micros(COMMAND_POLL_TIMEOUT_MICROS));
            
            // Check for stop signal
            if controller.should_stop() {
                println!("\n⏹ Strategy stopped by user");
                break;
            }
            
            // Check for skip signal
            if controller.should_skip() {
                println!("\n⏭️  Skipping to next file...");
                break;
            }
            
            // Handle pause state
            if !controller.is_running() {
                controller.wait_while_paused();
                if controller.should_stop() || controller.should_skip() {
                    break;
                }
                continue;
            }
            
            // Speed adjustment
            let speed = controller.speed_multiplier();
            let adjusted_duration = (ELAPSE_DURATION_NS as f64 / speed) as i64;
            
            match hbt.elapse(adjusted_duration) {
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
                        
                        // Send data to GUI
                        let depth_for_data = hbt.depth(0);
                        let mid_price = calculate_mid_price(depth_for_data);
                        
                        let (position_value, unrealized_pnl) = self.calculate_position_metrics(mid_price);
                        
                        if let Err(e) = sender.send(PerformanceData {
                            timestamp: update_count as f64,
                            equity: cash + realized_pnl + position_value,
                            realized_pnl,
                            unrealized_pnl,
                            position: self.position_qty,
                            mid_price,
                            strategy_name: "Momentum".to_string(),
                        }) {
                            eprintln!("Warning: Failed to send performance data: {}", e);
                        }
                        
                        // Print status
                        if update_count % PRINT_INTERVAL == 0 {
                            let depth_for_print = hbt.depth(0);
                            self.print_status(
                                update_count, 
                                realized_pnl, 
                                cash,
                                depth_for_print
                            );
                        }
                    }
                }
                Err(_) => {
                    println!("\nEnd of data reached!");
                    break;
                }
            }
        }

        // 포지션이 남아있으면 청산
        if self.position_state != PositionState::Flat {
            println!("\nClosing remaining position...");
            let _ = self.close_position(&mut hbt, &mut realized_pnl)?;
        }

        let final_depth = hbt.depth(0);
        self.print_final_stats(realized_pnl, cash, final_depth);

        Ok(())
    }

    /// Run strategy on a single file
    fn run_strategy(&mut self, data_file: &str, sender: Option<&Sender<PerformanceData>>) -> Result<()> {
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

        loop {
            match hbt.elapse(ELAPSE_DURATION_NS) {
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
                        let _ = depth;
                        
                        // Execute strategy logic
                        self.execute_strategy(&mut hbt, &mut realized_pnl)?;
                        
                        // Send data to GUI
                        if let Some(sender) = sender {
                            let depth_for_data = hbt.depth(0);
                            let mid_price = calculate_mid_price(depth_for_data);
                            
                            let (position_value, unrealized_pnl) = self.calculate_position_metrics(mid_price);
                            
                            if let Err(e) = sender.send(PerformanceData {
                                timestamp: update_count as f64,
                                equity: cash + realized_pnl + position_value,
                                realized_pnl,
                                unrealized_pnl,
                                position: self.position_qty,
                                mid_price,
                                strategy_name: "Momentum".to_string(),
                            }) {
                                eprintln!("Warning: Failed to send performance data: {}", e);
                            }
                        }
                        
                        // 상태 출력
                        if update_count % PRINT_INTERVAL == 0 {
                            let depth_for_print = hbt.depth(0);
                            self.print_status(
                                update_count, 
                                realized_pnl, 
                                cash,
                                depth_for_print
                            );
                        }
                    }
                }
                Err(_) => {
                    println!("\nEnd of data reached!");
                    break;
                }
            }
        }

        // 포지션이 남아있으면 청산
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

        // 청산 조건 확인
        if self.position_state != PositionState::Flat {
            if self.should_close_position(mid_price) {
                println!("  Closing position due to stop loss or take profit");
                return self.close_position(hbt, realized_pnl);
            }
        }

        // 신호 생성
        let signal = self.momentum_indicator.generate_signal();
        let momentum_value = self.momentum_indicator.get_momentum();

        match self.position_state {
            PositionState::Flat => {
                // 새 포지션 진입
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
                // 롱 포지션 중 반대 신호 시 청산
                if signal == SignalType::Short {
                    println!("  ⚠️  Reverse signal detected, closing LONG position");
                    self.close_position(hbt, realized_pnl)?;
                }
            }
            PositionState::Short => {
                // 숏 포지션 중 반대 신호 시 청산
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
        let depth = hbt.depth(0);
        let tick_size = depth.tick_size();
        let best_ask_tick = depth.best_ask_tick();
        let best_ask_price = best_ask_tick as f64 * tick_size;
        
        hbt.submit_buy_order(
            0,
            100, // order_id
            best_ask_price,
            self.position_size,
            TimeInForce::GTC,
            OrdType::Limit,
            false,
        )?;

        hbt.wait_order_response(0, 100, 10_000_000_000)?;

        let orders = hbt.orders(0);
        if let Some(order) = orders.get(&100) {
            if order.status == Status::Filled {
                self.entry_price = order.price_tick as f64 * tick_size;
                self.position_qty = order.qty;
                self.position_state = PositionState::Long;
                
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
        let depth = hbt.depth(0);
        let tick_size = depth.tick_size();
        let best_bid_tick = depth.best_bid_tick();
        let best_bid_price = best_bid_tick as f64 * tick_size;
        
        hbt.submit_sell_order(
            0,
            101, // order_id
            best_bid_price,
            self.position_size,
            TimeInForce::GTC,
            OrdType::Limit,
            false,
        )?;

        hbt.wait_order_response(0, 101, 10_000_000_000)?;

        let orders = hbt.orders(0);
        if let Some(order) = orders.get(&101) {
            if order.status == Status::Filled {
                self.entry_price = order.price_tick as f64 * tick_size;
                self.position_qty = order.qty;
                self.position_state = PositionState::Short;
                
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
        let depth = hbt.depth(0);
        let tick_size = depth.tick_size();

        match self.position_state {
            PositionState::Long => {
                let best_bid_tick = depth.best_bid_tick();
                let best_bid_price = best_bid_tick as f64 * tick_size;
                
                hbt.submit_sell_order(
                    0,
                    102,
                    best_bid_price,
                    self.position_qty,
                    TimeInForce::GTC,
                    OrdType::Limit,
                    false,
                )?;

                hbt.wait_order_response(0, 102, 10_000_000_000)?;

                let orders = hbt.orders(0);
                if let Some(order) = orders.get(&102) {
                    if order.status == Status::Filled {
                        let exit_price = order.price_tick as f64 * tick_size;
                        let pnl = (exit_price - self.entry_price) * self.position_qty;
                        let fee = (exit_price * self.position_qty + self.entry_price * self.position_qty) * 0.0001;
                        *realized_pnl += pnl - fee;
                        
                        println!("    ✓ Closed LONG @ {:.2} | PnL: {:.2} | Fee: {:.2}", 
                                 exit_price, pnl, fee);
                    }
                }
            }
            PositionState::Short => {
                let best_ask_tick = depth.best_ask_tick();
                let best_ask_price = best_ask_tick as f64 * tick_size;
                
                hbt.submit_buy_order(
                    0,
                    103,
                    best_ask_price,
                    self.position_qty,
                    TimeInForce::GTC,
                    OrdType::Limit,
                    false,
                )?;

                hbt.wait_order_response(0, 103, 10_000_000_000)?;

                let orders = hbt.orders(0);
                if let Some(order) = orders.get(&103) {
                    if order.status == Status::Filled {
                        let exit_price = order.price_tick as f64 * tick_size;
                        let pnl = (self.entry_price - exit_price) * self.position_qty;
                        let fee = (exit_price * self.position_qty + self.entry_price * self.position_qty) * 0.0001;
                        *realized_pnl += pnl - fee;
                        
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

    /// 포지션 메트릭 계산 (position_value, unrealized_pnl)
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

    fn print_status<MD>(&self, update_count: usize, realized_pnl: f64, cash: f64, depth: &MD)
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

        let unrealized_pnl = match self.position_state {
            PositionState::Long => (mid_price - self.entry_price) * self.position_qty,
            PositionState::Short => (self.entry_price - mid_price) * self.position_qty,
            PositionState::Flat => 0.0,
        };

        let total_equity = cash + realized_pnl + position_value;
        let momentum = self.momentum_indicator.get_momentum();

        println!("\n[Update #{}] Status:", update_count);
        println!("  Market: Bid={:.2} Ask={:.2} Mid={:.2}", best_bid, best_ask, mid_price);
        println!("  Momentum: {:.4} ({:.2}%)", momentum, momentum * 100.0);
        println!("  Position: {:?} @ {:.2} qty {:.4}", self.position_state, self.entry_price, self.position_qty);
        println!("  PnL: Realized={:.2} Unrealized={:.2} Total={:.2}", 
                 realized_pnl, unrealized_pnl, realized_pnl + unrealized_pnl);
        println!("  Equity: {:.2} (ROI: {:.2}%)", total_equity, (total_equity - cash) / cash * 100.0);
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
