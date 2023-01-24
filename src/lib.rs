#[macro_use]
extern crate lazy_static;

pub mod data;
pub mod visualizer;
use anyhow::Result;
use chrono::prelude::*;
use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json::{self};
use std::collections::HashMap;
use std::{fs, time};
use std::sync::Mutex;
use thiserror::Error;
use timerfd::{SetTimeFlags, TimerFd, TimerState};

#[derive(Error, Debug)]
pub enum PDError {
    #[error("Error getting JavaScript file for {}", .0)]
    VisualizerJSFileGetError(String),

    #[error("Error getting HashMap entry for {}", .0)]
    VisualizerHashMapEntryError(String),

    #[error("Error getting run values for {}", .0)]
    VisualizerRunValueGetError(String),

    #[error("Error getting Vmstat value for {}", .0)]
    VisualizerVmstatValueGetError(String),

    #[error("Error getting Line Name Error")]
    CollectorLineNameError,

    #[error("Error getting Line Value Error")]
    CollectorLineValueError,

    #[error("Unsupported API")]
    VisualizerUnsupportedAPI,
}

lazy_static! {
    pub static ref PERFORMANCE_DATA: Mutex<PerformanceData> = Mutex::new(PerformanceData::new());
}

#[allow(missing_docs)]
pub struct PerformanceData {
    pub collectors: HashMap<String, data::DataType>,
    pub init_params: InitParams,
}

impl PerformanceData {
    pub fn new() -> Self {
        let collectors = HashMap::new();
        let init_params = InitParams::new();

        PerformanceData {
            collectors,
            init_params,
        }
    }

    pub fn set_params(&mut self, params: InitParams) {
        self.init_params = params;
    }

    pub fn add_datatype(&mut self, name: String, dt: data::DataType) {
        self.collectors.insert(name, dt);
    }

    pub fn init_collectors(&mut self) -> Result<()> {
        let _ret = fs::create_dir_all(self.init_params.dir_name.clone()).unwrap();

        /*
         * Create a meta_data.yaml to hold the InitParams that was used by the collector.
         * This will help when we visualize the data and we don't have to guess these values.
         */
        let meta_data_path = format!("{}/meta_data.yaml", self.init_params.dir_name.clone());
        let meta_data_handle = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(meta_data_path.clone())
            .expect("Could not create meta-data file");

        serde_yaml::to_writer(meta_data_handle, &self.init_params)?;

        for (_name, datatype) in self.collectors.iter_mut() {
            datatype.init_data_type(self.init_params.clone())?;
        }
        Ok(())
    }

    pub fn collect_static_data(&mut self) -> Result<()> {
        for (_name, datatype) in self.collectors.iter_mut() {
            if !datatype.is_static {
                continue;
            }
            datatype.collect_data()?;
            datatype.print_to_file()?;
        }

        Ok(())
    }

    pub fn collect_data_serial(&mut self) -> Result<()> {
        let start = time::Instant::now();
        let mut current = time::Instant::now();
        let end = current + time::Duration::from_secs(self.init_params.period);

        let mut tfd = TimerFd::new().unwrap();
        tfd.set_state(
            TimerState::Periodic {
                current: time::Duration::from_secs(self.init_params.interval),
                interval: time::Duration::from_secs(self.init_params.interval),
            },
            SetTimeFlags::Default,
        );
        while current <= end {
            let ret = tfd.read();
            if ret > 1 {
                error!("Missed {} interval(s)", ret - 1);
            }
            info!("Time elapsed: {:?}", start.elapsed());
            current += time::Duration::from_secs(ret);
            for (_name, datatype) in self.collectors.iter_mut() {
                if datatype.is_static {
                    continue;
                }
                datatype.collect_data()?;
                datatype.print_to_file()?;
            }
            let data_collection_time = time::Instant::now() - current;
            info!("Collection time: {:?}", data_collection_time);
        }
        tfd.set_state(TimerState::Disarmed, SetTimeFlags::Default);
        Ok(())
    }
}

impl Default for PerformanceData {
    fn default() -> Self {
        Self::new()
    }
}

