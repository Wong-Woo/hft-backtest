use hftbacktest::depth::MarketDepth;

/// Order book display structure (Single Responsibility Principle)
pub struct OrderBookDisplay {
    ask_depth_levels: usize,
    bid_depth_levels: usize,
}

impl OrderBookDisplay {
    pub fn new(ask_depth_levels: usize, bid_depth_levels: usize) -> Self {
        Self { 
            ask_depth_levels,
            bid_depth_levels,
        }
    }

    /// Display order book in real exchange format
    pub fn display<Q>(&self, depth: &Q)
    where
        Q: MarketDepth,
    {
        let best_bid_tick = depth.best_bid_tick();
        let best_ask_tick = depth.best_ask_tick();
        let tick_size = depth.tick_size();

        // Collect ask side (sell orders) - search wider range
        let mut asks = Vec::new();
        for i in 0..(self.ask_depth_levels * 100) {  // Search wider range
            let tick = best_ask_tick + i as i64;
            let qty = depth.ask_qty_at_tick(tick);
            if qty > 0.0 {
                let price = tick as f64 * tick_size;
                asks.push((price, qty));
                if asks.len() >= self.ask_depth_levels {
                    break;
                }
            }
        }

        // Collect bid side (buy orders) - search wider range
        let mut bids = Vec::new();
        for i in 0..(self.bid_depth_levels * 100) {  // Search wider range
            let tick = best_bid_tick - i as i64;
            let qty = depth.bid_qty_at_tick(tick);
            if qty > 0.0 {
                let price = tick as f64 * tick_size;
                bids.push((price, qty));
                if bids.len() >= self.bid_depth_levels {
                    break;
                }
            }
        }

        println!("\n{}", "=".repeat(70));
        println!("{:^70}", "ORDER BOOK");
        println!("{}", "=".repeat(70));
        
        // Ask side (from high to low price, displayed top to bottom)
        println!("{:^70}", "--- ASK (Sell) ---");
        println!("{:>10} {:>25} {:>25}", "LEVEL", "PRICE", "SIZE");
        println!("{}", "-".repeat(70));
        
        let ask_count = asks.len().min(self.ask_depth_levels);
        for i in 0..ask_count {
            let (price, qty) = asks[ask_count - 1 - i];
            println!("{:>10} {:>25.2} {:>25.4}", 
                     ask_count - i, price, qty);
        }
        
        // Spread display
        if best_bid_tick != i64::MIN && best_ask_tick != i64::MAX {
            let best_bid = best_bid_tick as f64 * tick_size;
            let best_ask = best_ask_tick as f64 * tick_size;
            let spread = best_ask - best_bid;
            let spread_pct = (spread / best_bid) * 100.0;
            println!("{}", "=".repeat(70));
            println!("{:^70}", format!("SPREAD: {:.2} ({:.3}%)", spread, spread_pct));
            println!("{}", "=".repeat(70));
        }
        
        // Bid side (from high to low price)
        println!("{:>10} {:>25} {:>25}", "LEVEL", "PRICE", "SIZE");
        println!("{}", "-".repeat(70));
        println!("{:^70}", "--- BID (Buy) ---");
        
        let bid_count = bids.len().min(self.bid_depth_levels);
        for i in 0..bid_count {
            let (price, qty) = bids[i];
            println!("{:>10} {:>25.2} {:>25.4}", 
                     i + 1, price, qty);
        }
        
        println!("{}", "=".repeat(70));
        println!("Total depth: {} asks, {} bids", ask_count, bid_count);
    }
}
