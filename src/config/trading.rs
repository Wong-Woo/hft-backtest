pub const TICK_SIZE: f64 = 0.00001;
pub const LOT_SIZE: f64 = 0.001;
pub const INITIAL_CAPITAL: f64 = 10000.0;

pub const PRICE_DECIMAL_PLACES: usize = calculate_decimal_places(TICK_SIZE);

const fn calculate_decimal_places(tick_size: f64) -> usize {
    if (tick_size - 0.00001).abs() < 1e-10 { 5 }
    else if (tick_size - 0.0001).abs() < 1e-9 { 4 }
    else if (tick_size - 0.001).abs() < 1e-8 { 3 }
    else if (tick_size - 0.01).abs() < 1e-7 { 2 }
    else if (tick_size - 0.1).abs() < 1e-6 { 1 }
    else if (tick_size - 1.0).abs() < 1e-5 { 0 }
    else {
        let mut count = 0;
        let mut value = tick_size;
        while value < 1.0 && count < 10 {
            value *= 10.0;
            count += 1;
        }
        count
    }
}