pub fn get_file(dir: String, name: String) -> Result<fs::File> {
    for path in fs::read_dir(dir.clone()).unwrap() {
        let mut file_name = path?.file_name().into_string().unwrap();
        if file_name.contains(&name) {
            file_name = dir + "/" + &file_name;
            return Ok(fs::OpenOptions::new()
                .read(true)
                .open(file_name)
                .expect("Could not open file")
            );
        }
    }
    panic!("File not found");
}

lazy_static! {
    pub static ref VISUALIZATION_DATA: Mutex<VisualizationData> = Mutex::new(VisualizationData::new());
}

pub struct VisualizationData {
    pub visualizers: HashMap<String, visualizer::DataVisualizer>,
    pub js_files: HashMap<String, String>,
    pub run_names: Vec<String>,
}

impl VisualizationData {
    pub fn new() -> Self {
        VisualizationData {
            visualizers: HashMap::new(),
            js_files: HashMap::new(),
            run_names: Vec::new(),
        }
    }

    pub fn init_visualizers(&mut self, dir: String) -> Result<String, tide::Error> {
        let meta_data_file_handle = get_file(dir.clone(), "meta_data".to_string())?;
        let mut params = InitParams::new();
        for document in serde_yaml::Deserializer::from_reader(meta_data_file_handle) {
            params = InitParams::deserialize(document)?;
            self.run_names.push(params.run_name.clone());
        }

        for (_name, visualizer) in self.visualizers.iter_mut() {
            visualizer.init_visualizer(dir.clone(), params.run_name.clone())?;
        }
        Ok(params.run_name.clone())
    }

    pub fn add_visualizer(&mut self, name: String, dv: visualizer::DataVisualizer) {
        self.js_files.insert(dv.js_file_name.clone(), dv.js.clone());
        self.visualizers.insert(name, dv);
    }

    pub fn get_js_file(&mut self, name: String) -> Result<&str> {
        let file = self.js_files.get_mut(&name).ok_or(PDError::VisualizerJSFileGetError(name.to_string().into()))?;
        Ok(file)
    }

    pub fn unpack_data(&mut self, name: String) -> Result<()> {
        for (_, datavisualizer) in self.visualizers.iter_mut() {
            datavisualizer.process_raw_data(name.clone())?;
        }
        Ok(())
    }

    pub fn get_run_names(&mut self) -> Result<String> {
        Ok(serde_json::to_string(&self.run_names)?)
    }

    pub fn get_data(&mut self, name: &str, query: String) -> Result<String> {
        let visualizer = self.visualizers.get_mut(name).ok_or(PDError::VisualizerHashMapEntryError(name.to_string().into()))?;
        visualizer.get_data(query.clone())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InitParams {
    pub time_now: DateTime<Utc>,
    pub time_str: String,
    pub dir_name: String,
    pub period: u64,
    pub interval: u64,
    pub run_name: String,
    pub collector_version: String,
    pub commit_sha_short: String,
}

impl InitParams {
    pub fn new() -> Self {
        let time_now = Utc::now();
        let time_str = time_now.format("%Y-%m-%d_%H_%M_%S").to_string();
        let dir_name = format!("./performance_data_{}", time_str);
        let collector_version = env!("CARGO_PKG_VERSION").to_string();
        let commit_sha_short = env!("VERGEN_GIT_SHA_SHORT").to_string();

        InitParams {
            time_now,
            time_str,
            dir_name,
            period: 0,
            interval: 0,
            run_name: String::new(),
            collector_version,
            commit_sha_short,
        }
    }
}

impl Default for InitParams {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{InitParams, PerformanceData};
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_performance_data_new() {
        let pd = PerformanceData::new();

        let dir_name = format!(
            "./performance_data_{}",
            pd.init_params.time_now.format("%Y-%m-%d_%H_%M_%S")
        );
        assert!(pd.collectors.is_empty());
        assert!(pd.init_params.dir_name == dir_name);
    }

    #[test]
    fn test_performance_data_dir_creation() {
        let mut params = InitParams::new();
        params.dir_name = format!("./performance_data_dir_creation_{}", params.time_str);

        let mut pd = PerformanceData::new();
        pd.set_params(params.clone());
        pd.init_collectors().unwrap();
        assert!(Path::new(&pd.init_params.dir_name).exists());
        let full_path = format!("{}/meta_data.yaml", params.dir_name.clone());
        assert!(Path::new(&full_path).exists());
        fs::remove_dir_all(pd.init_params.dir_name).unwrap();
    }
}
