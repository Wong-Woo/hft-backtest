use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use crossbeam_channel::Sender;
use hftbacktest::{
    backtest::{Backtest, ExchangeKind, L2AssetBuilder, assettype::LinearAsset,
        data::DataSource, models::{CommonFees, ConstantLatency, ProbQueueModel, 
        PowerProbQueueFunc3, TradingValueFeeModel}},
    prelude::{HashMapMarketDepth, Bot},
    types::ElapseResult,
};
use crate::common::is_valid_depth;
use crate::config::{TICK_SIZE, LOT_SIZE, ELAPSE_DURATION_NS, COMMAND_POLL_TIMEOUT_MICROS};
use crate::ui::PerformanceData;
use crate::controller::StrategyController;
use super::{Strategy, StrategyState, TickContext, build_performance_data, extract_orderbook};

pub struct StrategyRunner<S: Strategy> {
    strategy: S,
    data_files: Vec<PathBuf>,
}

impl<S: Strategy> StrategyRunner<S> {
    pub fn new(strategy: S, files: Vec<String>) -> Result<Self> {
        let data_files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
        if data_files.is_empty() {
            anyhow::bail!("No data files provided");
        }
        
        println!("Strategy: {}", strategy.name());
        println!("Using {} file(s):", data_files.len());
        for (i, f) in data_files.iter().enumerate() {
            println!("  [{}] {}", i + 1, f.display());
        }
        
        Ok(Self { strategy, data_files })
    }

    pub fn run_with_controller(
        mut self,
        sender: Sender<PerformanceData>,
        controller: Arc<StrategyController>,
    ) -> Result<()> {
        let file_count = self.data_files.len();
        
        for file_idx in 0..file_count {
            while !controller.is_running() && !controller.should_stop() {
                controller.process_commands(Duration::from_millis(100));
            }
            
            if controller.should_stop() {
                println!("\n⏹ Strategy stopped by user");
                break;
            }
            
            let data_file = self.data_files[file_idx].clone();
            
            if file_idx > 0 {
                controller.notify_new_file();
            }
            
            println!("\n{}", "=".repeat(60));
            println!("Running {} on file [{}/{}]: {}", 
                     self.strategy.name(),
                     file_idx + 1, 
                     file_count, 
                     data_file.display());
            println!("{}\n", "=".repeat(60));
            
            self.run_single_file(
                data_file.to_str().unwrap(),
                &sender,
                &controller,
            )?;
        }
        
        if !controller.should_stop() {
            controller.mark_completed();
            println!("\n✅ All files processed successfully!");
        }
        
        self.keep_alive_until_close(&controller);
        
        Ok(())
    }

    fn run_single_file(
        &mut self,
        data_file: &str,
        sender: &Sender<PerformanceData>,
        controller: &StrategyController,
    ) -> Result<()> {
        println!("Loading data from: {}", data_file);
        
        let mut hbt = create_backtest(data_file)?;
        
        self.strategy.on_file_start(data_file);
        
        let mut state = StrategyState::new();
        let initial_capital = self.strategy.initial_capital();
        let update_interval = self.strategy.update_interval();
        let orderbook_depth = self.strategy.orderbook_depth();
        
        let mut last_gui_update = Instant::now();
        let mut last_command_check = Instant::now();
        let command_check_interval = Duration::from_millis(16);
        let mut data_ended = false;

        println!("{} started...\n", self.strategy.name());

        loop {
            if data_ended {
                println!("\nEnd of data reached!");
                self.strategy.on_file_end(&state);
                break;
            }
            
            while controller.state() == crate::controller::ControlState::Paused {
                controller.process_commands(Duration::from_millis(50));
                if controller.should_stop() {
                    return Ok(());
                }
            }
            
            if last_command_check.elapsed() >= command_check_interval {
                controller.process_commands(Duration::from_micros(COMMAND_POLL_TIMEOUT_MICROS));
                last_command_check = Instant::now();
                
                if controller.should_stop() {
                    println!("\n⏹ Strategy stopped by user");
                    break;
                }
            }
            
            let speed = controller.speed_multiplier();
            let (iterations_per_loop, loop_delay_ms) = calculate_speed_params(speed);
            
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
                        
                        state.update_count += 1;
                        
                        if state.update_count % update_interval == 0 {
                            let mut ctx = TickContext::new(&mut hbt);
                            state.mid_price = ctx.mid_price();
                            
                            if let Err(e) = self.strategy.on_tick(&mut ctx, &mut state) {
                                eprintln!("Strategy error: {:?}", e);
                            }
                        }
                    }
                    Err(_) => {
                        data_ended = true;
                        break;
                    }
                }
            }
            
            // Send data to GUI
            if last_gui_update.elapsed() >= Duration::from_millis(33) {
                let depth = hbt.depth(0);
                if is_valid_depth(depth) {
                    let (bids, asks) = extract_orderbook(depth, orderbook_depth);
                    let sim_time_secs = state.update_count as f64 * (ELAPSE_DURATION_NS as f64 / 1_000_000_000.0);
                    
                    let perf_data = build_performance_data(
                        &state,
                        initial_capital,
                        self.strategy.name(),
                        bids,
                        asks,
                        sim_time_secs,
                    );
                    
                    let _ = sender.try_send(perf_data);
                }
                last_gui_update = Instant::now();
            }
            
            if loop_delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(loop_delay_ms));
            } else {
                std::thread::yield_now();
            }
        }

        Ok(())
    }

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
}

fn calculate_speed_params(speed: f64) -> (usize, u64) {
    if speed >= 100.0 {
        (100, 0)
    } else if speed >= 10.0 {
        ((speed / 10.0).ceil() as usize, 1)
    } else if speed >= 1.0 {
        (1, (10.0 / speed) as u64)
    } else {
        (1, (10.0 / speed) as u64)
    }
}

fn create_backtest(data_file: &str) -> Result<Backtest<HashMapMarketDepth>> {
    let asset = L2AssetBuilder::new()
        .data(vec![DataSource::File(data_file.to_string())])
        .exchange(ExchangeKind::NoPartialFillExchange)
        .latency_model(ConstantLatency::new(50_000, 50_000))
        .fee_model(TradingValueFeeModel::new(CommonFees::new(-0.00005, 0.0007)))
        .queue_model(ProbQueueModel::new(PowerProbQueueFunc3::new(2.0)))
        .asset_type(LinearAsset::new(1.0))
        .depth(|| HashMapMarketDepth::new(TICK_SIZE, LOT_SIZE))
        .build()?;

    Ok(Backtest::builder()
        .add_asset(asset)
        .build()?)
}
