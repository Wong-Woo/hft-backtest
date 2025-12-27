use std::collections::VecDeque;

/// 오더북에서 ML 모델용 특성(feature)을 추출하는 모듈
/// 
/// 추출하는 특성들:
/// 1. 가격 관련: mid price, spread, weighted mid price
/// 2. 불균형 지표: bid/ask 수량 비율, 다층 불균형
/// 3. 압력 지표: bid/ask 압력, 누적 압력
/// 4. 변동성 지표: 가격 변동 표준편차
/// 5. 시계열 특성: 이전 가격 변화율

/// 오더북 레벨 정보
#[derive(Debug, Clone, Copy)]
pub struct Level {
    pub price: f64,
    pub quantity: f64,
}

/// 추출된 특성 벡터
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OrderBookFeatures {
    /// 중간 가격
    pub mid_price: f64,
    /// 스프레드 (bps)
    pub spread_bps: f64,
    /// 가중 중간 가격 (imbalance weighted)
    pub weighted_mid_price: f64,
    /// 1차 레벨 불균형 (-1 ~ 1)
    pub imbalance_level1: f64,
    /// 다층(5레벨) 불균형
    pub imbalance_multi_level: f64,
    /// Bid 압력 (상위 레벨 수량 합)
    pub bid_pressure: f64,
    /// Ask 압력 (상위 레벨 수량 합)
    pub ask_pressure: f64,
    /// 압력 비율
    pub pressure_ratio: f64,
    /// 최근 가격 변화율 (%)
    pub price_change_pct: f64,
    /// 변동성 (표준편차)
    pub volatility: f64,
    /// 수량 가중 스프레드
    pub volume_weighted_spread: f64,
    /// 거래 강도 지표
    pub trade_intensity: f64,
}

impl OrderBookFeatures {
    /// 특성 벡터를 f64 배열로 변환 (모델 입력용)
    pub fn to_vec(&self) -> Vec<f64> {
        vec![
            self.spread_bps,
            self.imbalance_level1,
            self.imbalance_multi_level,
            self.pressure_ratio,
            self.price_change_pct,
            self.volatility,
            self.volume_weighted_spread,
            self.trade_intensity,
        ]
    }

    /// 특성 차원 수
    pub fn feature_dim() -> usize {
        8
    }
}

/// 오더북 특성 추출기
pub struct OrderBookFeatureExtractor {
    /// 분석할 오더북 깊이 레벨 수
    depth_levels: usize,
    /// 과거 mid price 기록 (변동성 계산용)
    price_history: VecDeque<f64>,
    /// 과거 거래량 기록
    volume_history: VecDeque<f64>,
    /// 기록 유지 기간
    history_size: usize,
    /// 마지막 mid price
    last_mid_price: Option<f64>,
}

#[allow(dead_code)]
impl OrderBookFeatureExtractor {
    pub fn new(depth_levels: usize, history_size: usize) -> Self {
        Self {
            depth_levels,
            price_history: VecDeque::with_capacity(history_size),
            volume_history: VecDeque::with_capacity(history_size),
            history_size,
            last_mid_price: None,
        }
    }

