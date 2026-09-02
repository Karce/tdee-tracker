mod calc;
mod cli;
mod crypto;
mod model;
mod store;

use anyhow::{bail, Result};
use chrono::Local;
use clap::Parser;

use cli::{Cli, Command};
use model::{Config, Document, Entry};

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init {
            start_weight,
            goal,
            height_cm,
            age,
            sex,
            rate,
            seed_tdee,
            start_date,
            force,
        } => cmd_init(
            start_weight,
            goal,
            height_cm,
            age,
            sex.into(),
            rate,
            seed_tdee,
            start_date,
            force,
        ),
        Command::Log {
            date,
            weight,
            kcal,
            note,
        } => cmd_log(date, weight, kcal, note),
        Command::Status { json, brief } => cmd_status(json, brief),
        Command::History { days, json } => cmd_history(days, json),
        Command::Config {
            goal,
            rate,
            seed_tdee,
            window,
        } => cmd_config(goal, rate, seed_tdee, window),
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_init(
    start_weight: f64,
    goal: f64,
    height_cm: f64,
    age: u32,
    sex: model::Sex,
    rate: f64,
    seed_tdee_override: Option<u32>,
    start_date: Option<chrono::NaiveDate>,
    force: bool,
) -> Result<()> {
    if !force && store::data_file_exists()? {
        bail!("Data file already exists. Use --force to overwrite.");
    }
    let date = start_date.unwrap_or_else(|| Local::now().date_naive());
    let computed_seed = seed_tdee_override
        .unwrap_or_else(|| calc::seed_tdee(start_weight, height_cm, age, sex));
    let doc = Document {
        config: Config {
            start_date: date,
            start_weight_lb: start_weight,
            goal_weight_lb: goal,
            rate_lb_per_week: rate,
            height_cm,
            age_years: age,
            sex,
            seed_tdee_kcal: computed_seed,
            ema_alpha: 0.1,
            window_days: 28,
        },
        entries: Vec::new(),
    };
    store::save(&doc)?;
    println!("Initialised. seed TDEE = {} kcal", computed_seed);
    Ok(())
}

fn cmd_log(
    date: Option<chrono::NaiveDate>,
    weight: Option<f64>,
    kcal: Option<u32>,
    note: Option<String>,
) -> Result<()> {
    if weight.is_none() && kcal.is_none() {
        bail!("At least one of --weight or --kcal is required.");
    }
    let date = date.unwrap_or_else(|| Local::now().date_naive());
    let today = Local::now().date_naive();
    if date > today {
        bail!("Date cannot be in the future.");
    }
    if let Some(w) = weight {
        if !(50.0..=500.0).contains(&w) {
            bail!("Weight must be between 50 and 500 lb.");
        }
    }
    if let Some(k) = kcal {
        if k > 10000 {
            bail!("Kcal must be between 0 and 10000.");
        }
    }
    let mut doc = store::load()?;
    let entry = Entry {
        date,
        weight_lb: weight,
        kcal,
        note,
    };
    doc.upsert_entry(entry);
    store::save(&doc)?;
    let e = doc.entries.iter().find(|e| e.date == date).unwrap();
    print_entry_line(e);
    Ok(())
}

fn print_entry_line(e: &Entry) {
    let w = e
        .weight_lb
        .map(|v| format!("{v:.1} lb"))
        .unwrap_or_else(|| "--".into());
    let k = e
        .kcal
        .map(|v| format!("{v} kcal"))
        .unwrap_or_else(|| "--".into());
    println!("{}  weight {}  kcal {}", e.date, w, k);
}

