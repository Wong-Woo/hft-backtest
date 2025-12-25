mod config;
mod common;
mod strategy;
mod monitor;

use anyhow::Result;
use config::{
    DATA_FILE_PATH, INITIAL_CAPITAL,
    MOMENTUM_LOOKBACK_PERIOD, MOMENTUM_THRESHOLD, MOMENTUM_POSITION_SIZE,
    MOMENTUM_STOP_LOSS_PCT, MOMENTUM_TAKE_PROFIT_PCT,
};
use strategy::{MarketMakerRunner, MomentumRunner};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("momentum");

    match mode {
        "mm" | "market-maker" => run_market_maker_with_gui(),
        "momentum" => run_momentum_with_gui(),
        _ => {
            println!("Usage: cargo run [mode]");
            println!("  Modes:");
            println!("    mm            - Run market making strategy with GUI monitor");
            println!("    market-maker  - Run market making strategy with GUI monitor");
            println!("    momentum      - Run momentum strategy with GUI monitor (default)");
            Ok(())
        }
    }
}

fn run_momentum_with_gui() -> Result<()> {
    use crossbeam_channel::unbounded;
    use monitor::{launch_monitor, PerformanceData};
    use std::thread;

    println!("🚀 Momentum Trading Strategy with GUI Monitor\n");
    println!("Parameters:");
    println!("  Initial Capital: ${}", INITIAL_CAPITAL);
    println!("  Lookback Period: {}", MOMENTUM_LOOKBACK_PERIOD);
    println!("  Momentum Threshold: {} ({:.2}%)", MOMENTUM_THRESHOLD, MOMENTUM_THRESHOLD * 100.0);
    println!("  Position Size: {}", MOMENTUM_POSITION_SIZE);
    println!("  Stop Loss: {:.2}%", MOMENTUM_STOP_LOSS_PCT * 100.0);
    println!("  Take Profit: {:.2}%\n", MOMENTUM_TAKE_PROFIT_PCT * 100.0);

    // 채널 생성
    let (sender, receiver) = unbounded::<PerformanceData>();

    // 전략을 별도 스레드에서 실행
    let strategy_thread = thread::spawn(move || -> Result<()> {
        let mut runner = MomentumRunner::new(
            DATA_FILE_PATH.to_string(),
            MOMENTUM_LOOKBACK_PERIOD,
            MOMENTUM_THRESHOLD,
            MOMENTUM_POSITION_SIZE,
            MOMENTUM_STOP_LOSS_PCT,
            MOMENTUM_TAKE_PROFIT_PCT,
            INITIAL_CAPITAL,
        )?;
        runner.run_with_monitor(sender)?;
        Ok(())
    });

    // 메인 스레드에서 GUI 실행
    let gui_result = launch_monitor(receiver, INITIAL_CAPITAL, "Momentum");

    // 전략 스레드 종료 대기
    let _ = strategy_thread.join();

    gui_result
}

fn run_market_maker_with_gui() -> Result<()> {
    use crossbeam_channel::unbounded;
    use monitor::{launch_monitor, PerformanceData};
    use std::thread;
    use config::{GAMMA, INITIAL_KAPPA, MAX_INVENTORY, VOLATILITY_THRESHOLD,
                 ORDER_SIZE, DEPTH_LEVELS, ORDER_LAYERS};

    println!("🚀 Limit Order Market Making Strategy with GUI Monitor\n");
    println!("Parameters:");
    println!("  Initial Capital: ${}", INITIAL_CAPITAL);
    println!("  Gamma (γ): {}", GAMMA);
    println!("  Initial Kappa (k): {}", INITIAL_KAPPA);
    println!("  Max Inventory: {}", MAX_INVENTORY);
    println!("  Volatility Threshold: {}", VOLATILITY_THRESHOLD);
    println!("  Order Size: {}", ORDER_SIZE);
    println!("  Depth Levels: {}", DEPTH_LEVELS);
    println!("  Order Layers: {}\n", ORDER_LAYERS);

    // 채널 생성
    let (sender, receiver) = unbounded::<PerformanceData>();

    // 전략을 별도 스레드에서 실행
    let strategy_thread = thread::spawn(move || -> Result<()> {
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
        runner.run_with_monitor(sender)?;
        Ok(())
    });

    // 메인 스레드에서 GUI 실행
    let gui_result = launch_monitor(receiver, INITIAL_CAPITAL, "Market Making");

    // 전략 스레드 종료 대기
    let _ = strategy_thread.join();

    gui_result
}