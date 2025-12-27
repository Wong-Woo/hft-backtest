use anyhow::Result;
use candle_core::{Device, Tensor, DType};
use candle_nn::{Linear, Module, VarBuilder, VarMap, Optimizer, AdamW, ParamsAdamW, linear};
use std::collections::VecDeque;
use super::orderbook_features::OrderBookFeatures;

/// 예측 신호
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PredictionSignal {
    /// 가격 상승 예측 (Long)
    Up,
    /// 가격 하락 예측 (Short)
    Down,
    /// 변화 없음/불확실
    Neutral,
}

/// 학습 샘플
#[derive(Debug, Clone)]
struct TrainingSample {
    features: Vec<f64>,
    target: f64, // 1초 후 가격 변화율
}

/// MLP 기반 가격 예측 모델
/// 
/// 아키텍처:
/// - Input: 오더북 특성 벡터 (8차원)
/// - Hidden1: 32 neurons + ReLU
/// - Hidden2: 16 neurons + ReLU
/// - Output: 1 (가격 변화 예측)
#[allow(dead_code)]
pub struct PricePredictor {
    device: Device,
    varmap: VarMap,
    input_dim: usize,
    hidden1_dim: usize,
    hidden2_dim: usize,
    
    // 모델 레이어
    fc1: Linear,
    fc2: Linear,
    fc3: Linear,
    
    // 온라인 학습용 버퍼
    training_buffer: VecDeque<TrainingSample>,
    buffer_size: usize,
    
    // 예측 이력
    prediction_history: VecDeque<(f64, f64)>, // (예측, 실제)
    
    // 학습 통계
    total_predictions: usize,
    correct_predictions: usize,
    
    // 예측 임계값
    prediction_threshold: f64,
    
    // 특성 정규화 파라미터
    feature_means: Vec<f64>,
    feature_stds: Vec<f64>,
    normalization_samples: usize,
}

#[allow(dead_code)]
impl PricePredictor {
    /// 새 예측 모델 생성
    pub fn new(prediction_threshold: f64) -> Result<Self> {
        let device = Device::Cpu;
        let varmap = VarMap::new();
        let vs = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        
        let input_dim = OrderBookFeatures::feature_dim();
        let hidden1_dim = 32;
        let hidden2_dim = 16;
        
        // Xavier 초기화로 레이어 생성
        let fc1 = linear(input_dim, hidden1_dim, vs.pp("fc1"))?;
        let fc2 = linear(hidden1_dim, hidden2_dim, vs.pp("fc2"))?;
        let fc3 = linear(hidden2_dim, 1, vs.pp("fc3"))?;
        
        Ok(Self {
            device,
            varmap,
            input_dim,
            hidden1_dim,
            hidden2_dim,
            fc1,
            fc2,
            fc3,
            training_buffer: VecDeque::with_capacity(1000),
            buffer_size: 1000,
            prediction_history: VecDeque::with_capacity(100),
            total_predictions: 0,
            correct_predictions: 0,
            prediction_threshold,
            feature_means: vec![0.0; input_dim],
            feature_stds: vec![1.0; input_dim],
            normalization_samples: 0,
        })
    }

