use hftbacktest::depth::MarketDepth;

pub struct MicroPriceCalculator {
    depth_levels: usize,
}

impl MicroPriceCalculator {
    pub fn new(depth_levels: usize) -> Self {
        Self { depth_levels }
    }

    pub fn calculate(&self, depth: &dyn MarketDepth) -> f64 {
        let tick_size = depth.tick_size();
        let best_bid_tick = depth.best_bid_tick();
        let best_ask_tick = depth.best_ask_tick();

        if best_bid_tick == i64::MIN || best_ask_tick == i64::MAX {
            return 0.0;
        }

        let mut bid_volume = 0.0;
        let mut ask_volume = 0.0;
        let mut _bid_weighted_price = 0.0;
        let mut _ask_weighted_price = 0.0;

        // L1~Ln까지의 volume 계산
        for level in 0..self.depth_levels {
            let bid_tick = best_bid_tick - level as i64;
            let ask_tick = best_ask_tick + level as i64;

            let bid_qty = depth.bid_qty_at_tick(bid_tick);
            if bid_qty > 0.0 {
                let bid_price = bid_tick as f64 * tick_size;
                bid_volume += bid_qty;
                _bid_weighted_price += bid_qty * bid_price;
            }

            let ask_qty = depth.ask_qty_at_tick(ask_tick);
            if ask_qty > 0.0 {
                let ask_price = ask_tick as f64 * tick_size;
                ask_volume += ask_qty;
                _ask_weighted_price += ask_qty * ask_price;
            }
        }

        if bid_volume + ask_volume == 0.0 {
            return (best_bid_tick as f64 + best_ask_tick as f64) * tick_size / 2.0;
        }

        let best_bid_price = best_bid_tick as f64 * tick_size;
        let best_ask_price = best_ask_tick as f64 * tick_size;

        (bid_volume * best_ask_price + ask_volume * best_bid_price) / (bid_volume + ask_volume)
    }
}

pub struct OrderBookImbalance {
    depth_levels: usize,
}

impl OrderBookImbalance {
    pub fn new(depth_levels: usize) -> Self {
        Self { depth_levels }
    }

    pub fn calculate(&self, depth: &dyn MarketDepth) -> f64 {
        let best_bid_tick = depth.best_bid_tick();
        let best_ask_tick = depth.best_ask_tick();

        if best_bid_tick == i64::MIN || best_ask_tick == i64::MAX {
            return 0.0;
        }

        let mut bid_volume = 0.0;
        let mut ask_volume = 0.0;

        for level in 0..self.depth_levels {
            let bid_tick = best_bid_tick - level as i64;
            let ask_tick = best_ask_tick + level as i64;

            let bid_qty = depth.bid_qty_at_tick(bid_tick);
            if bid_qty > 0.0 {
                bid_volume += bid_qty;
            }

            let ask_qty = depth.ask_qty_at_tick(ask_tick);
            if ask_qty > 0.0 {
                ask_volume += ask_qty;
            }
        }

        if bid_volume + ask_volume == 0.0 {
            return 0.0;
        }

        (bid_volume - ask_volume) / (bid_volume + ask_volume)
    }
}
