use chrono::NaiveDate;
use dirs::home_dir;
use std::path::PathBuf;
use std::fs;

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

pub fn create_template_content(date: &NaiveDate, template_file: Option<&str>) -> Result<String, Box<dyn std::error::Error>> {
    match template_file {
        Some(file_path) => {
            // Read the template file
            let template_content = fs::read_to_string(file_path)
                .map_err(|e| format!("Failed to read template file '{}': {}", file_path, e))?;
            
            // Replace {date} placeholder with the formatted date
            let formatted_date = date.format("%Y-%m-%d").to_string();
            Ok(template_content.replace("{date}", &formatted_date))
        }
        None => {
            // Default to empty string if no template file is specified
            Ok("".to_string())
        }
    }
}
