use chrono::NaiveDate;
use dirs::home_dir;
use std::path::PathBuf;

pub fn get_time_tracking_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    get_time_tracking_dir_with_override(None)
}

pub fn get_time_tracking_dir_with_override(
    override_dir: Option<&str>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(dir) = override_dir {
        // If an override directory is provided, use it
        return Ok(PathBuf::from(dir));
    }

    // Default to ~/.time-tracking
    let home = home_dir().ok_or("Could not determine home directory")?;
    Ok(home.join(".time-tracking"))
}

pub fn create_template_content(_date: &NaiveDate) -> String {
    "".to_string()
}