    /// Forward pass
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = self.fc1.forward(x)?;
        let x = x.relu()?;
        let x = self.fc2.forward(&x)?;
        let x = x.relu()?;
        let x = self.fc3.forward(&x)?;
        Ok(x)
    }

    /// 특성 정규화 파라미터 업데이트 (온라인 방식)
    fn update_normalization(&mut self, features: &[f64]) {
        self.normalization_samples += 1;
        let n = self.normalization_samples as f64;
        
        for (i, &f) in features.iter().enumerate() {
            // 온라인 평균 계산
            let old_mean = self.feature_means[i];
            self.feature_means[i] = old_mean + (f - old_mean) / n;
            
            // 온라인 표준편차 계산 (Welford's algorithm)
            if n > 1.0 {
                let old_std = self.feature_stds[i];
                let new_mean = self.feature_means[i];
                self.feature_stds[i] = ((old_std.powi(2) * (n - 1.0) + (f - old_mean) * (f - new_mean)) / n).sqrt();
            }
        }
    }

    /// 특성 정규화
    fn normalize_features(&self, features: &[f64]) -> Vec<f64> {
        features.iter().enumerate().map(|(i, &f)| {
            let std = if self.feature_stds[i] > 1e-8 { self.feature_stds[i] } else { 1.0 };
            (f - self.feature_means[i]) / std
        }).collect()
    }

    /// 예측 수행
    pub fn predict(&mut self, features: &OrderBookFeatures) -> Result<(f64, PredictionSignal)> {
        let feature_vec = features.to_vec();
        
        // 정규화 파라미터 업데이트
        self.update_normalization(&feature_vec);
        
        // 특성 정규화
        let normalized = self.normalize_features(&feature_vec);
        
        // 텐서로 변환
        let input = Tensor::new(&normalized[..], &self.device)?
            .to_dtype(DType::F32)?
            .reshape((1, self.input_dim))?;
        
        // 예측
        let output = self.forward(&input)?;
        let prediction = output.squeeze(0)?.squeeze(0)?.to_scalar::<f32>()? as f64;
        
        self.total_predictions += 1;
        
        // 신호 생성
        let signal = if prediction > self.prediction_threshold {
            PredictionSignal::Up
        } else if prediction < -self.prediction_threshold {
            PredictionSignal::Down
        } else {
            PredictionSignal::Neutral
        };
        
        Ok((prediction, signal))
    }

    /// 학습 샘플 추가 (1초 후 실제 가격 변화와 함께)
    pub fn add_training_sample(&mut self, features: &OrderBookFeatures, price_change_pct: f64) {
        let sample = TrainingSample {
            features: features.to_vec(),
            target: price_change_pct,
        };
        
        self.training_buffer.push_back(sample);
        if self.training_buffer.len() > self.buffer_size {
            self.training_buffer.pop_front();
        }
        
        // 예측 정확도 추적
        if let Some((pred, _actual)) = self.prediction_history.back() {
            let pred_direction = if *pred > 0.0 { 1.0 } else { -1.0 };
            let actual_direction = if price_change_pct > 0.0 { 1.0 } else { -1.0 };
            if pred_direction * actual_direction > 0.0 {
                self.correct_predictions += 1;
            }
        }
    }

    /// 배치 학습 수행
    pub fn train_batch(&mut self, batch_size: usize, learning_rate: f64) -> Result<f64> {
        if self.training_buffer.len() < batch_size {
            return Ok(0.0);
        }

        // 무작위 배치 샘플링
        let samples: Vec<_> = self.training_buffer
            .iter()
            .rev()
            .take(batch_size)
            .cloned()
            .collect();

        // 입력/타겟 텐서 생성
        let mut inputs = Vec::with_capacity(batch_size * self.input_dim);
        let mut targets = Vec::with_capacity(batch_size);
        
        for sample in &samples {
            let normalized = self.normalize_features(&sample.features);
            inputs.extend(normalized);
            targets.push(sample.target as f32);
        }

        let input_tensor = Tensor::new(&inputs[..], &self.device)?
            .to_dtype(DType::F32)?
            .reshape((batch_size, self.input_dim))?;
        
        let target_tensor = Tensor::new(&targets[..], &self.device)?
            .reshape((batch_size, 1))?;

        // 옵티마이저 설정
        let params = ParamsAdamW {
            lr: learning_rate,
            ..Default::default()
        };
        let mut optimizer = AdamW::new(self.varmap.all_vars(), params)?;

        // Forward pass
        let predictions = self.forward(&input_tensor)?;
        
        // MSE Loss
        let diff = predictions.sub(&target_tensor)?;
        let loss = diff.sqr()?.mean_all()?;
        let loss_val = loss.to_scalar::<f32>()? as f64;

        // Backward pass
        optimizer.backward_step(&loss)?;

        Ok(loss_val)
    }

    /// 온라인 학습 (한 샘플씩)
    pub fn online_train(&mut self, features: &OrderBookFeatures, target: f64, learning_rate: f64) -> Result<f64> {
        self.add_training_sample(features, target);
        
        // 일정 샘플 수집 후 배치 학습
        if self.training_buffer.len() >= 64 && self.training_buffer.len() % 32 == 0 {
            return self.train_batch(32, learning_rate);
        }
        
        Ok(0.0)
    }

    /// 예측 정확도 반환
    pub fn get_accuracy(&self) -> f64 {
        if self.total_predictions == 0 {
            return 0.0;
        }
        self.correct_predictions as f64 / self.total_predictions as f64
    }

    /// 통계 초기화
    #[allow(dead_code)]
    pub fn reset_stats(&mut self) {
        self.total_predictions = 0;
        self.correct_predictions = 0;
        self.prediction_history.clear();
    }

    /// 학습 샘플 수 반환
    pub fn get_training_samples(&self) -> usize {
        self.training_buffer.len()
    }

    /// 모델 준비 여부 (충분한 학습 샘플이 있는지)
    pub fn is_ready(&self) -> bool {
        self.training_buffer.len() >= 100
    }

    /// 예측 기록 추가
    pub fn record_prediction(&mut self, prediction: f64, actual: f64) {
        self.prediction_history.push_back((prediction, actual));
        if self.prediction_history.len() > 100 {
            self.prediction_history.pop_front();
        }
    }

    /// 최근 예측 MAE 계산
    #[allow(dead_code)]
    pub fn get_recent_mae(&self) -> f64 {
        if self.prediction_history.is_empty() {
            return 0.0;
        }
        
        let sum: f64 = self.prediction_history
            .iter()
            .map(|(pred, actual)| (pred - actual).abs())
            .sum();
        
        sum / self.prediction_history.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predictor_creation() {
        let predictor = PricePredictor::new(0.001);
        assert!(predictor.is_ok());
    }

    #[test]
    fn test_prediction() {
        let mut predictor = PricePredictor::new(0.001).unwrap();
        
        let features = OrderBookFeatures {
            mid_price: 100.0,
            spread_bps: 5.0,
            weighted_mid_price: 100.0,
            imbalance_level1: 0.1,
            imbalance_multi_level: 0.05,
            bid_pressure: 1000.0,
            ask_pressure: 900.0,
            pressure_ratio: 0.1,
            price_change_pct: 0.01,
            volatility: 10.0,
            volume_weighted_spread: 5.0,
            trade_intensity: 0.02,
        };

        let result = predictor.predict(&features);
        assert!(result.is_ok());
    }
}
