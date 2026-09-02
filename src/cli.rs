use clap::{Parser, Subcommand, ValueEnum};
use chrono::NaiveDate;

#[derive(Parser)]
#[command(name = "tdee", version, about = "Adaptive TDEE tracker")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Init {
        #[arg(long)]
        start_weight: f64,
        #[arg(long)]
        goal: f64,
        #[arg(long)]
        height_cm: f64,
        #[arg(long)]
        age: u32,
        #[arg(long)]
        sex: SexArg,
        #[arg(long, default_value = "1.0")]
        rate: f64,
        #[arg(long)]
        seed_tdee: Option<u32>,
        #[arg(long)]
        start_date: Option<NaiveDate>,
        #[arg(long)]
        force: bool,
    },
    Log {
        #[arg(long)]
        date: Option<NaiveDate>,
        #[arg(long)]
        weight: Option<f64>,
        #[arg(long)]
        kcal: Option<u32>,
        #[arg(long)]
        note: Option<String>,
    },
    Status {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        brief: bool,
    },
    History {
        #[arg(long, default_value = "30")]
        days: u32,
        #[arg(long)]
        json: bool,
    },
    Config {
        #[arg(long)]
        goal: Option<f64>,
        #[arg(long)]
        rate: Option<f64>,
        #[arg(long)]
        seed_tdee: Option<u32>,
        #[arg(long)]
        window: Option<u32>,
    },
}

#[derive(Clone, ValueEnum)]
pub enum SexArg {
    Male,
    Female,
}

impl From<SexArg> for crate::model::Sex {
    fn from(s: SexArg) -> Self {
        match s {
            SexArg::Male => Self::Male,
            SexArg::Female => Self::Female,
        }
    }
}
