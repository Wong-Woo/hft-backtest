use anyhow::Result;
use hftbacktest::{
    backtest::{Backtest, BacktestError, ExchangeKind, L2AssetBuilder, assettype::LinearAsset,
        data::DataSource, models::{CommonFees, ConstantLatency, ProbQueueModel, 
        PowerProbQueueFunc3, TradingValueFeeModel}},
    prelude::{Bot, HashMapMarketDepth, Status, TimeInForce, OrdType},
    depth::MarketDepth,
};
use std::path::PathBuf;
use crossbeam_channel::Sender;
use crate::common::{DataLoader, calculate_mid_price, is_valid_depth};
use crate::config::{TICK_SIZE, LOT_SIZE, ELAPSE_DURATION_NS, UPDATE_INTERVAL};
use crate::monitor::PerformanceData;
use super::{MicroPriceCalculator, OrderBookImbalance, SpreadCalculator,
    RiskManager, OrderTracker, OrderSide};

pub struct MarketMakerRunner {
    data_files: Vec<PathBuf>,
    micro_price_calc: MicroPriceCalculator,
    imbalance_calc: OrderBookImbalance,
    spread_calc: SpreadCalculator,
    risk_manager: RiskManager,
    order_tracker: OrderTracker,
    order_size: f64,
    order_layers: usize,
    initial_capital: f64,
}

impl MarketMakerRunner {
    pub fn new(
        data_pattern: String,
        gamma: f64,
        _initial_kappa: f64,
        max_inventory: f64,
        volatility_threshold: f64,
        order_size: f64,
        depth_levels: usize,
        order_layers: usize,
        initial_capital: f64,
    ) -> Result<Self> {
        let data_files = DataLoader::load_files(&data_pattern)?;

        Ok(Self {
            data_files,
            micro_price_calc: MicroPriceCalculator::new(depth_levels),
            imbalance_calc: OrderBookImbalance::new(depth_levels),
            spread_calc: SpreadCalculator::new(gamma),
            risk_manager: RiskManager::new(max_inventory, volatility_threshold, 60),
            order_tracker: OrderTracker::new(),
            order_size,
            order_layers,
            initial_capital,
        })
    }

    /// GUI 모니터와 함께 전략 실행
    pub fn run_with_monitor(&mut self, sender: Sender<PerformanceData>) -> Result<()> {
        let file_count = self.data_files.len();
        
        for file_idx in 0..file_count {
            let data_file = self.data_files[file_idx].clone();
            
            println!("\n{}", "=".repeat(60));
            println!("Running strategy on file [{}/{}]: {}", 
                     file_idx + 1, 
                     file_count, 
                     data_file.display());
            println!("{}\n", "=".repeat(60));
            
            self.run_strategy(data_file.to_str().unwrap(), Some(&sender))?;
        }
        
        println!("\n✅ All files processed successfully!");
        Ok(())
    }

    /// 단일 파일에 대한 전략 실행
    fn run_strategy(&mut self, data_file: &str, sender: Option<&Sender<PerformanceData>>) -> Result<()> {
        println!("Loading data from: {}", data_file);

        let mut hbt = self.create_backtest(data_file)?;
        
        println!("Market making strategy started...\n");

        let mut inventory = 0.0;
        let mut realized_pnl = 0.0;
        let cash = self.initial_capital;
        let mut initial_price = 0.0;
        let mut update_count = 0;
        let mut initial_orders_placed = false;

        println!("Waiting for market data...\n");

        loop {
            match hbt.elapse(ELAPSE_DURATION_NS) {
                Ok(_) => {
                    let depth = hbt.depth(0);
                    
                    if !is_valid_depth(depth) {
                        continue;
                    }

                    update_count += 1;
                    
                    if initial_price == 0.0 {
                        initial_price = calculate_mid_price(depth);
                        println!("Initial price set: {:.2}\n", initial_price);
                        
                        let _ = depth;
                        self.place_initial_orders(&mut hbt)?;
                        initial_orders_placed = true;
                        continue;
                    }
                    
                    if !initial_orders_placed {
                        let _ = depth;
                        self.place_initial_orders(&mut hbt)?;
                        initial_orders_placed = true;
                        continue;
                    }

                    if update_count % UPDATE_INTERVAL == 0 {
                        let _ = depth;
                        self.check_and_refill_orders(&mut hbt, &mut inventory, &mut realized_pnl)?;
                        
                        // GUI로 데이터 전송
                        if let Some(sender) = sender {
                            let depth_for_data = hbt.depth(0);
                            let mid_price = calculate_mid_price(depth_for_data);
                            let inventory_value = inventory * mid_price;
                            let unrealized_pnl = inventory * (mid_price - initial_price);
                            
                            let _ = sender.send(PerformanceData {
                                timestamp: update_count as f64,
                                equity: cash + realized_pnl + inventory_value,
                                realized_pnl,
                                unrealized_pnl,
                                position: inventory,
                                mid_price,
                                strategy_name: "Market Making".to_string(),
                            });
                        }
                        
                        let depth_for_print = hbt.depth(0);
                        self.print_status(
                            update_count as u64, 
                            inventory, 
                            realized_pnl, 
                            cash,
                            initial_price,
                            depth_for_print
                        );
                    }
                }
                Err(_) => {
                    println!("\nEnd of data reached!");
                    break;
                }
            }
        }

        let final_depth = hbt.depth(0);
        self.print_final_stats(inventory, realized_pnl, cash, initial_price, final_depth);

        Ok(())
    }

