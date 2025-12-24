use anyhow::Result;
use hftbacktest::{
    backtest::{
        Backtest,
        ExchangeKind,
        L2AssetBuilder,
        assettype::LinearAsset,
        data::DataSource,
        models::{
            CommonFees,
            ConstantLatency,
            ProbQueueModel,
            PowerProbQueueFunc3,
            TradingValueFeeModel,
        },
    },
    prelude::{Bot, HashMapMarketDepth},
    depth::MarketDepth,
};

use crate::display::OrderBookDisplay;
use crate::config::{TICK_SIZE, LOT_SIZE};

/// 백테스팅 실행을 담당하는 구조체 (Single Responsibility Principle)
pub struct BacktestRunner {
    data_file: String,
    display: OrderBookDisplay,
}

impl BacktestRunner {
    pub fn new(data_file: String, display: OrderBookDisplay) -> Self {
        Self { data_file, display }
    }

    /// Run backtesting and display order book
    pub fn run(&self) -> Result<()> {
        println!("Loading data from: {}", self.data_file);

        // Setup and create backtester
        let mut hbt = self.create_backtest()?;

        println!("Backtesting started...\n");

        // Run full day backtest
        let mut update_count = 0;
        let mut display_count = 0;
        let elapse_duration = 100_000_000; // Check every 100ms
        let display_interval = 10; // Display every 10th update (every 1 second)

        // Wait for market data to populate
        println!("Waiting for market data to populate...\n");
        
        let mut market_ready = false;
        let start_time = std::time::Instant::now();
        
        loop {
            // Process events by elapsing time
            match hbt.elapse(elapse_duration) {
                Ok(_) => {
                    let depth = hbt.depth(0);
                    let best_bid_tick = depth.best_bid_tick();
                    let best_ask_tick = depth.best_ask_tick();
                    
                    update_count += 1;
                    
                    // Check if market data is valid
                    if best_bid_tick != i64::MIN && best_ask_tick != i64::MAX {
                        if !market_ready {
                            market_ready = true;
                            println!("Market data loaded! Starting display...\n");
                        }
                        
                        // Display order book at intervals
                        if update_count % display_interval == 0 {
                            display_count += 1;
                            
                            // Display order book
                            self.display.display(depth);
                            
                            let tick_size = depth.tick_size();
                            let best_bid = best_bid_tick as f64 * tick_size;
                            let best_ask = best_ask_tick as f64 * tick_size;
                            let spread = best_ask - best_bid;
                            let elapsed = start_time.elapsed();
                            
                            println!("\nTimestamp: {}", hbt.current_timestamp());
                            println!("Total updates: {} | Display count: {} | Elapsed: {:.2}s", 
                                     update_count, display_count, elapsed.as_secs_f64());
                            println!("Best Bid: {:.2} (tick: {})", best_bid, best_bid_tick);
                            println!("Best Ask: {:.2} (tick: {})", best_ask, best_ask_tick);
                            println!("Spread: {:.2} ({:.2}%)", spread, (spread / best_bid * 100.0));
                        }
                    }
                }
                Err(_) => {
                    println!("\nEnd of backtest data reached!");
                    break;
                }
            }
        }

        let total_elapsed = start_time.elapsed();
        println!("\n=== Backtest Complete! ===");
        println!("Total updates: {}", update_count);
        println!("Total displays: {}", display_count);
        println!("Elapsed time: {:.2}s", total_elapsed.as_secs_f64());
        println!("Processing speed: {:.0} updates/sec", update_count as f64 / total_elapsed.as_secs_f64());

        Ok(())
    }

    /// Create backtest instance (Dependency Inversion Principle)
    fn create_backtest(&self) -> Result<Backtest<HashMapMarketDepth>> {
        // Latency model: constant latency (entry: 100us, response: 100us)
        let latency_model = ConstantLatency::new(100_000, 100_000);
        
        // Asset type: linear asset (multiplier 1.0)
        let asset_type = LinearAsset::new(1.0);
        
        // Queue model: probability-based queue model
        let queue_model = ProbQueueModel::new(PowerProbQueueFunc3::new(3.0));
        
        // Fee model: maker -0.01%, taker 0.04%
        let fee_model = TradingValueFeeModel::new(CommonFees::new(-0.0001, 0.0004));

        let hbt = Backtest::builder()
            .add_asset(
                L2AssetBuilder::new()
                    .data(vec![DataSource::File(self.data_file.clone())])
                    .latency_model(latency_model)
                    .asset_type(asset_type)
                    .fee_model(fee_model)
                    .exchange(ExchangeKind::NoPartialFillExchange)
                    .queue_model(queue_model)
                    .depth(|| HashMapMarketDepth::new(TICK_SIZE, LOT_SIZE))
                    .build()?,
            )
            .build()?;

        Ok(hbt)
    }
}
