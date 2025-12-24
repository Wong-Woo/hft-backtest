mod config;
mod display;
mod backtest;

use anyhow::Result;
use config::{DATA_FILE_PATH, ASK_DEPTH_LEVELS, BID_DEPTH_LEVELS};
use display::OrderBookDisplay;
use backtest::BacktestRunner;

fn main() -> Result<()> {
    println!("🚀 HFT Backtesting with 20-Depth Order Book Display\n");

    // Dependency injection (SOLID principle)
    let display = OrderBookDisplay::new(ASK_DEPTH_LEVELS, BID_DEPTH_LEVELS);
    let runner = BacktestRunner::new(DATA_FILE_PATH.to_string(), display);

    // Run backtesting
    runner.run()?;

    Ok(())
}