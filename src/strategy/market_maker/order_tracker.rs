use std::collections::HashMap;

/// 주문 추적 및 관리 (디버깅 및 PnL 계산용)
#[derive(Debug)]
pub struct OrderTracker {
    active_orders: HashMap<u64, OrderInfo>,
    filled_count: u64,
    total_buy_volume: f64,
    total_sell_volume: f64,
}

#[derive(Debug, Clone)]
pub struct OrderInfo {
    #[allow(dead_code)]
    pub order_id: u64,
    pub side: OrderSide,
    #[allow(dead_code)]
    pub price: f64,
    pub qty: f64,
    #[allow(dead_code)]
    pub layer: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderTracker {
    pub fn new() -> Self {
        Self {
            active_orders: HashMap::new(),
            filled_count: 0,
            total_buy_volume: 0.0,
            total_sell_volume: 0.0,
        }
    }

    /// 새 주문 등록
    pub fn register_order(&mut self, order_id: u64, side: OrderSide, price: f64, qty: f64, layer: usize) {
        self.active_orders.insert(order_id, OrderInfo {
            order_id,
            side,
            price,
            qty,
            layer,
        });
    }

    /// 주문 체결 처리
    pub fn mark_filled(&mut self, order_id: u64) -> Option<OrderInfo> {
        if let Some(order) = self.active_orders.remove(&order_id) {
            self.filled_count += 1;
            
            match order.side {
                OrderSide::Buy => self.total_buy_volume += order.qty,
                OrderSide::Sell => self.total_sell_volume += order.qty,
            }
            
            Some(order)
        } else {
            None
        }
    }

    /// 통계 정보
    pub fn get_stats(&self) -> (u64, f64, f64, usize) {
        (
            self.filled_count,
            self.total_buy_volume,
            self.total_sell_volume,
            self.active_orders.len(),
        )
    }

    /// 주문 존재 확인
    #[allow(dead_code)]
    pub fn has_order(&self, order_id: u64) -> bool {
        self.active_orders.contains_key(&order_id)
    }

    /// 모든 주문 제거
    #[allow(dead_code)]
    pub fn clear_all(&mut self) {
        self.active_orders.clear();
    }
}
