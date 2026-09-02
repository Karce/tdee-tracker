use crate::model::{Config, Entry, Sex};

pub fn trend(entries: &[Entry], alpha: f64) -> Vec<(chrono::NaiveDate, f64)> {
    let mut result = Vec::new();
    let mut prev_trend: Option<f64> = None;
    for e in entries {
        if let Some(w) = e.weight_lb {
            let t = match prev_trend {
                None => w,
                Some(pt) => pt + alpha * (w - pt),
            };
            prev_trend = Some(t);
            result.push((e.date, t));
        } else if let Some(pt) = prev_trend {
            result.push((e.date, pt));
        }
    }
    result
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Confidence {
    Insufficient,
    Rough,
    Provisional,
    Stable,
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Insufficient => write!(f, "insufficient"),
            Self::Rough => write!(f, "rough"),
            Self::Provisional => write!(f, "provisional"),
            Self::Stable => write!(f, "stable"),
        }
    }
}

pub fn confidence(days_with_both: usize) -> Confidence {
    match days_with_both {
        0..7 => Confidence::Insufficient,
        7..14 => Confidence::Rough,
        14..21 => Confidence::Provisional,
        _ => Confidence::Stable,
    }
}

pub struct TdeeResult {
    pub tdee: f64,
    pub days_with_both: usize,
    pub confidence: Confidence,
}

pub fn adaptive_tdee(entries: &[Entry], config: &Config) -> Option<TdeeResult> {
    let window = config.window_days as usize;
    let start = entries.len().saturating_sub(window);
    let window_entries: Vec<&Entry> = entries[start..].iter().collect();

    let days_with_both = window_entries
        .iter()
        .filter(|e| e.weight_lb.is_some() && e.kcal.is_some())
        .count();

    if days_with_both < 7 {
        return None;
    }

    let trend_vals = trend(&window_entries.iter().copied().cloned().collect::<Vec<_>>(), config.ema_alpha);
    if trend_vals.len() < 2 {
        return None;
    }

    let trend_start = trend_vals.first().unwrap().1;
    let trend_end = trend_vals.last().unwrap().1;

    let first_date = window_entries.first().unwrap().date;
    let last_date = window_entries.last().unwrap().date;
    let span_days = (last_date - first_date).num_days().max(1) as f64;

    let kcal_days: Vec<f64> = window_entries
        .iter()
        .filter_map(|e| e.kcal.map(|k| k as f64))
        .collect();
    let mean_kcal = kcal_days.iter().sum::<f64>() / kcal_days.len() as f64;

    let tdee = mean_kcal - (trend_end - trend_start) * 3500.0 / span_days;

    Some(TdeeResult {
        tdee,
        days_with_both,
        confidence: confidence(days_with_both),
    })
}

pub fn effective_tdee(entries: &[Entry], config: &Config) -> Option<f64> {
    adaptive_tdee(entries, config)
        .map(|r| r.tdee)
        .or(Some(config.seed_tdee_kcal as f64))
}

pub fn target_kcal(effective_tdee: f64, rate_lb_per_week: f64) -> f64 {
    effective_tdee - rate_lb_per_week * 3500.0 / 7.0
}

pub fn observed_rate(entries: &[Entry], alpha: f64) -> Option<f64> {
    let trend_vals = trend(entries, alpha);
    if trend_vals.len() < 2 {
        return None;
    }
    let start = trend_vals.first().unwrap();
    let end = trend_vals.last().unwrap();
    let span_days = (end.0 - start.0).num_days() as f64;
    if span_days < 1.0 {
        return None;
    }
    Some((end.1 - start.1) / span_days * 7.0)
}

pub fn eta_weeks(trend_now: f64, goal: f64, obs_rate: f64) -> Option<f64> {
    if obs_rate >= 0.0 {
        return None;
    }
    Some((trend_now - goal) / obs_rate.abs())
}

pub fn mifflin_st_jeor(weight_kg: f64, height_cm: f64, age: u32, sex: Sex) -> f64 {
    let base = 10.0 * weight_kg + 6.25 * height_cm - 5.0 * age as f64;
    match sex {
        Sex::Male => base + 5.0,
        Sex::Female => base - 161.0,
    }
}

