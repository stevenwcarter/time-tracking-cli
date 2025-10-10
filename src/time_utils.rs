use chrono::{Datelike, Duration, NaiveDate, Weekday};

pub fn parse_weekday(day_str: &str) -> Result<Weekday, Box<dyn std::error::Error>> {
    match day_str.to_lowercase().as_str() {
        "monday" | "mon" => Ok(Weekday::Mon),
        "tuesday" | "tue" => Ok(Weekday::Tue),
        "wednesday" | "wed" => Ok(Weekday::Wed),
        "thursday" | "thu" => Ok(Weekday::Thu),
        "friday" | "fri" => Ok(Weekday::Fri),
        "saturday" | "sat" => Ok(Weekday::Sat),
        "sunday" | "sun" => Ok(Weekday::Sun),
        _ => Err(format!("Invalid weekday: '{}'. Valid options are: Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday", day_str).into()),
    }
}

pub fn get_week_dates(date: &NaiveDate, week_start_day: Weekday) -> Vec<NaiveDate> {
    // Calculate how many days to go back to reach the week start day
    let current_weekday = date.weekday();
    let days_since_week_start = (current_weekday.num_days_from_monday() as i32
        - week_start_day.num_days_from_monday() as i32
        + 7)
        % 7;

    let week_start = *date - Duration::days(days_since_week_start as i64);

    // Generate all 7 days of the week
    (0..7).map(|i| week_start + Duration::days(i)).collect()
}

