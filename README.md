# HFT Backtest - Limit Order Market Making

고빈도거래(HFT) 백테스팅 프레임워크 - Limit Order Market Making 전략 구현

## 프로젝트 구조

```
src/
├── config.rs              # 설정 파일
├── main.rs                # 메인 엔트리 포인트
├── common/                # 공통 유틸리티
│   └── data_loader.rs     # 데이터 파일 로딩
├── display/               # 오더북 시각화
│   └── order_book_display.rs
├── print_depth/           # 오더북 출력 모드
│   └── print_depth_runner.rs
├── monitor/               # GUI 성능 모니터
│   ├── mod.rs
│   └── performance_monitor.rs
└── strategy/              # 전략 구현
    ├── market_maker/      # Market Making 전략
    │   ├── market_maker_runner.rs  # 전략 실행
    │   ├── pricing.rs              # 가격 계산 (Micro Price, Imbalance)
    │   ├── spread.rs               # 스프레드 계산 (Avellaneda-Stoikov)
    │   ├── risk_manager.rs         # 리스크 관리 (Toxic Flow Detection)
    │   └── order_manager.rs        # 주문 집행 (Layering)
    └── momentum/          # Momentum 전략
        ├── momentum_runner.rs      # 전략 실행
        └── indicator.rs            # 모멘텀 지표 계산
```

## 실행 방법

### 1. 오더북 출력 모드
```bash
cargo run          # 기본 모드
cargo run print    # 명시적 지정
```

### 2. Market Making 전략
```bash
cargo run mm              # 짧은 명령어
cargo run market-maker    # 긴 명령어
```

### 3. Momentum 전략
```bash
cargo run momentum        # 모멘텀 기반 전략
```

### 4. GUI 모니터와 함께 실행 🖥️
```bash
cargo run mm-gui              # Market Making + GUI
cargo run market-maker-gui    # Market Making + GUI
cargo run momentum-gui        # Momentum + GUI
```

GUI 모니터에서 실시간으로 확인 가능:
- 📈 Equity Curve (자본 곡선)
- 💰 Total PnL (실현/미실현 손익)
- 📊 Position (포지션 크기)
- 💹 Mid Price (중간가)

## 전략 구성 요소 (SOLID Principles)

### Single Responsibility Principle (SRP)

각 모듈이 단일 책임만 가지도록 설계:

- **MicroPriceCalculator**: Micro price 계산만 담당
- **OrderBookImbalance**: Order book imbalance 계산
- **SpreadCalculator**: 최적 스프레드 계산
- **LiquidityDensity**: 유동성 밀도 추정
- **RiskManager**: 리스크 관리 및 Toxic Flow 감지
- **OrderManager**: 주문 집행 및 레이어링

### Dependency Injection

`MarketMakerRunner`는 모든 컴포넌트를 주입받아 사용:

```rust
pub struct MarketMakerRunner {
    micro_price_calc: MicroPriceCalculator,
    imbalance_calc: OrderBookImbalance,
    spread_calc: SpreadCalculator,
    liquidity_density: LiquidityDensity,
    risk_manager: RiskManager,
    order_manager: OrderManager,
    // ...
}
```

## Market Making 전략 로직

### 1. 적정가 계산 (Pricing)

#### Micro Price
```
P_micro = (V_bid × P_ask + V_ask × P_bid) / (V_bid + V_ask)
```

#### Order Book Imbalance
```
ρ = (V_bid - V_ask) / (V_bid + V_ask)
```
- ρ > 0: 매수 압력 → bid 주문을 더 공격적으로
- ρ < 0: 매도 압력 → ask 주문을 더 공격적으로

### 2. 스프레드 계산 (Avellaneda-Stoikov Model)

#### Reservation Price (재고 리스크 반영)
```
r(s, q, t, σ) = s - q × γ × σ²
```
- s: 중간가 (micro price)
- q: 현재 재고
- γ: 위험 회피 성향 파라미터
- σ²: 시장 변동성

#### Optimal Spread
```
δ = (2/γ) × ln(1 + γ/k)
```
- k: 유동성 밀도 (kappa)

