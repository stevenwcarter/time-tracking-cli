use chrono::NaiveDate;
use dirs::home_dir;
use std::path::PathBuf;

pub fn get_time_tracking_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = home_dir().ok_or("Could not determine home directory")?;
    Ok(home.join(".time-tracking"))
}

pub fn create_template_content(date: &NaiveDate) -> String {
    format!("# Time Tracking - {}\n\n", date.format("%Y-%m-%d"))
}