fn cmd_status(json: bool, brief: bool) -> Result<()> {
    let doc = store::load()?;
    let config = &doc.config;
    let entries = &doc.entries;

    let trend_vals = calc::trend(entries, config.ema_alpha);
    let today_entry = entries.last();
    let trend_now = trend_vals.last().map(|t| t.1);
    let tdee_result = calc::adaptive_tdee(entries, config);
    let eff_tdee = calc::effective_tdee(entries, config);
    let obs_rate = calc::observed_rate(entries, config.ema_alpha);

    let today_weight = today_entry.and_then(|e| e.weight_lb);
    let today_kcal = today_entry.and_then(|e| e.kcal);
    let today_date = today_entry.map(|e| e.date);

    let (tdee_val, conf, days_both) = match &tdee_result {
        Some(r) => (Some(r.tdee), r.confidence, r.days_with_both),
        None => (None, calc::Confidence::Insufficient, 0),
    };

    let target = eff_tdee.map(|t| calc::target_kcal(t, config.rate_lb_per_week));
    let lb_to_goal = trend_now.map(|t| t - config.goal_weight_lb);

    let eta = trend_now.and_then(|t| {
        obs_rate.and_then(|r| {
            if conf as u8 >= calc::Confidence::Rough as u8 {
                calc::eta_weeks(t, config.goal_weight_lb, r)
            } else {
                None
            }
        })
    });

    if json {
        let obj = serde_json::json!({
            "date": today_date,
            "weight_lb": today_weight,
            "trend_lb": trend_now,
            "kcal": today_kcal,
            "tdee": tdee_val.or(Some(config.seed_tdee_kcal as f64)),
            "confidence": format!("{conf}"),
            "days_with_both": days_both,
            "target_kcal": target,
            "observed_rate": obs_rate,
            "lb_to_goal": lb_to_goal,
            "eta_weeks": eta,
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
        return Ok(());
    }

    if brief {
        let trend_str = trend_now
            .map(|t| format!("{t:.1}"))
            .unwrap_or_else(|| "--".into());
        let tdee_str = eff_tdee
            .map(|t| format!("{}", t.round() as i64))
            .unwrap_or_else(|| "--".into());
        let target_str = target
            .map(|t| format!("{}", t.round() as i64))
            .unwrap_or_else(|| "--".into());
        let rate_str = obs_rate
            .map(|r| format!("{r:.1}"))
            .unwrap_or_else(|| "--".into());
        let to_go = lb_to_goal
            .map(|l| format!("{l:.1}"))
            .unwrap_or_else(|| "--".into());
        println!(
            "trend {} \u{b7} TDEE {} ({}) \u{b7} eat \u{2264}{} \u{b7} {} lb/wk \u{b7} {} to go",
            trend_str, tdee_str, conf, target_str, rate_str, to_go
        );
        return Ok(());
    }

    // Human output
    if let Some(e) = today_entry {
        let w = e
            .weight_lb
            .map(|v| format!("{v:.1} lb"))
            .unwrap_or_else(|| "-- lb".into());
        let t = trend_now
            .map(|v| format!("{v:.1} lb"))
            .unwrap_or_else(|| "-- lb".into());
        let k = e
            .kcal
            .map(|v| format!("{v} logged"))
            .unwrap_or_else(|| "-- logged".into());
        println!("{}  weight {}  trend {}  kcal {}", e.date, w, t, k);
    }

    let tdee_display = eff_tdee
        .map(|t| format!("{}", t.round() as i64))
        .unwrap_or_else(|| "--".into());
    let target_display = target
        .map(|t| format!("{}", t.round() as i64))
        .unwrap_or_else(|| "--".into());
    println!(
        "TDEE {} kcal ({}, {}/{} days)  \u{b7}  target {} kcal/day for -{:.1} lb/wk",
        tdee_display, conf, days_both, config.window_days, target_display, config.rate_lb_per_week
    );

    if let Some(rate) = obs_rate {
        let ltg = lb_to_goal.unwrap_or(0.0);
        let eta_str = eta
            .map(|w| format!("~{} wk", w.round() as i64))
            .unwrap_or_else(|| "--".into());
        println!(
            "observed {:.1} lb/wk  \u{b7}  {:.1} lb to {}  \u{b7}  ETA {}",
            rate, ltg, config.goal_weight_lb, eta_str
        );
    }

    Ok(())
}

fn cmd_history(days: u32, json: bool) -> Result<()> {
    let doc = store::load()?;
    let entries = &doc.entries;
    let config = &doc.config;
    let trend_vals = calc::trend(entries, config.ema_alpha);

    let skip = if entries.len() > days as usize {
        entries.len() - days as usize
    } else {
        0
    };

    if json {
        let rows: Vec<serde_json::Value> = entries
            .iter()
            .skip(skip)
            .map(|e| {
                let t = trend_vals.iter().find(|tv| tv.0 == e.date).map(|tv| tv.1);
                serde_json::json!({
                    "date": e.date,
                    "weight_lb": e.weight_lb,
                    "trend_lb": t,
                    "kcal": e.kcal,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    println!("{:<12} {:>8} {:>8} {:>6}", "date", "weight", "trend", "kcal");
    for e in entries.iter().skip(skip) {
        let t = trend_vals.iter().find(|tv| tv.0 == e.date).map(|tv| tv.1);
        let w = e
            .weight_lb
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "--".into());
        let tr = t
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "--".into());
        let k = e
            .kcal
            .map(|v| format!("{v}"))
            .unwrap_or_else(|| "--".into());
        println!("{:<12} {:>8} {:>8} {:>6}", e.date, w, tr, k);
    }
    Ok(())
}

fn cmd_config(
    goal: Option<f64>,
    rate: Option<f64>,
    seed_tdee: Option<u32>,
    window: Option<u32>,
) -> Result<()> {
    let mut doc = store::load()?;
    let changed = goal.is_some() || rate.is_some() || seed_tdee.is_some() || window.is_some();

    if let Some(g) = goal {
        doc.config.goal_weight_lb = g;
    }
    if let Some(r) = rate {
        doc.config.rate_lb_per_week = r;
    }
    if let Some(s) = seed_tdee {
        doc.config.seed_tdee_kcal = s;
    }
    if let Some(w) = window {
        doc.config.window_days = w;
    }

    if changed {
        store::save(&doc)?;
        println!("Config updated.");
    }

    println!(
        "goal={:.1} rate={:.1} seed_tdee={} window={} alpha={}",
        doc.config.goal_weight_lb,
        doc.config.rate_lb_per_week,
        doc.config.seed_tdee_kcal,
        doc.config.window_days,
        doc.config.ema_alpha
    );
    Ok(())
}
