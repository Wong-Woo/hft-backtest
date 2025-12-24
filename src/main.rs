mod config;
mod display;
mod print_depth;
mod common;
mod strategy;

use anyhow::Result;
use config::{
    DATA_FILE_PATH, ASK_DEPTH_LEVELS, BID_DEPTH_LEVELS,
    GAMMA, INITIAL_KAPPA, MAX_INVENTORY, VOLATILITY_THRESHOLD,
    ORDER_SIZE, DEPTH_LEVELS, ORDER_LAYERS, INITIAL_CAPITAL,
    MOMENTUM_LOOKBACK_PERIOD, MOMENTUM_THRESHOLD, MOMENTUM_POSITION_SIZE,
    MOMENTUM_STOP_LOSS_PCT, MOMENTUM_TAKE_PROFIT_PCT,
};
use display::OrderBookDisplay;
use print_depth::PrintDepthRunner;
use strategy::{MarketMakerRunner, MomentumRunner};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("print");

    match mode {
        "print" => run_print_depth(),
        "mm" | "market-maker" => run_market_maker(),
        "momentum" => run_momentum(),
        _ => {
            println!("Usage: cargo run [mode]");
            println!("  Modes:");
            println!("    print          - Print order book depth (default)");
            println!("    mm             - Run market making strategy");
            println!("    market-maker   - Run market making strategy");
            println!("    momentum       - Run momentum strategy");
            Ok(())
        }
    }
}

fn run_print_depth() -> Result<()> {
    println!("🚀 Print Order Book Depth from Market Data\n");

    let display = OrderBookDisplay::new(ASK_DEPTH_LEVELS, BID_DEPTH_LEVELS);
    let runner = PrintDepthRunner::new(DATA_FILE_PATH.to_string(), display)?;

    runner.run()?;

    Ok(())
}
fn run_momentum() -> Result<()> {
    println!("🚀 Momentum Trading Strategy\n");
    println!("Parameters:");
    println!("  Initial Capital: ${}", INITIAL_CAPITAL);
    println!("  Lookback Period: {}", MOMENTUM_LOOKBACK_PERIOD);
    println!("  Momentum Threshold: {} ({:.2}%)", MOMENTUM_THRESHOLD, MOMENTUM_THRESHOLD * 100.0);
    println!("  Position Size: {}", MOMENTUM_POSITION_SIZE);
    println!("  Stop Loss: {:.2}%", MOMENTUM_STOP_LOSS_PCT * 100.0);
    println!("  Take Profit: {:.2}%\n", MOMENTUM_TAKE_PROFIT_PCT * 100.0);

    let mut runner = MomentumRunner::new(
        DATA_FILE_PATH.to_string(),
        MOMENTUM_LOOKBACK_PERIOD,
        MOMENTUM_THRESHOLD,
        MOMENTUM_POSITION_SIZE,
        MOMENTUM_STOP_LOSS_PCT,
        MOMENTUM_TAKE_PROFIT_PCT,
        INITIAL_CAPITAL,
    )?;

    runner.run()?;

    Ok(())
}
fn run_market_maker() -> Result<()> {
    println!("🚀 Limit Order Market Making Strategy\n");
    println!("Parameters:");
    println!("  Initial Capital: ${}", INITIAL_CAPITAL);
    println!("  Gamma (γ): {}", GAMMA);
    println!("  Initial Kappa (k): {}", INITIAL_KAPPA);
    println!("  Max Inventory: {}", MAX_INVENTORY);
    println!("  Volatility Threshold: {}", VOLATILITY_THRESHOLD);
    println!("  Order Size: {}", ORDER_SIZE);
    println!("  Depth Levels: {}", DEPTH_LEVELS);
    println!("  Order Layers: {}\n", ORDER_LAYERS);

    let mut runner = MarketMakerRunner::new(
        DATA_FILE_PATH.to_string(),
        GAMMA,
        INITIAL_KAPPA,
        MAX_INVENTORY,
        VOLATILITY_THRESHOLD,
        ORDER_SIZE,
        DEPTH_LEVELS,
        ORDER_LAYERS,
        INITIAL_CAPITAL,
    )?;

    runner.run()?;

    Ok(())
}