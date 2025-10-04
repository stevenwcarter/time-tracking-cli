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
