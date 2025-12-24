use hftbacktest::{
    prelude::*,
    backtest::{Backtest, BacktestError},
    depth::MarketDepth,
};

/// 주문 집행 관리
pub struct OrderManager {
    order_layers: usize,  // 레이어링 개수
    layer_spacing: f64,   // 레이어 간격 (틱 단위)
}

impl OrderManager {
    pub fn new(order_layers: usize, layer_spacing: f64) -> Self {
        Self {
            order_layers,
            layer_spacing,
        }
    }

    /// 양방향 주문 생성 (레이어링 포함)
    /// reservation_price: 재고 리스크 반영한 중간가
    /// spread: 최적 스프레드
    pub fn place_layered_orders<MD>(
        &self,
        hbt: &mut Backtest<MD>,
        reservation_price: f64,
        half_spread: f64,
        order_size: f64,
        imbalance: f64,
    ) -> Result<(), BacktestError>
    where
        MD: MarketDepth,
    {
        let tick_size = hbt.depth(0).tick_size();
        
        // Imbalance에 따른 가격 조정
        let imbalance_adjustment = imbalance * half_spread * 0.2;  // 최대 20% 조정
        
        for layer in 0..self.order_layers {
            let layer_offset = layer as f64 * self.layer_spacing * tick_size;
            
            // Bid (매수) 주문
            let bid_price = reservation_price - half_spread - layer_offset + imbalance_adjustment;
            let bid_tick = (bid_price / tick_size).round() as i64;
            
            // Ask (매도) 주문  
            let ask_price = reservation_price + half_spread + layer_offset - imbalance_adjustment;
            let ask_tick = (ask_price / tick_size).round() as i64;
            
            // 레이어별 수량 감소 (첫 레이어가 가장 큼)
            let layer_size = order_size / (1.0 + layer as f64 * 0.5);
            
            // 실제 주문 제출
            hbt.submit_buy_order(
                0, 
                (layer * 2) as u64, 
                bid_tick as f64, 
                layer_size, 
                TimeInForce::GTX, 
                OrdType::Limit, 
                false
            ).ok();
            hbt.submit_sell_order(
                0, 
                (layer * 2 + 1) as u64, 
                ask_tick as f64, 
                layer_size, 
                TimeInForce::GTX, 
                OrdType::Limit, 
                false
            ).ok();
        }

        Ok(())
    }

    /// 모든 열린 주문 취소
    pub fn cancel_all_orders<MD>(&self, hbt: &mut Backtest<MD>) -> Result<(), BacktestError>
    where
        MD: MarketDepth,
    {
        // 모든 주문 취소
        for layer in 0..self.order_layers {
            let _ = hbt.cancel(0, (layer * 2) as u64, false);
            let _ = hbt.cancel(0, (layer * 2 + 1) as u64, false);
        }
        Ok(())
    }

    /// 재고 상태에 따라 한쪽 주문만 제출
    pub fn place_sided_orders<MD>(
        &self,
        hbt: &mut Backtest<MD>,
        reservation_price: f64,
        half_spread: f64,
        order_size: f64,
        inventory: f64,
        inventory_threshold: f64,
    ) -> Result<(), BacktestError>
    where
        MD: MarketDepth,
    {
        let tick_size = hbt.depth(0).tick_size();
        
        // 재고가 많으면 매도만, 적으면 매수만
        if inventory > inventory_threshold {
            // 매도 주문만
            for layer in 0..self.order_layers {
                let layer_offset = layer as f64 * self.layer_spacing * tick_size;
                let ask_price = reservation_price + half_spread + layer_offset;
                let ask_tick = (ask_price / tick_size).round() as i64;
                let layer_size = order_size / (1.0 + layer as f64 * 0.5);
                
                hbt.submit_sell_order(
                    0, 
                    (layer * 2 + 1) as u64, 
                    ask_tick as f64, 
                    layer_size, 
                    TimeInForce::GTX, 
                    OrdType::Limit, 
                    false
                ).ok();
            }
        } else if inventory < -inventory_threshold {
            // 매수 주문만
            for layer in 0..self.order_layers {
                let layer_offset = layer as f64 * self.layer_spacing * tick_size;
                let bid_price = reservation_price - half_spread - layer_offset;
                let bid_tick = (bid_price / tick_size).round() as i64;
                let layer_size = order_size / (1.0 + layer as f64 * 0.5);
                
                hbt.submit_buy_order(
                    0, 
                    (layer * 2) as u64, 
                    bid_tick as f64, 
                    layer_size, 
                    TimeInForce::GTX, 
                    OrdType::Limit, 
                    false
                ).ok();
            }
        } else {
            // 양방향 주문
            self.place_layered_orders(hbt, reservation_price, half_spread, order_size, 0.0)?;
        }

        Ok(())
    }
}
