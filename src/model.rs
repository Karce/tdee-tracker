use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub config: Config,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub start_date: NaiveDate,
    pub start_weight_lb: f64,
    pub goal_weight_lb: f64,
    pub rate_lb_per_week: f64,
    pub height_cm: f64,
    pub age_years: u32,
    pub sex: Sex,
    pub seed_tdee_kcal: u32,
    pub ema_alpha: f64,
    pub window_days: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Sex {
    Male,
    Female,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub date: NaiveDate,
    pub weight_lb: Option<f64>,
    pub kcal: Option<u32>,
    pub note: Option<String>,
}

impl Document {
    pub fn upsert_entry(&mut self, entry: Entry) {
        match self.entries.binary_search_by_key(&entry.date, |e| e.date) {
            Ok(i) => {
                if entry.weight_lb.is_some() {
                    self.entries[i].weight_lb = entry.weight_lb;
                }
                if entry.kcal.is_some() {
                    self.entries[i].kcal = entry.kcal;
                }
                if entry.note.is_some() {
                    self.entries[i].note = entry.note;
                }
            }
            Err(i) => self.entries.insert(i, entry),
        }
    }
}