    fn check_and_refill_orders<MD>(
        &mut self,
        hbt: &mut Backtest<MD>,
        inventory: &mut f64,
        realized_pnl: &mut f64,
    ) -> Result<(), BacktestError>
    where
        MD: MarketDepth,
    {
        let depth = hbt.depth(0);
        let tick_size = depth.tick_size();
        
        let orders = hbt.orders(0);
        let mut filled_orders = Vec::new();
        let mut expired_orders = Vec::new();
        
        for layer in 0..self.order_layers {
            let buy_order_id = (layer * 2) as u64;
            let sell_order_id = (layer * 2 + 1) as u64;
            
            if let Some(order) = orders.get(&buy_order_id) {
                if order.status == Status::Filled {
                    let fill_price = order.price_tick as f64 * tick_size;
                    let fill_qty = order.qty;
                    
                    *inventory += fill_qty;
                    
                    let cost = fill_price * fill_qty;
                    let fee = cost * 0.0001;
                    *realized_pnl -= cost;
                    *realized_pnl += fee;
                    
                    filled_orders.push((buy_order_id, OrderSide::Buy, fill_price, fill_qty, layer));
                    
                    println!("  ✓ BUY  filled @ {:.2} qty {:.4} | Layer {} | Cost: -{:.2} + Fee: +{:.4}", 
                             fill_price, fill_qty, layer + 1, cost, fee);
                    
                    self.order_tracker.mark_filled(buy_order_id);
                } else if order.status == Status::Expired || order.status == Status::Canceled {
                    expired_orders.push((buy_order_id, OrderSide::Buy, layer));
                }
            } else {
                expired_orders.push((buy_order_id, OrderSide::Buy, layer));
            }
            
            if let Some(order) = orders.get(&sell_order_id) {
                if order.status == Status::Filled {
                    let fill_price = order.price_tick as f64 * tick_size;
                    let fill_qty = order.qty;
                    
                    *inventory -= fill_qty;
                    
                    let revenue = fill_price * fill_qty;
                    let fee = revenue * 0.0001;
                    *realized_pnl += revenue;
                    *realized_pnl += fee;
                    
                    filled_orders.push((sell_order_id, OrderSide::Sell, fill_price, fill_qty, layer));
                    
                    println!("  ✓ SELL filled @ {:.2} qty {:.4} | Layer {} | Revenue: +{:.2} + Fee: +{:.4}", 
                             fill_price, fill_qty, layer + 1, revenue, fee);
                    
                    self.order_tracker.mark_filled(sell_order_id);
                } else if order.status == Status::Expired || order.status == Status::Canceled {
                    expired_orders.push((sell_order_id, OrderSide::Sell, layer));
                }
            } else {
                expired_orders.push((sell_order_id, OrderSide::Sell, layer));
            }
        }
        
        let orders_to_resubmit: Vec<_> = filled_orders.into_iter()
            .map(|(id, side, _, _, layer)| (id, side, layer, true))
            .chain(expired_orders.into_iter()
                .map(|(id, side, layer)| (id, side, layer, false)))
            .collect();
        
        if !orders_to_resubmit.is_empty() {
            if orders_to_resubmit.iter().any(|(_, _, _, filled)| *filled) {
                println!("  → Refilling {} filled order(s)...", 
                         orders_to_resubmit.iter().filter(|(_, _, _, f)| *f).count());
            }
            
            let micro_price = self.micro_price_calc.calculate(depth);
            let imbalance = self.imbalance_calc.calculate(depth);
            let volatility = self.risk_manager.calculate_volatility();
            
            let reservation_price = self.spread_calc.calculate_reservation_price(
                micro_price, *inventory, volatility
            );
            
            let fixed_spread = crate::config::FIXED_SPREAD_TICKS * tick_size;
            let half_spread = fixed_spread / 2.0;
            let imbalance_adjustment = imbalance * half_spread * 0.1;
            
            let adjusted_size = self.risk_manager.adjust_order_size(self.order_size, *inventory);
            
            for (order_id, side, layer, _) in orders_to_resubmit {
                let layer_offset = layer as f64 * 1.0 * tick_size;
                let layer_size = adjusted_size / (1.0 + layer as f64 * 0.5);
                
                match side {
                    OrderSide::Buy => {
                        let bid_price = reservation_price - half_spread - layer_offset + imbalance_adjustment;
                        let bid_tick = (bid_price / tick_size).round() as i64;
                        
                        if let Ok(_) = hbt.submit_buy_order(
                            0, 
                            order_id, 
                            bid_tick as f64,
                            layer_size, 
                            TimeInForce::GTX,
                            OrdType::Limit, 
                            false
                        ) {
                            self.order_tracker.register_order(order_id, OrderSide::Buy, bid_price, layer_size, layer);
                        }
                    }
                    OrderSide::Sell => {
                        let ask_price = reservation_price + half_spread + layer_offset - imbalance_adjustment;
                        let ask_tick = (ask_price / tick_size).round() as i64;
                        
                        if let Ok(_) = hbt.submit_sell_order(
                            0, 
                            order_id, 
                            ask_tick as f64,
                            layer_size, 
                            TimeInForce::GTX,
                            OrdType::Limit, 
                            false
                        ) {
                            self.order_tracker.register_order(order_id, OrderSide::Sell, ask_price, layer_size, layer);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    fn place_initial_orders<MD>(
        &mut self,
        hbt: &mut Backtest<MD>,
    ) -> Result<(), BacktestError>
    where
        MD: MarketDepth,
    {
        let depth = hbt.depth(0);
        let tick_size = depth.tick_size();
        
        let best_bid_price = depth.best_bid_tick() as f64 * tick_size;
        let best_ask_price = depth.best_ask_tick() as f64 * tick_size;
        let market_spread = best_ask_price - best_bid_price;
        
        let micro_price = self.micro_price_calc.calculate(depth);
        let imbalance = self.imbalance_calc.calculate(depth);
        
        let fixed_spread = crate::config::FIXED_SPREAD_TICKS * tick_size;
        let half_spread = fixed_spread / 2.0;
        
        let volatility = self.risk_manager.calculate_volatility();
        let inventory = 0.0;
        let reservation_price = self.spread_calc.calculate_reservation_price(
            micro_price, inventory, volatility
        );
        
        let imbalance_adjustment = imbalance * half_spread * 0.1;
        
        println!("  Initial Order Submission:");
        println!("    Market: Bid {:.2} | Ask {:.2} | Spread {:.2}", 
                 best_bid_price, best_ask_price, market_spread);
        println!("    Micro Price: {:.2}, Reservation: {:.2}, Fixed Spread: {:.4}", 
                 micro_price, reservation_price, fixed_spread);
        
        for layer in 0..self.order_layers {
            let layer_offset = layer as f64 * 1.0 * tick_size;
            let layer_size = self.order_size / (1.0 + layer as f64 * 0.5);
            
            let bid_price = reservation_price - half_spread - layer_offset + imbalance_adjustment;
            let bid_tick = (bid_price / tick_size).round() as i64;
            let buy_order_id = (layer * 2) as u64;
            
            if let Ok(_) = hbt.submit_buy_order(
                0,
                buy_order_id,
                bid_tick as f64,
                layer_size,
                TimeInForce::GTX,
                OrdType::Limit,
                false,
            ) {
                self.order_tracker.register_order(buy_order_id, OrderSide::Buy, bid_price, layer_size, layer);
                println!("    → BUY  Layer {} @ {:.2} (tick {}) qty {:.4}", 
                         layer + 1, bid_price, bid_tick, layer_size);
            }
            
            let ask_price = reservation_price + half_spread + layer_offset - imbalance_adjustment;
            let ask_tick = (ask_price / tick_size).round() as i64;
            let sell_order_id = (layer * 2 + 1) as u64;
            
            if let Ok(_) = hbt.submit_sell_order(
                0,
                sell_order_id,
                ask_tick as f64,
                layer_size,
                TimeInForce::GTX,
                OrdType::Limit,
                false,
            ) {
                self.order_tracker.register_order(sell_order_id, OrderSide::Sell, ask_price, layer_size, layer);
                println!("    → SELL Layer {} @ {:.2} (tick {}) qty {:.4}", 
                         layer + 1, ask_price, ask_tick, layer_size);
            }
        }
        
        Ok(())
    }

    fn print_status(
        &self,
        update_count: u64,
        inventory: f64,
        realized_pnl: f64,
        cash: f64,
        initial_price: f64,
        depth: &dyn MarketDepth,
    ) {
        let tick_size = depth.tick_size();
        let best_bid = depth.best_bid_tick() as f64 * tick_size;
        let best_ask = depth.best_ask_tick() as f64 * tick_size;
        let spread = best_ask - best_bid;
        let current_price = (best_bid + best_ask) / 2.0;
        
        let inventory_value = inventory * current_price;
        let portfolio_value = cash + inventory_value;
        
        let return_pct = ((portfolio_value - self.initial_capital) / self.initial_capital) * 100.0;
        let unrealized_pnl = inventory * (current_price - initial_price);
        let total_pnl = realized_pnl + unrealized_pnl;
        
        let micro_price = self.micro_price_calc.calculate(depth);
        let imbalance = self.imbalance_calc.calculate(depth);
        let volatility = self.risk_manager.calculate_volatility();
        
        let (filled_count, buy_vol, sell_vol, active_count) = self.order_tracker.get_stats();
        
        println!("\n--- Strategy Status (Update: {}) ---", update_count);
        println!("  Market: Bid {:.2} | Ask {:.2} | Spread {:.2}", best_bid, best_ask, spread);
        println!("  Micro Price: {:.2} | Imbalance: {:.4}", micro_price, imbalance);
        println!("  Volatility: {:.6}", volatility);
        println!("  Inventory: {:.4} | Cash: {:.2}", inventory, cash);
        println!("  Realized PnL: {:.2} | Unrealized PnL: {:.2} | Total PnL: {:.2}", 
                 realized_pnl, unrealized_pnl, total_pnl);
        println!("  Portfolio Value: {:.2} | Return: {:.4}%", portfolio_value, return_pct);
        println!("  Orders: Active {} | Filled {} | Buy Vol {:.4} | Sell Vol {:.4}", 
                 active_count, filled_count, buy_vol, sell_vol);
    }

    fn print_final_stats(
        &self, 
        inventory: f64, 
        realized_pnl: f64,
        cash: f64,
        initial_price: f64,
        depth: &dyn MarketDepth,
    ) {
        let tick_size = depth.tick_size();
        let best_bid = depth.best_bid_tick() as f64 * tick_size;
        let best_ask = depth.best_ask_tick() as f64 * tick_size;
        let final_price = (best_bid + best_ask) / 2.0;
        
        let inventory_value = inventory * final_price;
        let portfolio_value = cash + inventory_value;
        
        let return_pct = ((portfolio_value - self.initial_capital) / self.initial_capital) * 100.0;
        let unrealized_pnl = inventory * (final_price - initial_price);
        let total_pnl = realized_pnl + unrealized_pnl;
        
        println!("\n{}", "=".repeat(60));
        println!("=== Strategy Complete ===");
        println!("  Initial Capital: ${:.2}", self.initial_capital);
        println!("  Final Cash: ${:.2}", cash);
        println!("  Final Inventory: {:.4} @ ${:.2}", inventory, final_price);
        println!("  Inventory Value: ${:.2}", inventory_value);
        println!("  Final Portfolio Value: ${:.2}", portfolio_value);
        println!("");
        println!("  Realized PnL: ${:.2}", realized_pnl);
        println!("  Unrealized PnL: ${:.2}", unrealized_pnl);
        println!("  Total PnL: ${:.2}", total_pnl);
        println!("  Total Return: {:.4}%", return_pct);
        println!("{}", "=".repeat(60));
    }

    fn create_backtest(&self, data_file: &str) -> Result<Backtest<HashMapMarketDepth>> {
        let latency_model = ConstantLatency::new(100_000, 100_000);
        let asset_type = LinearAsset::new(1.0);
        let queue_model = ProbQueueModel::new(PowerProbQueueFunc3::new(3.0));
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
