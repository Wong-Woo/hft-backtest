use hftbacktest::depth::MarketDepth;

/// 헬퍼 함수 모음
pub mod helpers {
    use super::*;

    /// MarketDepth에서 mid price 계산
    pub fn calculate_mid_price<MD: MarketDepth>(depth: &MD) -> f64 {
        let tick_size = depth.tick_size();
        (depth.best_bid_tick() as f64 + depth.best_ask_tick() as f64) / 2.0 * tick_size
    }

    /// MarketDepth가 유효한지 확인
    pub fn is_valid_depth<MD: MarketDepth>(depth: &MD) -> bool {
        depth.best_bid_tick() != i64::MIN && depth.best_ask_tick() != i64::MAX
    }
}
