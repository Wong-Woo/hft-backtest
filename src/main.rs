mod config;
mod common;
mod strategy;
mod controller;
mod ui;

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
    use ui::launch_monitor;
    use controller::StrategyController;
    use std::thread;
    use std::sync::Arc;

    println!("🚀 Momentum Trading Strategy with GUI Monitor\n");
    println!("Parameters:");
    println!("  Initial Capital: ${}", INITIAL_CAPITAL);
    println!("  Lookback Period: {}", MOMENTUM_LOOKBACK_PERIOD);
    println!("  Momentum Threshold: {} ({:.2}%)", MOMENTUM_THRESHOLD, MOMENTUM_THRESHOLD * 100.0);
    println!("  Position Size: {}", MOMENTUM_POSITION_SIZE);
    println!("  Stop Loss: {:.2}%", MOMENTUM_STOP_LOSS_PCT * 100.0);
    println!("  Take Profit: {:.2}%\n", MOMENTUM_TAKE_PROFIT_PCT * 100.0);

    // 채널 생성
    let (data_tx, data_rx) = unbounded();
    let (cmd_tx, cmd_rx) = unbounded();
    let (response_tx, response_rx) = unbounded();

    // Controller 생성
    let controller = Arc::new(StrategyController::new(cmd_rx, response_tx.clone()));
    let controller_clone = Arc::clone(&controller);

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
        runner.run_with_controller(data_tx, controller_clone)?;
        Ok(())
    });

    // 메인 스레드에서 GUI 실행
    let gui_result = launch_monitor(
        data_rx,
        response_rx,
        cmd_tx,
        INITIAL_CAPITAL,
        "Momentum",
        DATA_FILE_PATH.to_string(),
    );

    // GUI 종료 시 전략도 종료하도록 Stop 명령 전송
    let _ = controller.should_stop();

    // 전략 스레드 종료 대기
    let _ = strategy_thread.join();

    gui_result
}

fn run_market_maker_with_gui() -> Result<()> {
    use crossbeam_channel::unbounded;
    use ui::launch_monitor;
    use controller::StrategyController;
    use std::thread;
    use std::sync::Arc;
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
    let (data_tx, data_rx) = unbounded();
    let (cmd_tx, cmd_rx) = unbounded();
    let (response_tx, response_rx) = unbounded();

    // Controller 생성
    let controller = Arc::new(StrategyController::new(cmd_rx, response_tx.clone()));
    let controller_clone = Arc::clone(&controller);

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
        runner.run_with_controller(data_tx, controller_clone)?;
        Ok(())
    });

    // 메인 스레드에서 GUI 실행
    let gui_result = launch_monitor(
        data_rx,
        response_rx,
        cmd_tx,
        INITIAL_CAPITAL,
        "Market Making",
        DATA_FILE_PATH.to_string(),
    );

    // GUI 종료 시 전략도 종료하도록 Stop 명령 전송
    let _ = controller.should_stop();

    // 전략 스레드 종료 대기
    let _ = strategy_thread.join();

    gui_result
}