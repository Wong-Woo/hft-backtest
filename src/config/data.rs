use std::env;

const DEFAULT_DATA_FILE_PATH: &str = r"D:\quant-data\1000PEPEUSDT\1000PEPEUSDT_20240626.npz";

pub fn get_data_file_path() -> String {
    env::var("DATA_FILE_PATH").unwrap_or_else(|_| DEFAULT_DATA_FILE_PATH.to_string())
}
