# ML 가격 예측 전략 (Price Prediction Strategy)

## 개요

오더북 데이터를 활용하여 **1초 후 가격 변화를 예측**하는 머신러닝 기반 트레이딩 전략입니다.

### 핵심 특징
- **Candle** 딥러닝 프레임워크 사용 (Rust 네이티브)
- 오더북 불균형, 압력, 변동성 등 8가지 특성 추출
- MLP(Multi-Layer Perceptron) 신경망 기반 예측
- 온라인 학습으로 실시간 모델 개선

## 아키텍처

```
┌──────────────────────────────────────────────────────────────┐
│                    Orderbook Data                             │
│   (Best Bid/Ask, 다층 수량, 스프레드, 가격 변화)               │
└───────────────────────┬──────────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────────┐
│              OrderBookFeatureExtractor                        │
│  - spread_bps (스프레드, basis points)                        │
│  - imbalance_level1 (1차 레벨 불균형)                         │
│  - imbalance_multi_level (다층 불균형)                        │
│  - pressure_ratio (bid/ask 압력 비율)                        │
│  - price_change_pct (가격 변화율)                             │
│  - volatility (변동성)                                        │
│  - volume_weighted_spread (수량 가중 스프레드)                 │
│  - trade_intensity (거래 강도)                                │
└───────────────────────┬──────────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────────┐
│                   PricePredictor (MLP)                        │
│                                                              │
│   Input (8) → Hidden1 (32, ReLU) → Hidden2 (16, ReLU) → Out  │
│                                                              │
│   Output: 예측 가격 변화율 (%)                                │
└───────────────────────┬──────────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────────┐
│               PredictionSignal                               │
│   - Up: 예측값 > threshold → Long 포지션                      │
│   - Down: 예측값 < -threshold → Short 포지션                  │
│   - Neutral: 포지션 유지                                      │
└──────────────────────────────────────────────────────────────┘
```

## 파일 구조

```
src/strategy/prediction/
├── mod.rs                  # 모듈 공개 인터페이스
├── orderbook_features.rs   # 오더북 특성 추출기
├── price_predictor.rs      # MLP 신경망 예측 모델
└── prediction_runner.rs    # 전략 실행기
```

## 사용법

### 실행
```bash
# 기본 실행 (ML Prediction 전략이 기본)
cargo run

# 또는 명시적으로
cargo run predict
cargo run prediction
cargo run ml
```

### 설정 (config.rs)

```rust
// 포지션 크기
pub const PREDICTION_POSITION_SIZE: f64 = 0.05;

// 손절/익절 비율
pub const PREDICTION_STOP_LOSS_PCT: f64 = 0.005;     // 0.5%
pub const PREDICTION_TAKE_PROFIT_PCT: f64 = 0.01;    // 1%

// 예측 신뢰도 임계값 (이 이상일 때만 포지션 진입)
pub const PREDICTION_CONFIDENCE_THRESHOLD: f64 = 0.001;  // 0.1%

// 온라인 학습 학습률
pub const PREDICTION_LEARNING_RATE: f64 = 0.001;
```

## 전략 로직

### 1. Warmup 단계
- 처음 1000 샘플 동안은 학습만 수행
- 모델이 충분히 학습된 후 거래 시작

### 2. 예측 및 거래
1. 오더북에서 8개 특성 추출
2. 신경망으로 1초 후 가격 변화 예측
3. 예측값이 임계값 이상이면 포지션 진입
   - 양수: Long (가격 상승 예측)
   - 음수: Short (가격 하락 예측)

### 3. 온라인 학습
- 1초 후 실제 가격 변화와 예측 비교
- 오차를 기반으로 모델 가중치 업데이트
- 시장 변화에 적응하는 모델

### 4. 리스크 관리
- Stop-loss / Take-profit 기반 자동 청산
- 최대 포지션 보유 시간 제한 (5초)
- 반대 신호 시 즉시 청산

## 추출 특성 상세

| 특성 | 설명 | 범위 |
|------|------|------|
| `spread_bps` | 스프레드 (basis points) | 0 ~ ∞ |
| `imbalance_level1` | 1차 레벨 bid/ask 수량 불균형 | -1 ~ 1 |
| `imbalance_multi_level` | 다층(10레벨) 가중 불균형 | -1 ~ 1 |
| `pressure_ratio` | log(bid_qty / ask_qty) | -∞ ~ ∞ |
| `price_change_pct` | 최근 가격 변화율 | % |
| `volatility` | 가격 변동 표준편차 | bps |
| `volume_weighted_spread` | 수량 가중 스프레드 | 0 ~ ∞ |
| `trade_intensity` | 전체 수량 변화율 | % |

## 모델 아키텍처

```
Layer           Shape           Activation
─────────────────────────────────────────
Input           (8,)            -
FC1             (8, 32)         ReLU
FC2             (32, 16)        ReLU
FC3             (16, 1)         Linear
─────────────────────────────────────────
Total params: 8*32 + 32 + 32*16 + 16 + 16*1 + 1 = 817
```

## 성능 지표

전략 실행 중 다음 지표가 실시간으로 표시됩니다:

- **Prediction Accuracy**: 방향 예측 정확도 (%)
- **Training Samples**: 학습에 사용된 샘플 수
- **Win Rate**: 수익 거래 비율
- **Total Returns**: 총 수익률

## 개선 방향

1. **더 복잡한 모델**: LSTM, Transformer 등 시계열 모델 적용
2. **더 많은 특성**: 거래량, 시간대별 패턴 등 추가
3. **앙상블**: 여러 모델의 예측 결합
4. **하이퍼파라미터 튜닝**: 학습률, 네트워크 크기 최적화
