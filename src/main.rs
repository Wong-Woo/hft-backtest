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
    ORDER_SIZE, DEPTH_LEVELS, ORDER_LAYERS,
    PREDICTION_POSITION_SIZE, PREDICTION_STOP_LOSS_PCT, PREDICTION_TAKE_PROFIT_PCT,
    PREDICTION_CONFIDENCE_THRESHOLD, PREDICTION_LEARNING_RATE
};
use strategy::StrategyType;
use ui::launch_monitor_with_respawn;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("prediction");

    let strategy_type = match mode {
        "mm" | "market-maker" => {
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
            
            StrategyType::MarketMaker {
                gamma: GAMMA,
                initial_kappa: INITIAL_KAPPA,
                max_inventory: MAX_INVENTORY,
                volatility_threshold: VOLATILITY_THRESHOLD,
                order_size: ORDER_SIZE,
                depth_levels: DEPTH_LEVELS,
                order_layers: ORDER_LAYERS,
                initial_capital: INITIAL_CAPITAL,
            }
        }
        "momentum" => {
            println!("🚀 Momentum Trading Strategy with GUI Monitor\n");
            println!("Parameters:");
            println!("  Initial Capital: ${}", INITIAL_CAPITAL);
            println!("  Lookback Period: {}", MOMENTUM_LOOKBACK_PERIOD);
            println!("  Momentum Threshold: {} ({:.2}%)", MOMENTUM_THRESHOLD, MOMENTUM_THRESHOLD * 100.0);
            println!("  Position Size: {}", MOMENTUM_POSITION_SIZE);
            println!("  Stop Loss: {:.2}%", MOMENTUM_STOP_LOSS_PCT * 100.0);
            println!("  Take Profit: {:.2}%\n", MOMENTUM_TAKE_PROFIT_PCT * 100.0);
            
            StrategyType::Momentum {
                lookback_period: MOMENTUM_LOOKBACK_PERIOD,
                momentum_threshold: MOMENTUM_THRESHOLD,
                position_size: MOMENTUM_POSITION_SIZE,
                stop_loss_pct: MOMENTUM_STOP_LOSS_PCT,
                take_profit_pct: MOMENTUM_TAKE_PROFIT_PCT,
                initial_capital: INITIAL_CAPITAL,
            }
        }
        "predict" | "prediction" | "ml" => {
            println!("🧠 ML Price Prediction Strategy with GUI Monitor\n");
            println!("Parameters:");
            println!("  Initial Capital: ${}", INITIAL_CAPITAL);
            println!("  Position Size: {}", PREDICTION_POSITION_SIZE);
            println!("  Stop Loss: {:.2}%", PREDICTION_STOP_LOSS_PCT * 100.0);
            println!("  Take Profit: {:.2}%", PREDICTION_TAKE_PROFIT_PCT * 100.0);
            println!("  Prediction Confidence Threshold: {:.3}%", PREDICTION_CONFIDENCE_THRESHOLD * 100.0);
            println!("  Learning Rate: {}\n", PREDICTION_LEARNING_RATE);
            
            StrategyType::Prediction {
                position_size: PREDICTION_POSITION_SIZE,
                stop_loss_pct: PREDICTION_STOP_LOSS_PCT,
                take_profit_pct: PREDICTION_TAKE_PROFIT_PCT,
                initial_capital: INITIAL_CAPITAL,
                confidence_threshold: PREDICTION_CONFIDENCE_THRESHOLD,
                learning_rate: PREDICTION_LEARNING_RATE,
            }
        }
        _ => {
            println!("Usage: cargo run [mode]");
            println!("  Modes:");
            println!("    mm            - Run market making strategy with GUI monitor");
            println!("    market-maker  - Run market making strategy with GUI monitor");
            println!("    momentum      - Run momentum strategy with GUI monitor");
            println!("    predict       - Run ML prediction strategy with GUI monitor (default)");
            println!("    prediction    - Run ML prediction strategy with GUI monitor");
            println!("    ml            - Run ML prediction strategy with GUI monitor");
            return Ok(());
        }
    };

    let data_file_path = get_data_file_path();
    
    launch_monitor_with_respawn(
        strategy_type,
        INITIAL_CAPITAL,
        data_file_path,
    )
}