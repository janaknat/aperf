use crate::{data, InitParams, PDError, TimeType, PERFORMANCE_DATA};
use anyhow::Result;
use clap::Args;
use log::{debug, info};
use std::path::{Path, PathBuf};

#[derive(Args, Debug)]
pub struct Record {
    /// Name of the run.
    #[clap(short, long, value_parser)]
    pub run_name: Option<String>,

    /// Interval at which performance data is to be collected. Use 'ms' to specify milliseconds.
    /// Lowest allowed is 10ms. If unspecified, will be considered as seconds.
    #[clap(short, long, value_parser)]
    pub interval: Option<String>,

    /// Time for which the performance data is to be collected. Use 'ms' to specify milliseconds.
    /// Lowest allowed is 10ms. If unspecified, will be considered as seconds.
    #[clap(short, long, value_parser)]
    pub period: Option<String>,

    /// Gather profiling data using 'perf' binary.
    #[clap(long, value_parser)]
    pub profile: bool,

    /// Profile JVMs using async-profiler. Specify args using comma separated values. Profiles all JVMs if no args are provided.
    #[clap(long, value_parser, default_missing_value = Some("jps"), value_names = &["PID/Name>,<PID/Name>,...,<PID/Name"], num_args = 0..=1)]
    pub profile_java: Option<String>,

    /// Custom PMU config file to use.
    #[clap(long, value_parser)]
    pub pmu_config: Option<String>,
}

fn prepare_data_collectors() -> Result<()> {
    info!("Preparing data collectors...");
    PERFORMANCE_DATA.lock().unwrap().prepare_data_collectors()?;
    Ok(())
}

fn start_collection_serial() -> Result<()> {
    info!("Collecting data...");
    PERFORMANCE_DATA.lock().unwrap().collect_data_serial()?;
    Ok(())
}

fn collect_static_data() -> Result<()> {
    debug!("Collecting static data...");
    PERFORMANCE_DATA.lock().unwrap().collect_static_data()?;
    Ok(())
}

pub fn get_time_value(time_str: String, option: String) -> Result<(u64, TimeType)> {
    let mut value = 1000;
    let mut value_type = TimeType::SECONDS;
    if time_str.ends_with("ms") {
        value = time_str.strip_suffix("ms").unwrap().parse::<u64>()?;
        if value < 10 {
            return Err(PDError::CollectorInvalidParams(format!(
                "Collection {} cannot be less than 10ms.",
                option
            ))
            .into());
        }
        value_type = TimeType::MILLISECONDS;
    } else if time_str.ends_with('s') {
        value *= time_str.strip_suffix('s').unwrap().parse::<u64>()?;
    } else {
        match time_str.parse::<u64>() {
            Ok(v) => value *= v,
            Err(e) => {
                return Err(PDError::CollectorInvalidParams(format!(
                    "Could not parse {} - {}",
                    option, e
                ))
                .into())
            }
        }
    }
    if value == 0 {
        return Err(
            PDError::CollectorInvalidParams(format!("Collection {} cannot be 0.", option)).into(),
        );
    }
    Ok((value, value_type))
}

pub fn record(record: &Record, tmp_dir: &Path, runlog: &Path) -> Result<()> {
    let mut run_name = String::new();
    let mut interval = 1000;
    let mut period = 10000;
    let mut interval_type = TimeType::SECONDS;
    if let Some(i) = &record.interval {
        (interval, interval_type) = get_time_value(i.to_string(), "interval".to_string())?;
    }
    if let Some(i) = &record.period {
        (period, _) = get_time_value(i.to_string(), "period".to_string())?;
    }
    match &record.run_name {
        Some(r) => run_name = r.clone(),
        None => {}
    }
    *(data::INTERVAL_TYPE).lock().unwrap() = interval_type.clone();
    let mut params = InitParams::new(run_name);
    params.period_in_ms = period.try_into()?;
    params.interval_in_ms = interval.try_into()?;
    params.interval_type = interval_type;
    params.tmp_dir = tmp_dir.to_path_buf();
    params.runlog = runlog.to_path_buf();
    if let Some(p) = &record.pmu_config {
        params.pmu_config = Some(PathBuf::from(p));
    }

    match &record.profile_java {
        Some(j) => {
            params.profile.insert(
                String::from(data::java_profile::JAVA_PROFILE_FILE_NAME),
                j.clone(),
            );
        }
        None => {}
    }
    if record.profile {
        params.profile.insert(
            String::from(data::perf_profile::PERF_PROFILE_FILE_NAME),
            String::new(),
        );
        params.profile.insert(
            String::from(data::flamegraphs::FLAMEGRAPHS_FILE_NAME),
            String::new(),
        );
    }

    PERFORMANCE_DATA.lock().unwrap().set_params(params);
    PERFORMANCE_DATA.lock().unwrap().init_collectors()?;
    info!("Starting Data collection...");
    prepare_data_collectors()?;
    collect_static_data()?;
    start_collection_serial()?;
    info!("Data collection complete.");
    PERFORMANCE_DATA.lock().unwrap().end()?;

    Ok(())
}
