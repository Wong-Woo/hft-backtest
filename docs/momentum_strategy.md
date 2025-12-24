# Momentum Trading Strategy

모멘텀 기반 트레이딩 전략 구현 문서

## 전략 개요

이 전략은 가격의 모멘텀(momentum)을 추적하여 추세를 포착하는 트렌드 팔로잉 전략입니다.
마켓메이킹과 달리, 시장의 방향성을 예측하고 포지션을 취하는 방향성 전략입니다.

## 핵심 개념

### 1. 모멘텀 (Momentum)

모멘텀은 가격이 일정 기간 동안 얼마나 변화했는지를 측정합니다:

```
Momentum(t, n) = (P_t - P_{t-n}) / P_{t-n}
```

- `P_t`: 현재 가격
- `P_{t-n}`: n 기간 전 가격
- 양수: 상승 추세
- 음수: 하락 추세

### 2. 신호 생성 (Signal Generation)

```rust
if momentum > threshold:
    signal = LONG   // 매수 신호
elif momentum < -threshold:
    signal = SHORT  // 매도 신호
else:
    signal = NEUTRAL
```

### 3. 포지션 관리

#### 진입 조건
- **롱 진입**: 모멘텀 > 임계값
- **숏 진입**: 모멘텀 < -임계값

#### 청산 조건
1. **손절매 (Stop Loss)**: 손실이 일정 비율 초과
2. **익절매 (Take Profit)**: 이익이 목표 비율 도달
3. **반대 신호**: 반대 방향 모멘텀 신호 발생

## 전략 로직

### 1. 모멘텀 지표 계산

```rust
pub struct MomentumIndicator {
    lookback_period: usize,      // 계산 기간
    price_history: VecDeque<f64>, // 가격 히스토리
    momentum_threshold: f64,      // 신호 임계값
}
```

#### 가격 업데이트
```rust
fn update(&mut self, price: f64) {
    self.price_history.push_back(price);
    if self.price_history.len() > self.lookback_period {
        self.price_history.pop_front();
    }
}
```

#### 모멘텀 계산
```rust
fn calculate_momentum(&self) -> Option<f64> {
    if self.price_history.len() < 2 {
        return None;
    }
    let first_price = self.price_history[0];
    let last_price = *self.price_history.back().unwrap();
    Some((last_price - first_price) / first_price)
}
```

### 2. 전략 실행 흐름

```
1. 시장 데이터 수신
   ↓
2. Mid Price 계산 (Bid + Ask) / 2
   ↓
3. 모멘텀 지표 업데이트
   ↓
4. 지표 준비 확인 (lookback_period만큼 데이터 축적)
   ↓
5. 포지션 상태 확인
   ├─ 포지션 보유 중
   │  ├─ 손절/익절 조건 확인
   │  └─ 반대 신호 확인
   └─ 포지션 없음
      └─ 신호 생성 및 진입
```

### 3. 포지션 관리 상태머신

```
     ┌─────────┐
     │  FLAT   │◄──────────────┐
     └────┬────┘               │
          │                    │
    신호발생                   │
          │                    │
     ┌────▼────┐          청산조건
     │  LONG   │               │
     │   or    ├───────────────┘
     │  SHORT  │
     └─────────┘
```

## 구현 세부사항

### 1. 롱 포지션 진입

```rust
fn open_long_position<MD>(&mut self, hbt: &mut Backtest<MD>) 
    -> Result<(), BacktestError>
{
    let depth = hbt.depth(0);
    let best_ask_price = depth.best_ask_tick() as f64 * tick_size;
    
    hbt.submit_buy_order(
        0,
        order_id,
        best_ask_price,
        self.position_size,
        TimeInForce::GTC,
        OrdType::Limit,
        false,
    )?;
}
```

### 2. 청산 조건 확인

```rust
fn should_close_position(&self, current_price: f64) -> bool {
    match self.position_state {
        PositionState::Long => {
            let pnl_pct = (current_price - self.entry_price) / self.entry_price;
            pnl_pct <= -self.stop_loss_pct || pnl_pct >= self.take_profit_pct
        }
        PositionState::Short => {
            let pnl_pct = (self.entry_price - current_price) / self.entry_price;
            pnl_pct <= -self.stop_loss_pct || pnl_pct >= self.take_profit_pct
        }
        _ => false,
    }
}
```

## 파라미터 설정

### 기본 설정 (config.rs)