#### 유동성 밀도
```
λ(δ) = A × e^(-k×δ)
```
- λ(δ): 거리 δ에 둔 주문의 예상 체결률
- A: 기본 거래 빈도
- k: Decay rate

### 3. 주문 집행

```
매수(Bid): r - (δ/2)
매도(Ask): r + (δ/2)
```

#### 레이어링 (Layering)
- 여러 가격대에 주문 분산 배치
- 각 레이어별로 수량 조정

### 4. 리스크 관리

#### Toxic Flow Detection
- 급격한 변동성 증가 감지
- 임계치 초과 시 모든 주문 취소

#### 포지션 한도
- 최대 재고 제한
- 한도 초과 시 한쪽 방향 주문만 제출

#### 재고 기반 주문 크기 조정
- 재고가 많을수록 주문 크기 감소

## 설정 (config.rs)

```rust
// 데이터 파일 경로 (glob 패턴 지원)
pub const DATA_FILE_PATH: &str = r"D:\quant-data\BTCUSDT\BTCUSDT_20240626.npz";

// Market Making 파라미터
pub const GAMMA: f64 = 0.1;                    // 위험 회피 성향
pub const INITIAL_KAPPA: f64 = 1.5;            // 유동성 밀도
pub const MAX_INVENTORY: f64 = 10.0;           // 최대 재고
pub const VOLATILITY_THRESHOLD: f64 = 5.0;     // 변동성 임계치

// Momentum 전략 파라미터
pub const MOMENTUM_LOOKBACK_PERIOD: usize = 100;  // 모멘텀 계산 기간
pub const MOMENTUM_THRESHOLD: f64 = 0.002;        // 신호 발생 임계값 (0.2%)
pub const MOMENTUM_POSITION_SIZE: f64 = 0.05;     // 포지션 크기
pub const MOMENTUM_STOP_LOSS_PCT: f64 = 0.01;     // 손절 퍼센트 (1%)
pub const MOMENTUM_TAKE_PROFIT_PCT: f64 = 0.02;   // 익절 퍼센트 (2%)
pub const ORDER_SIZE: f64 = 0.01;              // 기본 주문 크기
pub const DEPTH_LEVELS: usize = 5;             // 오더북 깊이
pub const ORDER_LAYERS: usize = 3;             // 레이어링 개수
```

## 예시 출력

```
🚀 Limit Order Market Making Strategy

Parameters:
  Gamma (γ): 0.1
  Initial Kappa (k): 1.5
  Max Inventory: 10
  ...

Found 1 file(s) matching pattern '...'
  [1] D:\quant-data\BTCUSDT\BTCUSDT_20240626.npz

Running strategy on file [1/1]: ...

--- Strategy Status (Update: 100) ---
  Market: Bid 50000.00 | Ask 50000.10 | Spread 0.10
  Micro Price: 50000.05 | Imbalance: 0.0234
  Volatility: 0.000123
  Inventory: 0.0500 | Realized PnL: 12.34

  Layer 1: Bid @ 49999.95 (4999995), Ask @ 50000.15 (5000015), Size: 0.0100
  Layer 2: Bid @ 49999.94 (4999994), Ask @ 50000.16 (5000016), Size: 0.0067
  Layer 3: Bid @ 49999.93 (4999993), Ask @ 50000.17 (5000017), Size: 0.0050
```

## 참고 문서

- [limit_order_market_making.md](docs/limit_order_market_making.md) - 전략 상세 설명
- Avellaneda & Stoikov (2008) - "High-frequency trading in a limit order book"

## Dependencies

- `hftbacktest`: 백테스팅 프레임워크
- `anyhow`: 에러 처리
- `glob`: 파일 패턴 매칭
- `eframe`: egui 프레임워크 (GUI)
- `egui`: 즉각적인 모드 GUI 라이브러리
- `egui_plot`: egui 플롯/차트 위젯
- `crossbeam-channel`: 스레드 간 통신용 채널

## 기술 스택

- **백테스팅**: hftbacktest (고성능 HFT 시뮬레이션)
- **GUI**: egui (즉각적 모드 GUI, 크로스 플랫폼)
- **멀티스레딩**: 별도 스레드에서 GUI 실행, 채널로 데이터 통신
- **시각화**: egui_plot (실시간 차트 렌더링)
