use anyhow::Result;
use crossbeam_channel::Sender;
use std::sync::Arc;
use crate::controller::StrategyController;
use crate::ui::PerformanceData;
use super::{MarketMakerRunner, MomentumRunner, PredictionRunner};

#[derive(Debug, Clone)]
pub enum StrategyType {
    MarketMaker {
        gamma: f64,
        initial_kappa: f64,
        max_inventory: f64,
        volatility_threshold: f64,
        order_size: f64,
        depth_levels: usize,
        order_layers: usize,
        initial_capital: f64,
    },
    Momentum {
        lookback_period: usize,
        momentum_threshold: f64,
        position_size: f64,
        stop_loss_pct: f64,
        take_profit_pct: f64,
        initial_capital: f64,
    },
    Prediction {
        position_size: f64,
        stop_loss_pct: f64,
        take_profit_pct: f64,
        initial_capital: f64,
        confidence_threshold: f64,
        learning_rate: f64,
    },
}

impl StrategyType {
    pub fn name(&self) -> &'static str {
        match self {
            StrategyType::MarketMaker { .. } => "Market Making",
            StrategyType::Momentum { .. } => "Momentum",
            StrategyType::Prediction { .. } => "ML Prediction",
        }
    }

    pub fn run(
        &self,
        data_files: Vec<String>,
        sender: Sender<PerformanceData>,
        controller: Arc<StrategyController>,
    ) -> Result<()> {
        match self {
            StrategyType::MarketMaker {
                gamma, initial_kappa, max_inventory, volatility_threshold,
                order_size, depth_levels, order_layers, initial_capital,
            } => {
                let mut runner = MarketMakerRunner::new_with_files(
                    data_files,
                    *gamma, *initial_kappa, *max_inventory, *volatility_threshold,
                    *order_size, *depth_levels, *order_layers, *initial_capital,
                )?;
                runner.run_with_controller(sender, controller)
            }
            StrategyType::Momentum {
                lookback_period, momentum_threshold, position_size,
                stop_loss_pct, take_profit_pct, initial_capital,
            } => {
                let mut runner = MomentumRunner::new_with_files(
                    data_files,
                    *lookback_period, *momentum_threshold, *position_size,
                    *stop_loss_pct, *take_profit_pct, *initial_capital,
                )?;
                runner.run_with_controller(sender, controller)
            }
            StrategyType::Prediction {
                position_size, stop_loss_pct, take_profit_pct,
                initial_capital, confidence_threshold, learning_rate,
            } => {
                let mut runner = PredictionRunner::new_with_files(
                    data_files,
                    *position_size, *stop_loss_pct, *take_profit_pct,
                    *initial_capital, *confidence_threshold, *learning_rate,
                )?;
                runner.run_with_controller(sender, controller)
            }
        }
    }
}
