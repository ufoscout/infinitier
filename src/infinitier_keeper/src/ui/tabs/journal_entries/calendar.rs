//! Forgotten Realms (Harptos) calendar, built from the game's own data
//! so the Journal Entries tab renders each timestamp exactly like the
//! engine: `Day N, Hour H (DD Month, Year)`.
//!
//! Everything is read from resources rather than hard-coded:
//!
//! * `MONTHS.2DA` — the ordered list of months and one-day festivals,
//!   each row giving a length in days and a `dialog.tlk` strref for its
//!   name.
//! * `YEARS.2DA` — the campaign start time (game-seconds) and year, plus
//!   the strrefs of the `<DAY> <MONTHNAME>` / `<MONTHNAME>` day-month
//!   format strings.
//! * The outer `Day <GAMEDAYS>, Hour <HOUR> (<DAYANDMONTH>, <YEAR>)`
//!   wrapper is `dialog.tlk` strref 15980 (hard-coded in the engine's
//!   journal UI; resolved from the TLK here, with a literal fallback).
//!
//! This mirrors GemRB's `Calendar` and its `bg2/GUIJRNL.py` date logic.

use infinitier_core::game::GameData;
use infinitier_core::resource::gam::{GameTicks, GameTime};

/// `dialog.tlk` strref of the journal date wrapper format string.
const DATE_FORMAT_STRREF: u32 = 15980;
/// Fallback wrapper if strref 15980 can't be resolved.
const DATE_FORMAT_FALLBACK: &str = "Day <GAMEDAYS>, Hour <HOUR> (<DAYANDMONTH>, <YEAR>)";

/// One entry in the year's calendar: a 30-day month or a 1-day festival.
#[derive(Clone)]
struct Segment {
    days: i64,
    name: String,
    /// True for one-day special days (festivals), which render without a
    /// day-of-month number.
    special: bool,
}

/// The game's calendar, ready to format tick timestamps into dates.
#[derive(Clone)]
pub struct Calendar {
    segments: Vec<Segment>,
    days_in_year: i64,
    /// Day-of-year offset of the campaign start (`STARTTIME` ÷ one day).
    start_day: i64,
    start_year: i64,
    /// `<DAY> <MONTHNAME>` (normal months).
    normal_fmt: String,
    /// `<MONTHNAME>` (festival days).
    special_fmt: String,
    /// `Day <GAMEDAYS>, Hour <HOUR> (<DAYANDMONTH>, <YEAR>)`.
    date_fmt: String,
}

impl Calendar {
    /// Build the calendar from `MONTHS.2DA` + `YEARS.2DA` + `dialog.tlk`.
    /// Returns `None` if any required resource is missing or malformed.
    pub fn load(game_data: &GameData) -> Option<Calendar> {
        let months = game_data.import_2da_by_name("MONTHS").ok()?;
        let years = game_data.import_2da_by_name("YEARS").ok()?;
        let tlk = game_data.dialog_tlk().ok()?;

        // Walk MONTHS.2DA in row order (keys "0", "1", ...): column 0 is
        // the length in days, column 1 the strref of the name.
        let mut segments = Vec::new();
        let mut days_in_year = 0i64;
        let mut i = 0;
        while let Some(row) = months.rows.get(&i.to_string()) {
            let days: i64 = row.first()?.parse().ok()?;
            let name = row
                .get(1)
                .and_then(|s| s.parse::<u32>().ok())
                .and_then(|strref| tlk.get(strref))
                .unwrap_or_default();
            days_in_year += days;
            segments.push(Segment {
                days,
                name,
                special: days == 1,
            });
            i += 1;
        }
        if segments.is_empty() || days_in_year == 0 {
            return None;
        }

        let value = |key: &str| years.rows.get(key).and_then(|r| r.first()).cloned();
        let start_time_secs: u32 = value("STARTTIME")?.parse().ok()?;
        let start_year: i64 = value("STARTYEAR")?.parse().ok()?;
        let normal_fmt = value("NORMALDAYMONTHFORMAT")
            .and_then(|s| s.parse::<u32>().ok())
            .and_then(|strref| tlk.get(strref))
            .unwrap_or_else(|| "<DAY> <MONTHNAME>".to_string());
        let special_fmt = value("SPECIALDAYMONTHFORMAT")
            .and_then(|s| s.parse::<u32>().ok())
            .and_then(|strref| tlk.get(strref))
            .unwrap_or_else(|| "<MONTHNAME>".to_string());

        // STARTTIME is in game-seconds (7200 per day, like `game_time`);
        // reuse GameTime to turn it into a whole-day offset.
        let start_day = i64::from(GameTime::from_game_seconds(start_time_secs).dhm().day);

        let date_fmt = tlk
            .get(DATE_FORMAT_STRREF)
            .unwrap_or_else(|| DATE_FORMAT_FALLBACK.to_string());

        Some(Calendar {
            segments,
            days_in_year,
            start_day,
            start_year,
            normal_fmt,
            special_fmt,
            date_fmt,
        })
    }

    /// Render a journal entry timestamp as `Day N, Hour H (DD Month, Year)`.
    pub fn format(&self, time: GameTicks) -> String {
        let ticks = i64::from(time.ticks());
        let days = ticks / i64::from(GameTicks::PER_DAY);
        let hour = (ticks / i64::from(GameTicks::PER_HOUR)) % 24;

        let total = self.start_day + days;
        let year = self.start_year + total / self.days_in_year;
        let day_of_year = total % self.days_in_year;
        let day_month = self.day_month(day_of_year);

        self.date_fmt
            .replace("<GAMEDAYS>", &days.to_string())
            .replace("<HOUR>", &hour.to_string())
            .replace("<DAYANDMONTH>", &day_month)
            .replace("<YEAR>", &year.to_string())
    }

    /// Resolve a 0-based day-of-year into its `<DAY> <MONTHNAME>` (or, for
    /// festival days, `<MONTHNAME>`) string.
    fn day_month(&self, day_of_year: i64) -> String {
        let mut rem = day_of_year;
        for seg in &self.segments {
            if rem < seg.days {
                if seg.special {
                    return self.special_fmt.replace("<MONTHNAME>", &seg.name);
                }
                return self
                    .normal_fmt
                    .replace("<DAY>", &(rem + 1).to_string())
                    .replace("<MONTHNAME>", &seg.name);
            }
            rem -= seg.days;
        }
        String::new()
    }
}

