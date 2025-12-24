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
use std::path::PathBuf;

use crate::display::OrderBookDisplay;
use crate::config::{TICK_SIZE, LOT_SIZE};
use crate::common::DataLoader;

/// Depth 출력을 담당하는 구조체 (Single Responsibility Principle)
pub struct PrintDepthRunner {
    data_files: Vec<PathBuf>,
    display: OrderBookDisplay,
}

impl PrintDepthRunner {
    pub fn new(data_pattern: String, display: OrderBookDisplay) -> Result<Self> {
        let data_files = DataLoader::load_files(&data_pattern)?;
        
        Ok(Self { 
            data_files,
            display 
        })
    }

    /// Print depth for all matched files
    pub fn run(&self) -> Result<()> {
        for (file_idx, data_file) in self.data_files.iter().enumerate() {
            println!("\n{}", "=".repeat(60));
            println!("Processing file [{}/{}]: {}", 
                     file_idx + 1, 
                     self.data_files.len(), 
                     data_file.display());
            println!("{}\n", "=".repeat(60));
            
            self.run_single_file(data_file.to_str().unwrap())?;
        }
        
        println!("\n✅ All files processed successfully!");
        Ok(())
    }
    
    /// Print depth for a single file
    fn run_single_file(&self, data_file: &str) -> Result<()> {
        println!("Loading data from: {}", data_file);

        // Setup and create depth reader
        let mut hbt = self.create_backtest(data_file)?;

        println!("Print depth started...\n");

        // Read market data
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
                    println!("\nEnd of data reached!");
                    break;
                }
            }
        }

        let total_elapsed = start_time.elapsed();
        println!("\n=== Print Complete! ===");
        println!("Total updates: {}", update_count);
        println!("Total displays: {}", display_count);
        println!("Elapsed time: {:.2}s", total_elapsed.as_secs_f64());
        println!("Processing speed: {:.0} updates/sec", update_count as f64 / total_elapsed.as_secs_f64());

        Ok(())
    }

    /// Create depth reader instance (Dependency Inversion Principle)
    fn create_backtest(&self, data_file: &str) -> Result<Backtest<HashMapMarketDepth>> {
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
                    .data(vec![DataSource::File(data_file.to_string())])
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