    /// 오더북 데이터로부터 특성 추출
    pub fn extract(&mut self, bids: &[Level], asks: &[Level]) -> Option<OrderBookFeatures> {
        if bids.is_empty() || asks.is_empty() {
            return None;
        }

        // 기본 가격 정보
        let best_bid = bids[0].price;
        let best_ask = asks[0].price;
        let mid_price = (best_bid + best_ask) / 2.0;
        let spread = best_ask - best_bid;
        let spread_bps = (spread / mid_price) * 10000.0;

        // 1차 레벨 불균형
        let bid_qty_1 = bids[0].quantity;
        let ask_qty_1 = asks[0].quantity;
        let imbalance_level1 = (bid_qty_1 - ask_qty_1) / (bid_qty_1 + ask_qty_1);

        // 다층 불균형 (가용 레벨까지)
        let levels_to_use = self.depth_levels.min(bids.len()).min(asks.len());
        let mut total_bid_qty = 0.0;
        let mut total_ask_qty = 0.0;
        
        for i in 0..levels_to_use {
            // 거리에 따른 가중치 (가까울수록 높음)
            let weight = 1.0 / (i + 1) as f64;
            total_bid_qty += bids[i].quantity * weight;
            total_ask_qty += asks[i].quantity * weight;
        }
        
        let imbalance_multi_level = if total_bid_qty + total_ask_qty > 0.0 {
            (total_bid_qty - total_ask_qty) / (total_bid_qty + total_ask_qty)
        } else {
            0.0
        };

        // 압력 지표
        let bid_pressure: f64 = bids.iter().take(levels_to_use).map(|l| l.quantity).sum();
        let ask_pressure: f64 = asks.iter().take(levels_to_use).map(|l| l.quantity).sum();
        let pressure_ratio = if ask_pressure > 0.0 {
            (bid_pressure / ask_pressure).ln() // log ratio for symmetry
        } else {
            0.0
        };

        // 가중 중간 가격
        let weighted_mid_price = if bid_qty_1 + ask_qty_1 > 0.0 {
            (best_bid * ask_qty_1 + best_ask * bid_qty_1) / (bid_qty_1 + ask_qty_1)
        } else {
            mid_price
        };

        // 가격 변화율
        let price_change_pct = if let Some(last_price) = self.last_mid_price {
            ((mid_price - last_price) / last_price) * 100.0
        } else {
            0.0
        };

        // 변동성 계산
        let volatility = self.calculate_volatility();

        // 수량 가중 스프레드
        let volume_weighted_spread = spread * (bid_qty_1 + ask_qty_1) / 2.0;

        // 거래 강도 (전체 수량 변화)
        let current_total_volume = bid_pressure + ask_pressure;
        let trade_intensity = if let Some(&last_vol) = self.volume_history.back() {
            if last_vol > 0.0 {
                (current_total_volume - last_vol) / last_vol
            } else {
                0.0
            }
        } else {
            0.0
        };

        // 히스토리 업데이트
        self.update_history(mid_price, current_total_volume);

        Some(OrderBookFeatures {
            mid_price,
            spread_bps,
            weighted_mid_price,
            imbalance_level1,
            imbalance_multi_level,
            bid_pressure,
            ask_pressure,
            pressure_ratio,
            price_change_pct,
            volatility,
            volume_weighted_spread,
            trade_intensity,
        })
    }

    /// 변동성 계산 (가격 변화의 표준편차)
    fn calculate_volatility(&self) -> f64 {
        if self.price_history.len() < 2 {
            return 0.0;
        }

        // 수익률 계산
        let returns: Vec<f64> = self.price_history
            .iter()
            .zip(self.price_history.iter().skip(1))
            .map(|(prev, curr)| (curr - prev) / prev)
            .collect();

        if returns.is_empty() {
            return 0.0;
        }

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;

        variance.sqrt() * 10000.0 // bps 단위로 변환
    }

    /// 히스토리 업데이트
    fn update_history(&mut self, mid_price: f64, total_volume: f64) {
        self.last_mid_price = Some(mid_price);
        
        self.price_history.push_back(mid_price);
        if self.price_history.len() > self.history_size {
            self.price_history.pop_front();
        }

        self.volume_history.push_back(total_volume);
        if self.volume_history.len() > self.history_size {
            self.volume_history.pop_front();
        }
    }

    /// 충분한 히스토리가 있는지 확인
    pub fn is_ready(&self) -> bool {
        self.price_history.len() >= 10
    }

    /// 현재 mid price 반환
    pub fn get_mid_price(&self) -> Option<f64> {
        self.last_mid_price
    }

    /// 히스토리 초기화
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.price_history.clear();
        self.volume_history.clear();
        self.last_mid_price = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_extraction() {
        let mut extractor = OrderBookFeatureExtractor::new(5, 100);
        
        let bids = vec![
            Level { price: 100.0, quantity: 10.0 },
            Level { price: 99.0, quantity: 20.0 },
            Level { price: 98.0, quantity: 30.0 },
        ];
        
        let asks = vec![
            Level { price: 101.0, quantity: 15.0 },
            Level { price: 102.0, quantity: 25.0 },
            Level { price: 103.0, quantity: 35.0 },
        ];

        let features = extractor.extract(&bids, &asks).unwrap();
        
        assert!((features.mid_price - 100.5).abs() < 0.01);
        assert!(features.spread_bps > 0.0);
        assert!(features.imbalance_level1.abs() <= 1.0);
    }
}
