use anyhow::{Result, bail};
use time::{Date, Duration, Weekday};

use crate::DATE_FORMAT;

pub fn parse_weekday(day_str: &str) -> Result<Weekday> {
    // eq_ignore_ascii_case avoids the to_lowercase() heap allocation
    let eq = |s: &str| day_str.eq_ignore_ascii_case(s);
    if eq("monday") || eq("mon") {
        return Ok(Weekday::Monday);
    }
    if eq("tuesday") || eq("tue") {
        return Ok(Weekday::Tuesday);
    }
    if eq("wednesday") || eq("wed") {
        return Ok(Weekday::Wednesday);
    }
    if eq("thursday") || eq("thu") {
        return Ok(Weekday::Thursday);
    }
    if eq("friday") || eq("fri") {
        return Ok(Weekday::Friday);
    }
    if eq("saturday") || eq("sat") {
        return Ok(Weekday::Saturday);
    }
    if eq("sunday") || eq("sun") {
        return Ok(Weekday::Sunday);
    }
    bail!(
        "Invalid weekday: '{}'. Valid options are: Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday",
        day_str
    )
}

pub fn get_week_dates(date: &Date, week_start_day: Weekday) -> [Date; 7] {
    // Calculate how many days to go back to reach the week start day
    let current_weekday = date.weekday();
    let days_since_week_start = (current_weekday.number_from_monday() as i32
        - week_start_day.number_from_monday() as i32
        + 7)
        % 7;

    let week_start = *date - Duration::days(days_since_week_start as i64);

    // Generate all 7 days of the week
    std::array::from_fn(|i| week_start + Duration::days(i as i64))
}

pub fn format_day_with_date(date: &Date) -> String {
    format!("{} {}", date.weekday(), date.format(&DATE_FORMAT).unwrap())
}

pub trait WeekdayExt {
    fn short_name(&self) -> &'static str;
}

impl WeekdayExt for Weekday {
    fn short_name(&self) -> &'static str {
        match self {
            Weekday::Monday => "Mon",
            Weekday::Tuesday => "Tue",
            Weekday::Wednesday => "Wed",
            Weekday::Thursday => "Thu",
            Weekday::Friday => "Fri",
            Weekday::Saturday => "Sat",
            Weekday::Sunday => "Sun",
        }
    }
}

#[cfg(test)]
mod tests {
    use time::macros::date;

    use super::*;

    #[test]
    fn test_parse_weekday_full_names() {
        assert_eq!(parse_weekday("Monday").unwrap(), Weekday::Monday);
        assert_eq!(parse_weekday("Tuesday").unwrap(), Weekday::Tuesday);
        assert_eq!(parse_weekday("Wednesday").unwrap(), Weekday::Wednesday);
        assert_eq!(parse_weekday("Thursday").unwrap(), Weekday::Thursday);
        assert_eq!(parse_weekday("Friday").unwrap(), Weekday::Friday);
        assert_eq!(parse_weekday("Saturday").unwrap(), Weekday::Saturday);
        assert_eq!(parse_weekday("Sunday").unwrap(), Weekday::Sunday);
    }

    #[test]
    fn test_parse_weekday_short_names() {
        assert_eq!(parse_weekday("Mon").unwrap(), Weekday::Monday);
        assert_eq!(parse_weekday("Tue").unwrap(), Weekday::Tuesday);
        assert_eq!(parse_weekday("Wed").unwrap(), Weekday::Wednesday);
        assert_eq!(parse_weekday("Thu").unwrap(), Weekday::Thursday);
        assert_eq!(parse_weekday("Fri").unwrap(), Weekday::Friday);
        assert_eq!(parse_weekday("Sat").unwrap(), Weekday::Saturday);
        assert_eq!(parse_weekday("Sun").unwrap(), Weekday::Sunday);
    }

    #[test]
    fn test_parse_weekday_case_insensitive() {
        assert_eq!(parse_weekday("monday").unwrap(), Weekday::Monday);
        assert_eq!(parse_weekday("TUESDAY").unwrap(), Weekday::Tuesday);
        assert_eq!(parse_weekday("WeDnEsDaY").unwrap(), Weekday::Wednesday);
        assert_eq!(parse_weekday("thu").unwrap(), Weekday::Thursday);
        assert_eq!(parse_weekday("FRI").unwrap(), Weekday::Friday);
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
        let date = date!(2023 - 10 - 11);
        let week_dates = get_week_dates(&date, Weekday::Monday);

        assert_eq!(week_dates.len(), 7);
        // Week should start on Monday 2023-10-09
        assert_eq!(week_dates[0], date!(2023 - 10 - 9));
        assert_eq!(week_dates[1], date!(2023 - 10 - 10));
        assert_eq!(week_dates[2], date!(2023 - 10 - 11)); // Input date
        assert_eq!(week_dates[6], date!(2023 - 10 - 15));

        // Verify weekdays
        assert_eq!(week_dates[0].weekday(), Weekday::Monday);
        assert_eq!(week_dates[6].weekday(), Weekday::Sunday);
    }

