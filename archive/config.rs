use std::fs;
use std::io::Write;
use rand::Rng;

pub struct Config {
    pub api_key: String,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let api_key = get_or_create_api_key()?;
        Ok(Self { api_key })
    }
}

fn get_or_create_api_key() -> Result<String, Box<dyn std::error::Error>> {
    let file_path = ".zfswm_api";
    if let Ok(api_key) = fs::read_to_string(file_path) {
        Ok(api_key.trim().to_string())
    } else {
        let api_key: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        let mut file = fs::File::create(file_path)?;
        file.write_all(api_key.as_bytes())?;
        Ok(api_key)
    }
}