mod config;
mod common;
mod strategy;
mod controller;
mod ui;

use anyhow::Result;
use config::{
    get_data_file_path, INITIAL_CAPITAL,
    MOMENTUM_LOOKBACK_PERIOD, MOMENTUM_THRESHOLD, MOMENTUM_POSITION_SIZE,
    MOMENTUM_STOP_LOSS_PCT, MOMENTUM_TAKE_PROFIT_PCT,
    GAMMA, INITIAL_KAPPA, MAX_INVENTORY, VOLATILITY_THRESHOLD,
    ORDER_SIZE, DEPTH_LEVELS, ORDER_LAYERS
};
use strategy::{MarketMakerRunner, MomentumRunner};
use std::sync::Arc;
use std::thread;
use crossbeam_channel::unbounded;
use ui::{launch_monitor, PerformanceData};
use controller::StrategyController;

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

fn run_strategy_with_gui<F>(strategy_name: &str, run_strategy: F) -> Result<()>
where
    F: FnOnce(crossbeam_channel::Sender<PerformanceData>, Arc<StrategyController>) -> Result<()> + Send + 'static,
{
    let data_file_path = get_data_file_path();

    // Create channels for communication
    let (data_tx, data_rx) = unbounded();
    let (cmd_tx, cmd_rx) = unbounded();
    let (response_tx, response_rx) = unbounded();

    // Create controller
    let controller = Arc::new(StrategyController::new(cmd_rx, response_tx.clone()));
    let controller_clone = Arc::clone(&controller);

    // Run strategy in separate thread
    let strategy_thread = thread::spawn(move || -> Result<()> {
        run_strategy(data_tx, controller_clone)
    });

    // Run GUI in main thread
    let gui_result = launch_monitor(
        data_rx,
        response_rx,
        cmd_tx,
        INITIAL_CAPITAL,
        strategy_name,
        data_file_path,
    );

    // Signal stop to strategy when GUI closes
    controller.stop();

    // Wait for strategy thread to finish
    let _ = strategy_thread.join();

    gui_result
}

fn run_momentum_with_gui() -> Result<()> {
    println!("🚀 Momentum Trading Strategy with GUI Monitor\n");
    println!("Parameters:");
    println!("  Initial Capital: ${}", INITIAL_CAPITAL);
    println!("  Lookback Period: {}", MOMENTUM_LOOKBACK_PERIOD);
    println!("  Momentum Threshold: {} ({:.2}%)", MOMENTUM_THRESHOLD, MOMENTUM_THRESHOLD * 100.0);
    println!("  Position Size: {}", MOMENTUM_POSITION_SIZE);
    println!("  Stop Loss: {:.2}%", MOMENTUM_STOP_LOSS_PCT * 100.0);
    println!("  Take Profit: {:.2}%\n", MOMENTUM_TAKE_PROFIT_PCT * 100.0);

    let data_file_path = get_data_file_path();
    let data_file_clone = data_file_path.clone();

    run_strategy_with_gui("Momentum", move |data_tx, controller| {
        let mut runner = MomentumRunner::new(
            data_file_clone,
            MOMENTUM_LOOKBACK_PERIOD,
            MOMENTUM_THRESHOLD,
            MOMENTUM_POSITION_SIZE,
            MOMENTUM_STOP_LOSS_PCT,
            MOMENTUM_TAKE_PROFIT_PCT,
            INITIAL_CAPITAL,
        )?;
        runner.run_with_controller(data_tx, controller)?;
        Ok(())
    })
}

fn run_market_maker_with_gui() -> Result<()> {
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

    let data_file_path = get_data_file_path();
    let data_file_clone = data_file_path.clone();

    run_strategy_with_gui("Market Making", move |data_tx, controller| {
        let mut runner = MarketMakerRunner::new(
            data_file_clone,
            GAMMA,
            INITIAL_KAPPA,
            MAX_INVENTORY,
            VOLATILITY_THRESHOLD,
            ORDER_SIZE,
            DEPTH_LEVELS,
            ORDER_LAYERS,
            INITIAL_CAPITAL,
        )?;
        runner.run_with_controller(data_tx, controller)?;
        Ok(())
    })
}