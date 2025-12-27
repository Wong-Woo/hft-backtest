use std::collections::VecDeque;
use crate::ui::PerformanceData;

pub struct ChartHistory {
    pub equity: VecDeque<(f64, f64)>,
    pub pnl: VecDeque<(f64, f64)>,
    pub position: VecDeque<(f64, f64)>,
    pub price: VecDeque<(f64, f64)>,
    pub win_rate: VecDeque<(f64, f64)>,
    pub avg_profit: VecDeque<(f64, f64)>,
    pub fill_ratio: VecDeque<(f64, f64)>,
    pub position_hold_time: VecDeque<(f64, f64)>,
    pub latency: VecDeque<(f64, f64)>,
    max_points: usize,
}

impl ChartHistory {
    pub fn new(max_points: usize) -> Self {
        Self {
            equity: VecDeque::new(),
            pnl: VecDeque::new(),
            position: VecDeque::new(),
            price: VecDeque::new(),
            win_rate: VecDeque::new(),
            avg_profit: VecDeque::new(),
            fill_ratio: VecDeque::new(),
            position_hold_time: VecDeque::new(),
            latency: VecDeque::new(),
            max_points,
        }
    }

    pub fn set_max_points(&mut self, max_points: usize) {
        self.max_points = max_points;
    }

    pub fn max_points(&self) -> usize {
        self.max_points
    }

    pub fn len(&self) -> usize {
        self.equity.len()
    }

    pub fn clear(&mut self) {
        self.equity.clear();
        self.pnl.clear();
        self.position.clear();
        self.price.clear();
        self.win_rate.clear();
        self.avg_profit.clear();
        self.fill_ratio.clear();
        self.position_hold_time.clear();
        self.latency.clear();
    }

    pub fn push(&mut self, data: &PerformanceData) {
        let ts = data.timestamp;
        
        self.equity.push_back((ts, data.equity));
        self.pnl.push_back((ts, data.realized_pnl + data.unrealized_pnl));
        self.position.push_back((ts, data.position));
        self.price.push_back((ts, data.mid_price));
        
        let win_rate = if data.num_trades > 0 {
            (data.winning_trades as f64 / data.num_trades as f64) * 100.0
        } else { 0.0 };
        self.win_rate.push_back((ts, win_rate));
        
        let avg_profit = if data.num_trades > 0 {
            data.realized_pnl / data.num_trades as f64
        } else { 0.0 };
        self.avg_profit.push_back((ts, avg_profit));
        
        let fill_ratio = if data.total_orders > 0 {
            (data.total_fills as f64 / data.total_orders as f64) * 100.0
        } else { 0.0 };
        self.fill_ratio.push_back((ts, fill_ratio));
        
        self.position_hold_time.push_back((ts, data.position_hold_time));
        self.latency.push_back((ts, data.latency_micros as f64));
        
        self.trim_to_max();
    }

    fn trim_to_max(&mut self) {
        while self.equity.len() > self.max_points {
            self.equity.pop_front();
            self.pnl.pop_front();
            self.position.pop_front();
            self.price.pop_front();
            self.win_rate.pop_front();
            self.avg_profit.pop_front();
            self.fill_ratio.pop_front();
            self.position_hold_time.pop_front();
            self.latency.pop_front();
        }
    }
}