pub fn seed_tdee(weight_lb: f64, height_cm: f64, age: u32, sex: Sex) -> u32 {
    let weight_kg = weight_lb / 2.20462;
    let bmr = mifflin_st_jeor(weight_kg, height_cm, age, sex);
    (bmr * 1.25).round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn make_entries_linear() -> Vec<Entry> {
        let start = NaiveDate::from_ymd_opt(2026, 9, 2).unwrap();
        (0..28)
            .map(|i| {
                let w = 200.0 - 2.0 * i as f64 / 27.0;
                Entry {
                    date: start + chrono::Duration::days(i),
                    weight_lb: Some(w),
                    kcal: Some(2000),
                    note: None,
                }
            })
            .collect()
    }

    fn make_config() -> Config {
        Config {
            start_date: NaiveDate::from_ymd_opt(2026, 9, 2).unwrap(),
            start_weight_lb: 200.0,
            goal_weight_lb: 180.0,
            rate_lb_per_week: 1.0,
            height_cm: 180.0,
            age_years: 35,
            sex: Sex::Male,
            seed_tdee_kcal: 2328,
            ema_alpha: 0.1,
            window_days: 28,
        }
    }

    #[test]
    fn test_trend_linear() {
        let entries = make_entries_linear();
        let config = make_config();
        let t = trend(&entries, config.ema_alpha);
        assert_eq!(t.len(), 28);
        let trend_change = t.last().unwrap().1 - t.first().unwrap().1;
        assert!(
            (trend_change - (-1.372)).abs() < 0.05,
            "trend change was {trend_change}, expected ~-1.372"
        );
    }

    #[test]
    fn test_adaptive_tdee_linear() {
        let entries = make_entries_linear();
        let config = make_config();
        let result = adaptive_tdee(&entries, &config).unwrap();
        assert!(
            (result.tdee - 2178.0).abs() < 15.0,
            "tdee was {}, expected ~2178",
            result.tdee
        );
        assert_eq!(result.confidence, Confidence::Stable);
        assert_eq!(result.days_with_both, 28);
    }

    #[test]
    fn test_target_kcal() {
        let config = make_config();
        let entries = make_entries_linear();
        let result = adaptive_tdee(&entries, &config).unwrap();
        let target = target_kcal(result.tdee, config.rate_lb_per_week);
        assert!(
            (target - 1678.0).abs() < 15.0,
            "target was {target}, expected ~1678"
        );
    }

    #[test]
    fn test_gap_entries() {
        let start = NaiveDate::from_ymd_opt(2026, 9, 2).unwrap();
        let entries: Vec<Entry> = (0..28)
            .map(|i| {
                let w = 200.0 - 2.0 * i as f64 / 27.0;
                let kcal = if i == 5 || i == 12 || i == 20 {
                    None
                } else {
                    Some(2000)
                };
                Entry {
                    date: start + chrono::Duration::days(i),
                    weight_lb: Some(w),
                    kcal,
                    note: None,
                }
            })
            .collect();
        let config = make_config();
        let result = adaptive_tdee(&entries, &config).unwrap();
        assert_eq!(result.days_with_both, 25);
        let kcal_mean = 2000.0;
        assert!(
            (result.tdee - kcal_mean).abs() < 300.0,
            "tdee with gaps was {}",
            result.tdee
        );
    }

    #[test]
    fn test_insufficient_days() {
        let start = NaiveDate::from_ymd_opt(2026, 9, 2).unwrap();
        let entries: Vec<Entry> = (0..5)
            .map(|i| Entry {
                date: start + chrono::Duration::days(i),
                weight_lb: Some(200.0),
                kcal: Some(2000),
                note: None,
            })
            .collect();
        let config = make_config();
        assert!(adaptive_tdee(&entries, &config).is_none());
    }

    #[test]
    fn test_confidence_levels() {
        assert_eq!(confidence(5), Confidence::Insufficient);
        assert_eq!(confidence(7), Confidence::Rough);
        assert_eq!(confidence(14), Confidence::Provisional);
        assert_eq!(confidence(21), Confidence::Stable);
        assert_eq!(confidence(28), Confidence::Stable);
    }

    #[test]
    fn test_mifflin_seed() {
        let weight_lb = 200.0;
        let s = seed_tdee(weight_lb, 180.0, 35, Sex::Male);
        assert!(
            (s as i32 - 2328).abs() <= 2,
            "seed was {s}, expected ~2328"
        );
    }

    #[test]
    fn test_observed_rate() {
        let entries = make_entries_linear();
        let config = make_config();
        let rate = observed_rate(&entries, config.ema_alpha).unwrap();
        assert!(rate < 0.0, "rate should be negative (losing): {rate}");
    }

    #[test]
    fn test_eta() {
        let eta = eta_weeks(197.0, 180.0, -1.0).unwrap();
        assert!((eta - 17.0).abs() < 0.1);
        assert!(eta_weeks(197.0, 180.0, 0.5).is_none());
    }
}
