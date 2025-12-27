use anyhow::Result;
use hftbacktest::{
    backtest::{Backtest, BacktestError, ExchangeKind, L2AssetBuilder, assettype::LinearAsset,
        data::DataSource, models::{CommonFees, ConstantLatency, ProbQueueModel, 
        PowerProbQueueFunc3, TradingValueFeeModel}},
    prelude::{Bot, HashMapMarketDepth, Status, TimeInForce, OrdType},
    depth::MarketDepth,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::VecDeque;
use crossbeam_channel::Sender;
use crate::common::{calculate_mid_price, is_valid_depth};
use crate::config::{TICK_SIZE, LOT_SIZE, ELAPSE_DURATION_NS, UPDATE_INTERVAL, COMMAND_POLL_TIMEOUT_MICROS};
use crate::ui::{PerformanceData, OrderBookLevel};
use crate::controller::StrategyController;
use super::{OrderBookFeatureExtractor, PricePredictor, PredictionSignal};
use super::orderbook_features::Level;

/// 예측 기반 거래를 위한 1초 후 가격 예측 정보
struct PricePredictionData {
    /// 예측 시점의 mid price
    mid_price: f64,
    /// 예측한 가격 변화
    predicted_change: f64,
    /// 예측 시점 타임스탬프
    timestamp: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PositionState {
    Flat,
    Long,
    Short,
}

/// 오더북 기반 1초 후 가격 예측 전략 Runner
/// 
/// 전략 로직:
/// 1. 오더북에서 특성을 추출하여 신경망 모델에 입력
/// 2. 1초 후 가격 변화를 예측
/// 3. 예측에 따라 포지션 진입/청산
/// 4. 온라인 학습으로 모델 지속 개선
#[allow(dead_code)]
pub struct PredictionRunner {
    data_files: Vec<PathBuf>,
    feature_extractor: OrderBookFeatureExtractor,
    predictor: PricePredictor,
    position_size: f64,
    initial_capital: f64,
    position_state: PositionState,
    entry_price: f64,
    position_qty: f64,
    
    // 예측 관련
    prediction_horizon_ns: i64, // 1초 = 1_000_000_000ns
    pending_predictions: VecDeque<PricePredictionData>,
    min_prediction_confidence: f64,
    
    // 학습 관련
    learning_rate: f64,
    warmup_samples: usize,
    is_warmed_up: bool,
    
    // 리스크 관리
    stop_loss_pct: f64,
    take_profit_pct: f64,
    max_position_time_ns: i64,
    position_entry_time: i64,
    
    // 메트릭
    num_trades: usize,
    winning_trades: usize,
    total_orders: usize,
    total_fills: usize,
    total_hold_time: Duration,
    prediction_accuracy: f64,
    total_predictions: usize,
    correct_predictions: usize,
}

impl PredictionRunner {
    pub fn new_with_files(
        files: Vec<String>,
        position_size: f64,
        stop_loss_pct: f64,
        take_profit_pct: f64,
        initial_capital: f64,
        min_prediction_confidence: f64,
        learning_rate: f64,
    ) -> Result<Self> {
        let data_files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
        if data_files.is_empty() {
            anyhow::bail!("No data files provided");
        }
        println!("Using {} file(s):", data_files.len());
        for (i, f) in data_files.iter().enumerate() {
            println!("  [{}] {}", i + 1, f.display());
        }
        Self::create_runner(data_files, position_size, stop_loss_pct, take_profit_pct, initial_capital, min_prediction_confidence, learning_rate)
    }
    
    fn create_runner(
        data_files: Vec<PathBuf>,
        position_size: f64,
        stop_loss_pct: f64,
        take_profit_pct: f64,
        initial_capital: f64,
        min_prediction_confidence: f64,
        learning_rate: f64,
    ) -> Result<Self> {
        let predictor = PricePredictor::new(min_prediction_confidence)?;

        Ok(Self {
            data_files,
            feature_extractor: OrderBookFeatureExtractor::new(10, 100),
            predictor,
            position_size,
            initial_capital,
            position_state: PositionState::Flat,
            entry_price: 0.0,
            position_qty: 0.0,
            prediction_horizon_ns: 1_000_000_000,
            pending_predictions: VecDeque::with_capacity(100),
            min_prediction_confidence,
            learning_rate,
            warmup_samples: 1000,
            is_warmed_up: false,
            stop_loss_pct,
            take_profit_pct,
            max_position_time_ns: 5_000_000_000,
            position_entry_time: 0,
            num_trades: 0,
            winning_trades: 0,
            total_orders: 0,
            total_fills: 0,
            total_hold_time: Duration::ZERO,
            prediction_accuracy: 0.0,
            total_predictions: 0,
            correct_predictions: 0,
        })
    }

    /// 오더북에서 Level 정보 추출
    fn extract_levels<MD>(&self, depth: &MD, count: usize) -> (Vec<Level>, Vec<Level>)
    where
        MD: MarketDepth,
    {
        let mut bids = Vec::new();
        let mut asks = Vec::new();
        
        let best_bid_tick = depth.best_bid_tick();
        let best_ask_tick = depth.best_ask_tick();
        let tick_size = depth.tick_size();
        
        if best_bid_tick != i64::MIN {
            for i in 0..count {
                let tick = best_bid_tick - i as i64;
                let qty = depth.bid_qty_at_tick(tick);
                if qty > 0.0 {
                    bids.push(Level {
                        price: tick as f64 * tick_size,
                        quantity: qty,
                    });
                }
            }
        }
        
        if best_ask_tick != i64::MAX {
            for i in 0..count {
                let tick = best_ask_tick + i as i64;
                let qty = depth.ask_qty_at_tick(tick);
                if qty > 0.0 {
                    asks.push(Level {
                        price: tick as f64 * tick_size,
                        quantity: qty,
                    });
                }
            }
        }
        
        (bids, asks)
    }

    /// UI용 오더북 레벨 추출
    fn extract_orderbook<MD>(&self, depth: &MD, levels: usize) -> (Vec<OrderBookLevel>, Vec<OrderBookLevel>)
    where
        MD: MarketDepth,
    {
        let mut bids = Vec::new();
        let mut asks = Vec::new();
        
        let best_bid_tick = depth.best_bid_tick();
        let best_ask_tick = depth.best_ask_tick();
        let tick_size = depth.tick_size();
        
        if best_bid_tick != i64::MIN {
            for i in 0..levels {
                let tick = best_bid_tick - i as i64;
                let qty = depth.bid_qty_at_tick(tick);
                if qty > 0.0 {
                    bids.push(OrderBookLevel {
                        price: tick as f64 * tick_size,
                        quantity: qty,
                    });
                }
            }
        }
        
        if best_ask_tick != i64::MAX {
            for i in 0..levels {
                let tick = best_ask_tick + i as i64;
                let qty = depth.ask_qty_at_tick(tick);
                if qty > 0.0 {
                    asks.push(OrderBookLevel {
                        price: tick as f64 * tick_size,
                        quantity: qty,
                    });
                }
            }
        }
        
        (bids, asks)
    }

    /// Controller를 통한 전략 실행
    pub fn run_with_controller(
        &mut self,
        sender: Sender<PerformanceData>,
        controller: Arc<StrategyController>,
    ) -> Result<()> {
        let file_count = self.data_files.len();
        
        for file_idx in 0..file_count {
            // Wait for start signal if in paused or stopped state
            while !controller.is_running() && !controller.should_stop() {
                controller.process_commands(Duration::from_millis(100));
            }
            
            if controller.should_stop() {
                println!("\n⏹ Strategy stopped by user");
                break;
            }
            
            let data_file = self.data_files[file_idx].clone();
            
            println!("\n{}", "=".repeat(60));
            println!("Running ML Prediction strategy on file [{}/{}]: {}", 
                     file_idx + 1, 
                     file_count, 
                     data_file.display());
            println!("{}\n", "=".repeat(60));
            
            self.run_strategy_with_control(
                data_file.to_str().unwrap(),
                &sender,
                &controller,
            )?;
        }
        
        if !controller.should_stop() {
            controller.mark_completed();
            println!("\n✅ All files processed successfully!");
        }
        
        // Keep thread alive until GUI closes
        self.keep_alive_until_close(&controller);
        
        Ok(())
    }
    
    /// Keep thread alive until GUI window closes
    fn keep_alive_until_close(&self, controller: &StrategyController) {
        println!("Backtest finished. Close the window to exit.");
        
        loop {
            if !controller.process_commands(Duration::from_millis(200)) {
                std::thread::sleep(Duration::from_millis(100));
                if !controller.process_commands(Duration::from_millis(100)) {
                    break;
                }
            }
        }
    }

    /// 단일 파일에 대한 전략 실행 (Controller 사용)
    fn run_strategy_with_control(
        &mut self,
        data_file: &str,
        sender: &Sender<PerformanceData>,
        controller: &StrategyController,
    ) -> Result<()> {
        println!("Loading data from: {}", data_file);

        let mut hbt = self.create_backtest(data_file)?;
        
        println!("ML Prediction strategy started...\n");
        println!("🔬 Warming up model with {} samples...\n", self.warmup_samples);

        let mut realized_pnl = 0.0;
        let cash = self.initial_capital;
        let mut update_count = 0;

        // Reset state
        self.position_state = PositionState::Flat;
        self.entry_price = 0.0;
        self.position_qty = 0.0;
        self.is_warmed_up = false;

        let mut last_gui_update = Instant::now();
        let mut last_command_check = Instant::now();
        let command_check_interval = Duration::from_millis(16); // ~60Hz command polling
        let mut current_time_ns: i64 = 0;

        loop {
            // Check pause/stop state (always, regardless of timing)
            if !controller.is_running() {
                // Process commands while paused
                controller.process_commands(Duration::from_millis(50));
                
                if controller.should_stop() {
                    println!("\n⏹ Strategy stopped by user");
                    break;
                }
                continue;
            }
            
            // Process commands at fixed interval when running
            if last_command_check.elapsed() >= command_check_interval {
                controller.process_commands(Duration::from_micros(COMMAND_POLL_TIMEOUT_MICROS));
                last_command_check = Instant::now();
                
                if controller.should_stop() {
                    println!("\n⏹ Strategy stopped by user");
                    break;
                }
            }
            
            // Speed adjustment - batch iterations for high speed
            let speed = controller.speed_multiplier();
            let iterations_per_loop = if speed > 10.0 {
                (speed / 10.0).ceil() as usize
            } else {
                1
            };
            
            for _ in 0..iterations_per_loop {
                match hbt.elapse(ELAPSE_DURATION_NS) {
                    Ok(_) => {
                        current_time_ns += ELAPSE_DURATION_NS;
                        let depth = hbt.depth(0);
                        
                        if !is_valid_depth(depth) {
                            continue;
                        }

                        update_count += 1;
                        
                        let mid_price = calculate_mid_price(depth);
                        
                        // 특성 추출
                        let (bids, asks) = self.extract_levels(depth, 10);
                        
                        if let Some(features) = self.feature_extractor.extract(&bids, &asks) {
                            // 과거 예측 검증 및 학습
                            self.validate_and_learn_predictions(mid_price, current_time_ns);
                            
                            // 새 예측 수행
                            if let Ok((prediction, signal)) = self.predictor.predict(&features) {
                                // 예측 기록
                                self.pending_predictions.push_back(PricePredictionData {
                                    mid_price,
                                    predicted_change: prediction,
                                    timestamp: current_time_ns,
                                });
                                
                                // 오래된 예측 제거
                                while self.pending_predictions.len() > 100 {
                                    self.pending_predictions.pop_front();
                                }
                                
                                // Warmup 체크
                                if !self.is_warmed_up && self.predictor.get_training_samples() >= self.warmup_samples {
                                    self.is_warmed_up = true;
                                    println!("\n🚀 Model warmed up! Starting trading...\n");
                                }
                                
                                // 거래 실행 (warmup 후에만)
                                if self.is_warmed_up && update_count % UPDATE_INTERVAL == 0 {
                                    self.execute_strategy(&mut hbt, &mut realized_pnl, signal, prediction, current_time_ns)?;
                                }
                            }
                        }
                    }
                    Err(_) => {
                        println!("\nEnd of data reached!");
                        // 남은 포지션 청산
                        if self.position_state != PositionState::Flat {
                            println!("\nClosing remaining position...");
                            let _ = self.close_position(&mut hbt, &mut realized_pnl)?;
                        }
                        let final_depth = hbt.depth(0);
                        self.print_final_stats(realized_pnl, cash, final_depth);
                        return Ok(());
                    }
                }
            }

            // GUI 업데이트 (throttled to ~30 FPS)
            if last_gui_update.elapsed() >= Duration::from_millis(33) {
                let depth_for_data = hbt.depth(0);
                if is_valid_depth(depth_for_data) {
                    let mid_price = calculate_mid_price(depth_for_data);
                    
                    let (position_value, unrealized_pnl) = self.calculate_position_metrics(mid_price);
                    let (bids, asks) = self.extract_orderbook(depth_for_data, 10);
                    let avg_hold_time = if self.num_trades > 0 {
                        self.total_hold_time.as_secs_f64() / self.num_trades as f64
                    } else {
                        0.0
                    };
                    
                    self.prediction_accuracy = self.predictor.get_accuracy();
                    
                    // Use try_send to avoid blocking GUI
                    // timestamp = simulation time in seconds
                    let sim_time_secs = update_count as f64 * (ELAPSE_DURATION_NS as f64 / 1_000_000_000.0);
                    let _ = sender.try_send(PerformanceData {
                        timestamp: sim_time_secs,
                        equity: cash + realized_pnl + position_value,
                        realized_pnl,
                        unrealized_pnl,
                        position: self.position_qty,
                        mid_price,
                        strategy_name: format!("ML Prediction (Acc: {:.1}%)", self.prediction_accuracy * 100.0),
                        num_trades: self.num_trades,
                        winning_trades: self.winning_trades,
                        total_fills: self.total_fills,
                        total_orders: self.total_orders,
                        position_hold_time: avg_hold_time,
                        latency_micros: 100,
                        bids,
                        asks,
                    });
                }
                last_gui_update = Instant::now();
            }
            
            // Yield at lower speeds
            if speed <= 1.0 {
                std::thread::yield_now();
            }
        }

        // 남은 포지션 청산
        if self.position_state != PositionState::Flat {
            println!("\nClosing remaining position...");
            let _ = self.close_position(&mut hbt, &mut realized_pnl)?;
        }

        let final_depth = hbt.depth(0);
        self.print_final_stats(realized_pnl, cash, final_depth);

        Ok(())
    }

    /// 과거 예측 검증 및 온라인 학습
    fn validate_and_learn_predictions(&mut self, current_mid_price: f64, current_time_ns: i64) {
        // 1초 전 예측 찾기
        while let Some(pred) = self.pending_predictions.front() {
            if current_time_ns - pred.timestamp >= self.prediction_horizon_ns {
                let actual_change = (current_mid_price - pred.mid_price) / pred.mid_price * 100.0;
                
                // 방향 정확도 체크
                self.total_predictions += 1;
                if (pred.predicted_change > 0.0 && actual_change > 0.0) ||
                   (pred.predicted_change < 0.0 && actual_change < 0.0) {
                    self.correct_predictions += 1;
                }
                
                // 예측 기록 (정확도 추적용)
                self.predictor.record_prediction(pred.predicted_change, actual_change);
                
                // 특성 재추출 후 학습 (실제 구현에서는 저장된 특성 사용)
                // 여기서는 간단히 버퍼에 있는 데이터로 배치 학습
                if self.predictor.get_training_samples() >= 64 && 
                   self.pending_predictions.len() % 32 == 0 {
                    if let Err(e) = self.predictor.train_batch(32, self.learning_rate) {
                        eprintln!("Training error: {}", e);
                    }
                }
                
                self.pending_predictions.pop_front();
            } else {
                break;
            }
        }
    }

    /// 전략 실행
    fn execute_strategy<MD>(
        &mut self,
        hbt: &mut Backtest<MD>,
        realized_pnl: &mut f64,
        signal: PredictionSignal,
        prediction: f64,
        current_time_ns: i64,
    ) -> Result<(), BacktestError>
    where
        MD: MarketDepth,
    {
        let depth = hbt.depth(0);
        let mid_price = calculate_mid_price(depth);

        // 포지션 종료 조건 체크
        if self.position_state != PositionState::Flat {
            // Stop-loss / Take-profit 체크
            if self.should_close_position(mid_price) {
                println!("  💔 Closing due to stop-loss/take-profit");
                return self.close_position(hbt, realized_pnl);
            }
            
            // 최대 보유 시간 초과
            if current_time_ns - self.position_entry_time > self.max_position_time_ns {
                println!("  ⏰ Closing due to max hold time");
                return self.close_position(hbt, realized_pnl);
            }
        }

        // 신호 기반 거래
        match self.position_state {
            PositionState::Flat => {
                match signal {
                    PredictionSignal::Up => {
                        println!("  🔮 Predicted UP ({:.4}%) - Opening LONG", prediction * 100.0);
                        self.open_long_position(hbt, current_time_ns)?;
                    }
                    PredictionSignal::Down => {
                        println!("  🔮 Predicted DOWN ({:.4}%) - Opening SHORT", prediction * 100.0);
                        self.open_short_position(hbt, current_time_ns)?;
                    }
                    PredictionSignal::Neutral => {}
                }
            }
            PositionState::Long => {
                if signal == PredictionSignal::Down {
                    println!("  ⚠️  Signal reversed, closing LONG");
                    self.close_position(hbt, realized_pnl)?;
                }
            }
            PositionState::Short => {
                if signal == PredictionSignal::Up {
                    println!("  ⚠️  Signal reversed, closing SHORT");
                    self.close_position(hbt, realized_pnl)?;
                }
            }
        }

        Ok(())
    }

    fn open_long_position<MD>(
        &mut self,
        hbt: &mut Backtest<MD>,
        current_time_ns: i64,
    ) -> Result<(), BacktestError>
    where
        MD: MarketDepth,
    {
        let depth = hbt.depth(0);
        let tick_size = depth.tick_size();
        let best_ask_tick = depth.best_ask_tick();
        let best_ask_price = best_ask_tick as f64 * tick_size;
        
        let order_id = 100 + self.total_orders as u64;
        hbt.submit_buy_order(
            0,
            order_id,
            best_ask_price,
            self.position_size,
            TimeInForce::GTC,
            OrdType::Limit,
            false,
        )?;
        self.total_orders += 1;

        hbt.wait_order_response(0, order_id, 10_000_000_000)?;

        let orders = hbt.orders(0);
        if let Some(order) = orders.get(&order_id) {
            if order.status == Status::Filled {
                self.entry_price = order.price_tick as f64 * tick_size;
                self.position_qty = order.qty;
                self.position_state = PositionState::Long;
                self.position_entry_time = current_time_ns;
                self.total_fills += 1;
                
                println!("    ✓ Opened LONG @ {:.6} qty {:.4}", self.entry_price, self.position_qty);
            }
        }

        Ok(())
    }

    fn open_short_position<MD>(
        &mut self,
        hbt: &mut Backtest<MD>,
        current_time_ns: i64,
    ) -> Result<(), BacktestError>
    where
        MD: MarketDepth,
    {
        let depth = hbt.depth(0);
        let tick_size = depth.tick_size();
        let best_bid_tick = depth.best_bid_tick();
        let best_bid_price = best_bid_tick as f64 * tick_size;
        
        let order_id = 200 + self.total_orders as u64;
        hbt.submit_sell_order(
            0,
            order_id,
            best_bid_price,
            self.position_size,
            TimeInForce::GTC,
            OrdType::Limit,
            false,
        )?;
        self.total_orders += 1;

        hbt.wait_order_response(0, order_id, 10_000_000_000)?;

        let orders = hbt.orders(0);
        if let Some(order) = orders.get(&order_id) {
            if order.status == Status::Filled {
                self.entry_price = order.price_tick as f64 * tick_size;
                self.position_qty = order.qty;
                self.position_state = PositionState::Short;
                self.position_entry_time = current_time_ns;
                self.total_fills += 1;
                
                println!("    ✓ Opened SHORT @ {:.6} qty {:.4}", self.entry_price, self.position_qty);
            }
        }

        Ok(())
    }

    fn close_position<MD>(
        &mut self,
        hbt: &mut Backtest<MD>,
        realized_pnl: &mut f64,
    ) -> Result<(), BacktestError>
    where
        MD: MarketDepth,
    {
        let depth = hbt.depth(0);
        let tick_size = depth.tick_size();

        match self.position_state {
            PositionState::Long => {
                let best_bid_tick = depth.best_bid_tick();
                let best_bid_price = best_bid_tick as f64 * tick_size;
                
                let order_id = 300 + self.total_orders as u64;
                hbt.submit_sell_order(
                    0,
                    order_id,
                    best_bid_price,
                    self.position_qty,
                    TimeInForce::GTC,
                    OrdType::Limit,
                    false,
                )?;
                self.total_orders += 1;

                hbt.wait_order_response(0, order_id, 10_000_000_000)?;

                let orders = hbt.orders(0);
                if let Some(order) = orders.get(&order_id) {
                    if order.status == Status::Filled {
                        let exit_price = order.price_tick as f64 * tick_size;
                        let pnl = (exit_price - self.entry_price) * self.position_qty;
                        let fee = (exit_price * self.position_qty + self.entry_price * self.position_qty) * 0.0001;
                        *realized_pnl += pnl - fee;
                        self.total_fills += 1;
                        
                        self.num_trades += 1;
                        if pnl > 0.0 {
                            self.winning_trades += 1;
                        }
                        
                        println!("    ✓ Closed LONG @ {:.6} | PnL: {:.4} | Fee: {:.4}", 
                                 exit_price, pnl, fee);
                    }
                }
            }
            PositionState::Short => {
                let best_ask_tick = depth.best_ask_tick();
                let best_ask_price = best_ask_tick as f64 * tick_size;
                
                let order_id = 400 + self.total_orders as u64;
                hbt.submit_buy_order(
                    0,
                    order_id,
                    best_ask_price,
                    self.position_qty,
                    TimeInForce::GTC,
                    OrdType::Limit,
                    false,
                )?;
                self.total_orders += 1;

                hbt.wait_order_response(0, order_id, 10_000_000_000)?;

                let orders = hbt.orders(0);
                if let Some(order) = orders.get(&order_id) {
                    if order.status == Status::Filled {
                        let exit_price = order.price_tick as f64 * tick_size;
                        let pnl = (self.entry_price - exit_price) * self.position_qty;
                        let fee = (exit_price * self.position_qty + self.entry_price * self.position_qty) * 0.0001;
                        *realized_pnl += pnl - fee;
                        self.total_fills += 1;
                        
                        self.num_trades += 1;
                        if pnl > 0.0 {
                            self.winning_trades += 1;
                        }
                        
                        println!("    ✓ Closed SHORT @ {:.6} | PnL: {:.4} | Fee: {:.4}", 
                                 exit_price, pnl, fee);
                    }
                }
            }
            PositionState::Flat => {}
        }

        self.position_state = PositionState::Flat;
        self.entry_price = 0.0;
        self.position_qty = 0.0;

        Ok(())
    }

    fn calculate_position_metrics(&self, mid_price: f64) -> (f64, f64) {
        match self.position_state {
            PositionState::Long => {
                let position_value = self.position_qty * mid_price;
                let unrealized_pnl = (mid_price - self.entry_price) * self.position_qty;
                (position_value, unrealized_pnl)
            }
            PositionState::Short => {
                let position_value = -self.position_qty * mid_price;
                let unrealized_pnl = (self.entry_price - mid_price) * self.position_qty;
                (position_value, unrealized_pnl)
            }
            PositionState::Flat => (0.0, 0.0),
        }
    }

    fn should_close_position(&self, current_price: f64) -> bool {
        if self.entry_price == 0.0 {
            return false;
        }

        match self.position_state {
            PositionState::Long => {
                let pnl_pct = (current_price - self.entry_price) / self.entry_price;
                pnl_pct <= -self.stop_loss_pct || pnl_pct >= self.take_profit_pct
            }
            PositionState::Short => {
                let pnl_pct = (self.entry_price - current_price) / self.entry_price;
                pnl_pct <= -self.stop_loss_pct || pnl_pct >= self.take_profit_pct
            }
            PositionState::Flat => false,
        }
    }

    fn create_backtest(&self, data_file: &str) -> Result<Backtest<HashMapMarketDepth>> {
        let latency_model = ConstantLatency::new(0, 0);
        let queue_model = ProbQueueModel::new(PowerProbQueueFunc3::new(3.0));
        let asset_type = LinearAsset::new(1.0);
        let fee_model = TradingValueFeeModel::new(CommonFees::new(-0.00005, 0.0007));

        let hbt = Backtest::builder()
            .add_asset(
                L2AssetBuilder::new()
                    .data(vec![
                        DataSource::File(data_file.to_string())
                    ])
                    .latency_model(latency_model)
                    .queue_model(queue_model)
                    .asset_type(asset_type)
                    .fee_model(fee_model)
                    .exchange(ExchangeKind::NoPartialFillExchange)
                    .depth(|| HashMapMarketDepth::new(TICK_SIZE, LOT_SIZE))
                    .build()?,
            )
            .build()?;

        Ok(hbt)
    }

    fn print_final_stats<MD>(&self, realized_pnl: f64, cash: f64, depth: &MD)
    where
        MD: MarketDepth,
    {
        let mid_price = calculate_mid_price(depth);
        let (position_value, _) = self.calculate_position_metrics(mid_price);
        let final_equity = cash + realized_pnl + position_value;
        let returns_pct = ((final_equity - self.initial_capital) / self.initial_capital) * 100.0;
        let win_rate = if self.num_trades > 0 {
            (self.winning_trades as f64 / self.num_trades as f64) * 100.0
        } else {
            0.0
        };
        let prediction_accuracy = if self.total_predictions > 0 {
            (self.correct_predictions as f64 / self.total_predictions as f64) * 100.0
        } else {
            0.0
        };

        println!("\n{}", "=".repeat(60));
        println!("📊 ML PREDICTION STRATEGY FINAL STATISTICS");
        println!("{}", "=".repeat(60));
        println!("Initial Capital:     ${:.2}", self.initial_capital);
        println!("Final Equity:        ${:.2}", final_equity);
        println!("Total Returns:       {:.2}%", returns_pct);
        println!("Realized P&L:        ${:.2}", realized_pnl);
        println!("{}", "-".repeat(60));
        println!("Total Trades:        {}", self.num_trades);
        println!("Winning Trades:      {}", self.winning_trades);
        println!("Win Rate:            {:.2}%", win_rate);
        println!("{}", "-".repeat(60));
        println!("🧠 MODEL PERFORMANCE");
        println!("Training Samples:    {}", self.predictor.get_training_samples());
        println!("Total Predictions:   {}", self.total_predictions);
        println!("Prediction Accuracy: {:.2}%", prediction_accuracy);
        println!("{}", "=".repeat(60));
    }
}