pub fn format_day_with_date(date: &NaiveDate) -> String {
    let day_name = match date.weekday().num_days_from_monday() {
        0 => "Monday",
        1 => "Tuesday",
        2 => "Wednesday",
        3 => "Thursday",
        4 => "Friday",
        5 => "Saturday",
        6 => "Sunday",
        _ => unreachable!(),
    };

    format!("{} {}", day_name, date.format("%Y-%m-%d"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_parse_weekday_full_names() {
        assert_eq!(parse_weekday("Monday").unwrap(), Weekday::Mon);
        assert_eq!(parse_weekday("Tuesday").unwrap(), Weekday::Tue);
        assert_eq!(parse_weekday("Wednesday").unwrap(), Weekday::Wed);
        assert_eq!(parse_weekday("Thursday").unwrap(), Weekday::Thu);
        assert_eq!(parse_weekday("Friday").unwrap(), Weekday::Fri);
        assert_eq!(parse_weekday("Saturday").unwrap(), Weekday::Sat);
        assert_eq!(parse_weekday("Sunday").unwrap(), Weekday::Sun);
    }

    #[test]
    fn test_parse_weekday_short_names() {
        assert_eq!(parse_weekday("Mon").unwrap(), Weekday::Mon);
        assert_eq!(parse_weekday("Tue").unwrap(), Weekday::Tue);
        assert_eq!(parse_weekday("Wed").unwrap(), Weekday::Wed);
        assert_eq!(parse_weekday("Thu").unwrap(), Weekday::Thu);
        assert_eq!(parse_weekday("Fri").unwrap(), Weekday::Fri);
        assert_eq!(parse_weekday("Sat").unwrap(), Weekday::Sat);
        assert_eq!(parse_weekday("Sun").unwrap(), Weekday::Sun);
    }

    #[test]
    fn test_parse_weekday_case_insensitive() {
        assert_eq!(parse_weekday("monday").unwrap(), Weekday::Mon);
        assert_eq!(parse_weekday("TUESDAY").unwrap(), Weekday::Tue);
        assert_eq!(parse_weekday("WeDnEsDaY").unwrap(), Weekday::Wed);
        assert_eq!(parse_weekday("thu").unwrap(), Weekday::Thu);
        assert_eq!(parse_weekday("FRI").unwrap(), Weekday::Fri);
    }

    #[test]
    fn test_parse_weekday_invalid() {
        assert!(parse_weekday("Invalid").is_err());
        assert!(parse_weekday("").is_err());
        assert!(parse_weekday("Moonday").is_err());
        assert!(parse_weekday("123").is_err());
        
        // Check error message contains helpful info
        let error = parse_weekday("Invalid").unwrap_err();
        let error_msg = error.to_string();
        assert!(error_msg.contains("Invalid weekday: 'Invalid'"));
        assert!(error_msg.contains("Valid options are:"));
    }

    #[test]
    fn test_get_week_dates_monday_start() {
        // Test with a Wednesday (2023-10-11)
        let date = NaiveDate::from_ymd_opt(2023, 10, 11).unwrap();
        let week_dates = get_week_dates(&date, Weekday::Mon);
        
        assert_eq!(week_dates.len(), 7);
        // Week should start on Monday 2023-10-09
        assert_eq!(week_dates[0], NaiveDate::from_ymd_opt(2023, 10, 9).unwrap());
        assert_eq!(week_dates[1], NaiveDate::from_ymd_opt(2023, 10, 10).unwrap());
        assert_eq!(week_dates[2], NaiveDate::from_ymd_opt(2023, 10, 11).unwrap()); // Input date
        assert_eq!(week_dates[6], NaiveDate::from_ymd_opt(2023, 10, 15).unwrap());
        
        // Verify weekdays
        assert_eq!(week_dates[0].weekday(), Weekday::Mon);
        assert_eq!(week_dates[6].weekday(), Weekday::Sun);
    }

    #[test]
    fn test_get_week_dates_saturday_start() {
        // Test with a Wednesday (2023-10-11)
        let date = NaiveDate::from_ymd_opt(2023, 10, 11).unwrap();
        let week_dates = get_week_dates(&date, Weekday::Sat);
        
        assert_eq!(week_dates.len(), 7);
        // Week should start on Saturday 2023-10-07
        assert_eq!(week_dates[0], NaiveDate::from_ymd_opt(2023, 10, 7).unwrap());
        assert_eq!(week_dates[4], NaiveDate::from_ymd_opt(2023, 10, 11).unwrap()); // Input date (Wed)
        assert_eq!(week_dates[6], NaiveDate::from_ymd_opt(2023, 10, 13).unwrap());
        
        // Verify weekdays
        assert_eq!(week_dates[0].weekday(), Weekday::Sat);
        assert_eq!(week_dates[6].weekday(), Weekday::Fri);
    }

    #[test]
    fn test_get_week_dates_sunday_start() {
        // Test with a Monday (2023-10-09)
        let date = NaiveDate::from_ymd_opt(2023, 10, 9).unwrap();
        let week_dates = get_week_dates(&date, Weekday::Sun);
        
        assert_eq!(week_dates.len(), 7);
        // Week should start on Sunday 2023-10-08
        assert_eq!(week_dates[0], NaiveDate::from_ymd_opt(2023, 10, 8).unwrap());
        assert_eq!(week_dates[1], NaiveDate::from_ymd_opt(2023, 10, 9).unwrap()); // Input date
        assert_eq!(week_dates[6], NaiveDate::from_ymd_opt(2023, 10, 14).unwrap());
        
        // Verify weekdays
        assert_eq!(week_dates[0].weekday(), Weekday::Sun);
        assert_eq!(week_dates[6].weekday(), Weekday::Sat);
    }

    #[test]
    fn test_get_week_dates_same_day_as_week_start() {
        // Test when the input date is the same as week start day
        let saturday = NaiveDate::from_ymd_opt(2023, 10, 7).unwrap(); // Saturday
        let week_dates = get_week_dates(&saturday, Weekday::Sat);
        
        assert_eq!(week_dates.len(), 7);
        assert_eq!(week_dates[0], saturday); // Should start on the same day
        assert_eq!(week_dates[6], NaiveDate::from_ymd_opt(2023, 10, 13).unwrap());
    }

    #[test]
    fn test_get_week_dates_end_of_month() {
        // Test with last day of month
        let date = NaiveDate::from_ymd_opt(2023, 10, 31).unwrap(); // Tuesday
        let week_dates = get_week_dates(&date, Weekday::Mon);
        
        assert_eq!(week_dates.len(), 7);
        // Week should start on Monday 2023-10-30
        assert_eq!(week_dates[0], NaiveDate::from_ymd_opt(2023, 10, 30).unwrap());
        assert_eq!(week_dates[1], date); // Input date
        // Should continue into November
        assert_eq!(week_dates[6], NaiveDate::from_ymd_opt(2023, 11, 5).unwrap());
    }

    #[test]
    fn test_get_week_dates_year_boundary() {
        // Test with date near year boundary
        let date = NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(); // Sunday
        let week_dates = get_week_dates(&date, Weekday::Mon);
        
        assert_eq!(week_dates.len(), 7);
        // Week should start on Monday 2023-12-25
        assert_eq!(week_dates[0], NaiveDate::from_ymd_opt(2023, 12, 25).unwrap());
        assert_eq!(week_dates[6], date); // Input date should be last day
    }

    #[test]
    fn test_format_day_with_date() {
        let monday = NaiveDate::from_ymd_opt(2023, 10, 9).unwrap();
        let tuesday = NaiveDate::from_ymd_opt(2023, 10, 10).unwrap();
        let wednesday = NaiveDate::from_ymd_opt(2023, 10, 11).unwrap();
        let thursday = NaiveDate::from_ymd_opt(2023, 10, 12).unwrap();
        let friday = NaiveDate::from_ymd_opt(2023, 10, 13).unwrap();
        let saturday = NaiveDate::from_ymd_opt(2023, 10, 14).unwrap();
        let sunday = NaiveDate::from_ymd_opt(2023, 10, 15).unwrap();

        assert_eq!(format_day_with_date(&monday), "Monday 2023-10-09");
        assert_eq!(format_day_with_date(&tuesday), "Tuesday 2023-10-10");
        assert_eq!(format_day_with_date(&wednesday), "Wednesday 2023-10-11");
        assert_eq!(format_day_with_date(&thursday), "Thursday 2023-10-12");
        assert_eq!(format_day_with_date(&friday), "Friday 2023-10-13");
        assert_eq!(format_day_with_date(&saturday), "Saturday 2023-10-14");
        assert_eq!(format_day_with_date(&sunday), "Sunday 2023-10-15");
    }

    #[test]
    fn test_format_day_with_date_different_years() {
        let date_2022 = NaiveDate::from_ymd_opt(2022, 1, 1).unwrap(); // Saturday
        let date_2024 = NaiveDate::from_ymd_opt(2024, 12, 25).unwrap(); // Wednesday

        assert_eq!(format_day_with_date(&date_2022), "Saturday 2022-01-01");
        assert_eq!(format_day_with_date(&date_2024), "Wednesday 2024-12-25");
    }

    #[test]
    fn test_format_day_with_date_leap_year() {
        let leap_day = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap(); // Thursday
        assert_eq!(format_day_with_date(&leap_day), "Thursday 2024-02-29");
    }
}