```rust
pub const MOMENTUM_LOOKBACK_PERIOD: usize = 100;  // 모멘텀 계산 기간
pub const MOMENTUM_THRESHOLD: f64 = 0.002;        // 신호 발생 임계값 (0.2%)
pub const MOMENTUM_POSITION_SIZE: f64 = 0.05;     // 포지션 크기
pub const MOMENTUM_STOP_LOSS_PCT: f64 = 0.01;     // 손절 비율 (1%)
pub const MOMENTUM_TAKE_PROFIT_PCT: f64 = 0.02;   // 익절 비율 (2%)
```

### 파라미터 튜닝 가이드

#### Lookback Period (계산 기간)
- **짧은 기간 (20-50)**: 빠른 신호, 노이즈 많음
- **중간 기간 (50-200)**: 균형잡힌 신호
- **긴 기간 (200+)**: 느린 신호, 안정적

#### Threshold (임계값)
- **낮은 값 (0.001-0.005)**: 민감한 신호, 많은 거래
- **중간 값 (0.005-0.02)**: 적절한 신호 빈도
- **높은 값 (0.02+)**: 강한 추세만 포착

#### Stop Loss / Take Profit
- **타이트 (0.005-0.01)**: 빠른 청산, 낮은 리스크
- **보통 (0.01-0.03)**: 균형잡힌 리스크-수익
- **느슨 (0.03+)**: 큰 움직임 추구, 높은 리스크

## 장단점

### 장점
1. ✅ 트렌드 포착에 효과적
2. ✅ 구현이 간단하고 직관적
3. ✅ 명확한 진입/청산 규칙
4. ✅ 방향성 시장에서 높은 수익 가능

### 단점
1. ❌ 횡보장에서 빈번한 손실
2. ❌ 지연된 신호 (lagging indicator)
3. ❌ 급격한 반전에 취약
4. ❌ 거래 비용에 민감

## 개선 방안

### 1. 필터 추가
```rust
// 변동성 필터
if volatility < MIN_VOLATILITY {
    return SignalType::Neutral;  // 횡보장 회피
}

// 볼륨 필터
if volume < MIN_VOLUME {
    return SignalType::Neutral;  // 유동성 부족 회피
}
```

### 2. 다중 타임프레임
```rust
let short_momentum = calc_momentum(20);
let long_momentum = calc_momentum(100);

if short_momentum > 0 && long_momentum > 0 {
    // 두 타임프레임 모두 상승 추세
    signal = SignalType::Long;
}
```

### 3. 동적 임계값
```rust
// 변동성 기반 임계값 조정
let dynamic_threshold = base_threshold * volatility_factor;
```

### 4. 트레일링 스톱
```rust
// 이익이 발생하면 손절가를 올림
if unrealized_pnl > 0 {
    new_stop = entry_price + (current_price - entry_price) * 0.5;
}
```

## 실행 예시

```bash
# 기본 설정으로 실행
cargo run momentum

# 결과 예시
🚀 Momentum Trading Strategy

Parameters:
  Initial Capital: $10000
  Lookback Period: 100
  Momentum Threshold: 0.002 (0.20%)
  Position Size: 0.05
  Stop Loss: 1.00%
  Take Profit: 2.00%

Running momentum strategy on file [1/1]: ...

  🟢 LONG signal detected | Momentum: 0.0025
    ✓ Opened LONG @ 50000.00 qty 0.0500

[Update #1000] Status:
  Market: Bid=50100.00 Ask=50100.10 Mid=50100.05
  Momentum: 0.0032 (0.32%)
  Position: Long @ 50000.00 qty 0.0500
  PnL: Realized=0.00 Unrealized=5.00 Total=5.00
  Equity: 10005.00 (ROI: 0.05%)

  ⚠️  Reverse signal detected, closing LONG position
    ✓ Closed LONG @ 50150.00 | PnL: 7.50 | Fee: 0.20

Final Statistics:
============================================================
Initial Capital: $10000.00
Realized PnL: $45.30
Total Equity: $10045.30
Total Return: 0.45%
============================================================
```

## Market Making vs Momentum 비교

| 특성 | Market Making | Momentum |
|------|--------------|----------|
| **방향성** | 중립 (neutral) | 방향성 (directional) |
| **수익 원천** | 스프레드 | 가격 변화 |
| **리스크** | 재고 리스크 | 방향 리스크 |
| **적합한 시장** | 횡보장, 높은 유동성 | 트렌딩 시장 |
| **거래 빈도** | 매우 높음 | 중간 |
| **포지션 보유** | 단기 | 중단기 |

## 참고 문헌

- Jegadeesh and Titman (1993) - "Returns to Buying Winners and Selling Losers"
- Moskowitz, Ooi, and Pedersen (2012) - "Time series momentum"
- Carhart (1997) - "On Persistence in Mutual Fund Performance"
