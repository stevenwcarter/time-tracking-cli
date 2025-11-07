use anyhow::{Context, Result};
use dirs::home_dir;
use std::fs;
use std::path::PathBuf;
use time::Date;

use crate::DATE_FORMAT;

pub fn get_time_tracking_dir() -> Result<PathBuf> {
    get_time_tracking_dir_with_override(None)
}

pub fn get_time_tracking_dir_with_override(override_dir: Option<&str>) -> Result<PathBuf> {
    if let Some(dir) = override_dir {
        // If an override directory is provided, use it
        return Ok(PathBuf::from(dir));
    }

    // Default to ~/.time-tracking
    let home = home_dir().context("could not determine home directory")?;
    Ok(home.join(".time-tracking"))
}

pub fn create_template_content(
    date: &Date,
    template_file: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    match template_file {
        Some(file_path) => {
            // Read the template file
            let template_content = fs::read_to_string(file_path)
                .map_err(|e| format!("Failed to read template file '{}': {}", file_path, e))?;

            // Replace {date} placeholder with the formatted date
            let formatted_date = date.format(&DATE_FORMAT).unwrap().to_string();
            Ok(template_content.replace("{date}", &formatted_date))
        }
        None => {
            // Default to empty string if no template file is specified
            Ok("".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use time::macros::date;

    #[test]
    fn test_get_time_tracking_dir_no_override() {
        let result = get_time_tracking_dir();
        assert!(result.is_ok());

        let path = result.unwrap();
        assert!(path.to_string_lossy().ends_with(".time-tracking"));
    }

    #[test]
    fn test_get_time_tracking_dir_with_override_none() {
        let result = get_time_tracking_dir_with_override(None);
        assert!(result.is_ok());

        let path = result.unwrap();
        assert!(path.to_string_lossy().ends_with(".time-tracking"));
    }

    #[test]
    fn test_get_time_tracking_dir_with_override_some() {
        let override_path = "/custom/path/time-tracking";
        let result = get_time_tracking_dir_with_override(Some(override_path));
        assert!(result.is_ok());

        let path = result.unwrap();
        assert_eq!(path, PathBuf::from(override_path));
    }

    #[test]
    fn test_get_time_tracking_dir_with_override_relative_path() {
        let override_path = "relative/path";
        let result = get_time_tracking_dir_with_override(Some(override_path));
        assert!(result.is_ok());

        let path = result.unwrap();
        assert_eq!(path, PathBuf::from(override_path));
    }

    #[test]
    fn test_get_time_tracking_dir_with_override_empty_string() {
        let result = get_time_tracking_dir_with_override(Some(""));
        assert!(result.is_ok());

        let path = result.unwrap();
        assert_eq!(path, PathBuf::from(""));
    }

    #[test]
    fn test_create_template_content_no_template() {
        let date = date!(2023 - 10 - 15);
        let result = create_template_content(&date, None);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_create_template_content_with_template_file() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("template.md");

        let template_content =
            "# Daily Report for {date}\n\n## Tasks\n- Task 1\n- Task 2\n\n## Notes\nDate: {date}";
        fs::write(&template_path, template_content).unwrap();

        let date = date!(2023 - 10 - 15);
        let result = create_template_content(&date, Some(template_path.to_str().unwrap()));

        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(
            content,
            "# Daily Report for 2023-10-15\n\n## Tasks\n- Task 1\n- Task 2\n\n## Notes\nDate: 2023-10-15"
        );
    }

    #[test]
    fn test_create_template_content_multiple_date_placeholders() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("template.md");

        let template_content = "Date: {date}\nFilename: {date}.md\nTitle: Report for {date}";
        fs::write(&template_path, template_content).unwrap();

        let date = date!(2023 - 12 - 25);
        let result = create_template_content(&date, Some(template_path.to_str().unwrap()));

        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(
            content,
            "Date: 2023-12-25\nFilename: 2023-12-25.md\nTitle: Report for 2023-12-25"
        );
    }

    #[test]
    fn test_create_template_content_no_date_placeholders() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("template.md");

        let template_content = "# Static Template\n\nThis template has no date placeholders.";
        fs::write(&template_path, template_content).unwrap();

        let date = date!(2023 - 10 - 15);
        let result = create_template_content(&date, Some(template_path.to_str().unwrap()));

        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(
            content,
            "# Static Template\n\nThis template has no date placeholders."
        );
    }

    #[test]
    fn test_create_template_content_empty_template_file() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("empty_template.md");

        fs::write(&template_path, "").unwrap();

        let date = date!(2023 - 10 - 15);
        let result = create_template_content(&date, Some(template_path.to_str().unwrap()));

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_create_template_content_file_not_found() {
        let non_existent_path = "/path/that/does/not/exist/template.md";
        let date = date!(2023 - 10 - 15);
        let result = create_template_content(&date, Some(non_existent_path));

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Failed to read template file"));
        assert!(error.to_string().contains(non_existent_path));
    }

    #[test]
    fn test_create_template_content_date_formatting() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("template.md");

        let template_content = "Date: {date}";
        fs::write(&template_path, template_content).unwrap();

        // Test different date formats
        let dates_and_expected = [
            (date!(2023 - 1 - 1), "2023-01-01"),
            (date!(2023 - 12 - 31), "2023-12-31"),
            (date!(2024 - 2 - 29), "2024-02-29"), // Leap year
            (date!(2000 - 6 - 15), "2000-06-15"),
        ];

        for (date, expected_date_str) in dates_and_expected {
            let result = create_template_content(&date, Some(template_path.to_str().unwrap()));
            assert!(result.is_ok());
            let content = result.unwrap();
            assert_eq!(content, format!("Date: {}", expected_date_str));
        }
    }

    #[test]
    fn test_create_template_content_complex_template() {
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("complex_template.md");

        let template_content = r#"# Time Tracking Report - {date}

## Summary
Date: {date}
Week: Week of {date}

## Time Entries
| Start | End | Project | Description |
|-------|-----|---------|-------------|
|       |     |         |             |

## Notes
Report generated for {date}

---
*Generated on {date}*"#;
        fs::write(&template_path, template_content).unwrap();

        let date = date!(2023 - 10 - 15);
        let result = create_template_content(&date, Some(template_path.to_str().unwrap()));

        assert!(result.is_ok());
        let content = result.unwrap();

        // Verify all {date} placeholders were replaced
        assert!(!content.contains("{date}"));
        assert!(content.contains("2023-10-15"));

        // Count occurrences of the date
        let date_count = content.matches("2023-10-15").count();
        assert_eq!(date_count, 5); // Should appear 5 times
    }
}