    #[test]
    fn test_get_week_dates_saturday_start() {
        // Test with a Wednesday (2023-10-11)
        let date = date!(2023 - 10 - 11);
        let week_dates = get_week_dates(&date, Weekday::Saturday);

        assert_eq!(week_dates.len(), 7);
        // Week should start on Saturday 2023-10-07
        assert_eq!(week_dates[0], date!(2023 - 10 - 7));
        assert_eq!(week_dates[4], date!(2023 - 10 - 11)); // Input date (Wednesday)
        assert_eq!(week_dates[6], date!(2023 - 10 - 13));

        // Verify weekdays
        assert_eq!(week_dates[0].weekday(), Weekday::Saturday);
        assert_eq!(week_dates[6].weekday(), Weekday::Friday);
    }

    #[test]
    fn test_get_week_dates_sunday_start() {
        // Test with a Monday (2023-10-09)
        let date = date!(2023 - 10 - 9);
        let week_dates = get_week_dates(&date, Weekday::Sunday);

        assert_eq!(week_dates.len(), 7);
        // Week should start on Sunday 2023-10-08
        assert_eq!(week_dates[0], date!(2023 - 10 - 8));
        assert_eq!(week_dates[1], date!(2023 - 10 - 9)); // Input date
        assert_eq!(week_dates[6], date!(2023 - 10 - 14));

        // Verify weekdays
        assert_eq!(week_dates[0].weekday(), Weekday::Sunday);
        assert_eq!(week_dates[6].weekday(), Weekday::Saturday);
    }

    #[test]
    fn test_get_week_dates_same_day_as_week_start() {
        // Test when the input date is the same as week start day
        let saturday = date!(2023 - 10 - 7); // Saturday
        let week_dates = get_week_dates(&saturday, Weekday::Saturday);

        assert_eq!(week_dates.len(), 7);
        assert_eq!(week_dates[0], saturday); // Should start on the same day
        assert_eq!(week_dates[6], date!(2023 - 10 - 13));
    }

    #[test]
    fn test_get_week_dates_end_of_month() {
        // Test with last day of month
        let date = date!(2023 - 10 - 31); // Tuesday
        let week_dates = get_week_dates(&date, Weekday::Monday);

        assert_eq!(week_dates.len(), 7);
        // Week should start on Monday 2023-10-30
        assert_eq!(week_dates[0], date!(2023 - 10 - 30));
        assert_eq!(week_dates[1], date); // Input date
        // Should continue into November
        assert_eq!(week_dates[6], date!(2023 - 11 - 5));
    }

    #[test]
    fn test_get_week_dates_year_boundary() {
        // Test with date near year boundary
        let date = date!(2023 - 12 - 31); // Sunday
        let week_dates = get_week_dates(&date, Weekday::Monday);

        assert_eq!(week_dates.len(), 7);
        // Week should start on Monday 2023-12-25
        assert_eq!(week_dates[0], date!(2023 - 12 - 25));
        assert_eq!(week_dates[6], date); // Input date should be last day
    }

    #[test]
    fn test_format_day_with_date() {
        let monday = date!(2023 - 10 - 9);
        let tuesday = date!(2023 - 10 - 10);
        let wednesday = date!(2023 - 10 - 11);
        let thursday = date!(2023 - 10 - 12);
        let friday = date!(2023 - 10 - 13);
        let saturday = date!(2023 - 10 - 14);
        let sunday = date!(2023 - 10 - 15);

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
        let date_2022 = date!(2022 - 01 - 01); // Saturday
        let date_2024 = date!(2024 - 12 - 25); // Wednesday

        assert_eq!(format_day_with_date(&date_2022), "Saturday 2022-01-01");
        assert_eq!(format_day_with_date(&date_2024), "Wednesday 2024-12-25");
    }

    #[test]
    fn test_format_day_with_date_leap_year() {
        let leap_day = date!(2024 - 2 - 29); // Thursday
        assert_eq!(format_day_with_date(&leap_day), "Thursday 2024-02-29");
    }
}
