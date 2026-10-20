//! One line of weather for the briefing, pure.
//!
//! A briefing asks whether an umbrella is needed today, a question about the hours still ahead,
//! so [`verdict`] reads the forecast slots rather than the current observation `wttr.in`'s
//! simple format returns. The hour is an argument, resolved in the owner's zone by the caller,
//! because the feed's slots are city-local and the box runs on UTC. [`suspect`] holds two
//! deliberately loose plausibility thresholds: it catches a broken feed, never a real cold front.

/// One forecast slot.
///
/// `precip_mm` is a float because the feed reports tenths of a millimetre the line prints.
#[derive(Debug, Clone, PartialEq)]
pub struct Hour {
    /// 0..=23, local to the place being forecast.
    pub hour: u32,
    /// Degrees Celsius.
    pub temp_c: i32,
    /// Chance of rain, 0..=100.
    pub rain_pct: u32,
    /// Millimetres.
    pub precip_mm: f64,
    /// The feed's own words for the sky.
    pub desc: String,
}

/// The day's verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// The day's low.
    pub low_c: i32,
    /// The day's high.
    pub high_c: i32,
    /// Whether to carry one.
    pub umbrella: bool,
    /// The sentence the briefing prints.
    pub line: String,
}

/// Rain chance at or above which the umbrella is worth carrying.
///
/// wttr.in's `chanceofrain` is coarse; 50% is the point where "it'll probably
/// be fine" stops being a defensible thing to tell someone.
const UMBRELLA_PCT: u32 = 50;
/// Below [`UMBRELLA_PCT`] and at or above this, the umbrella is the safe call.
const MAYBE_PCT: u32 = 30;

/// A coastal city does not lose this much between two three-hour slots.
const MAX_SLOT_SWING_C: i32 = 15;
/// Nor swing this much between its daily minimum and maximum.
const MAX_DAILY_SPREAD_C: i32 = 20;

/// The forecast slots still ahead: a past downpour must not drive the verdict, nor an early dry
/// spell hide an evening one. When every slot is behind, the last one stands in.
#[must_use]
pub fn remaining(hours: &[Hour], now_hour: u32) -> Vec<Hour> {
    let ahead: Vec<Hour> = hours
        .iter()
        .filter(|slot| slot.hour >= now_hour)
        .cloned()
        .collect();
    if ahead.is_empty() {
        hours.last().cloned().into_iter().collect()
    } else {
        ahead
    }
}

/// When the wettest hour ahead falls, in the words a person uses.
fn when(peak: &Hour) -> String {
    let part = if peak.hour >= 20 {
        "tonight"
    } else if peak.hour >= 17 {
        "this evening"
    } else if peak.hour >= 12 {
        "this afternoon"
    } else {
        "this morning"
    };
    format!(
        "{}% {part} ({:02}:00, {}mm)",
        peak.rain_pct, peak.hour, peak.precip_mm
    )
}

/// The day summary and the umbrella call, from the hours still ahead; `None` when the feed held
/// no hours at all.
#[must_use]
pub fn verdict(hours: &[Hour], now_hour: u32, low_c: i32, high_c: i32) -> Option<Verdict> {
    let ahead = remaining(hours, now_hour);
    let peak = ahead.iter().max_by(|a, b| {
        (a.rain_pct, a.precip_mm)
            .partial_cmp(&(b.rain_pct, b.precip_mm))
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;
    let temps = format!("{low_c}-{high_c}°C");
    let (umbrella, line) = if peak.rain_pct >= UMBRELLA_PCT {
        (
            true,
            format!("{temps}, rain likely — {}. Take an umbrella.", when(peak)),
        )
    } else if peak.rain_pct >= MAYBE_PCT {
        (
            true,
            format!(
                "{temps}, rain possible — {}. Umbrella is the safe call.",
                when(peak)
            ),
        )
    } else {
        (
            false,
            format!(
                "{temps}, {}, rain chance peaks at {}% — no umbrella needed.",
                ahead[0].desc.to_lowercase(),
                peak.rain_pct
            ),
        )
    };
    Some(Verdict {
        low_c,
        high_c,
        umbrella,
        line,
    })
}

/// Why this payload should not be trusted, or `None` if it looks like weather.
///
/// A reason rather than a refusal, so the briefing still has a line; it judges the data, so the
/// warning covers the whole line, rain included.
#[must_use]
pub fn suspect(hours: &[Hour], low_c: i32, high_c: i32) -> Option<String> {
    if high_c - low_c > MAX_DAILY_SPREAD_C {
        return Some(format!(
            "daily range {low_c}-{high_c}°C spans {}°C",
            high_c - low_c
        ));
    }
    hours.windows(2).find_map(|pair| {
        let (a, b) = (&pair[0], &pair[1]);
        let swing = (b.temp_c - a.temp_c).abs();
        (swing > MAX_SLOT_SWING_C).then(|| {
            format!(
                "{swing}°C jump between {:02}:00 ({}°C) and {:02}:00 ({}°C)",
                a.hour, a.temp_c, b.hour, b.temp_c
            )
        })
    })
}

/// The one line, with any caveat on the same line because the only consumer reads one line.
#[must_use]
pub fn line(hours: &[Hour], now_hour: u32, low_c: i32, high_c: i32) -> String {
    let Some(verdict) = verdict(hours, now_hour, low_c, high_c) else {
        return "no forecast: the feed named no hours for today.".to_owned();
    };
    match suspect(hours, low_c, high_c) {
        None => verdict.line,
        Some(why) => format!(
            "{}  [UNTRUSTED FEED: {why} — cross-check before reporting]",
            verdict.line
        ),
    }
}

/// wttr.in's `j1` payload, read into today's slots and the day's range.
///
/// A missing or misshapen field is a refusal here rather than a `0` that reaches a sentence; the
/// feed's numbers are strings, so each is parsed.
pub fn parse(payload: &serde_json::Value) -> Result<(Vec<Hour>, i32, i32), WeatherError> {
    let today = payload
        .get("weather")
        .and_then(|w| w.get(0))
        .ok_or(WeatherError::Unreadable("no weather[0]"))?;
    let number =
        |value: Option<&serde_json::Value>, what: &'static str| -> Result<i32, WeatherError> {
            value
                .and_then(serde_json::Value::as_str)
                .and_then(|text| text.trim().parse().ok())
                .ok_or(WeatherError::Unreadable(what))
        };
    let hours = today
        .get("hourly")
        .and_then(serde_json::Value::as_array)
        .ok_or(WeatherError::Unreadable("no hourly"))?
        .iter()
        .map(|slot| {
            Ok(Hour {
                // wttr.in spells the hour as "0", "300", "600" …
                hour: u32::try_from(number(slot.get("time"), "no time")? / 100)
                    .map_err(|_| WeatherError::Unreadable("a negative hour"))?,
                temp_c: number(slot.get("tempC"), "no tempC")?,
                rain_pct: u32::try_from(number(slot.get("chanceofrain"), "no chanceofrain")?)
                    .map_err(|_| WeatherError::Unreadable("a negative chanceofrain"))?,
                precip_mm: slot
                    .get("precipMM")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|text| text.trim().parse().ok())
                    .ok_or(WeatherError::Unreadable("no precipMM"))?,
                desc: slot
                    .get("weatherDesc")
                    .and_then(|d| d.get(0))
                    .and_then(|d| d.get("value"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned(),
            })
        })
        .collect::<Result<Vec<Hour>, WeatherError>>()?;
    Ok((
        hours,
        number(today.get("mintempC"), "no mintempC")?,
        number(today.get("maxtempC"), "no maxtempC")?,
    ))
}

/// The forecast could not be read.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WeatherError {
    /// The payload is not the shape this feed documents.
    #[error("the forecast could not be read: {0}")]
    Unreadable(&'static str),
    /// The feed could not be reached.
    #[error("the forecast could not be fetched: {0}")]
    Unreachable(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hour(hour: u32, temp_c: i32, rain_pct: u32, precip_mm: f64, desc: &str) -> Hour {
        Hour {
            hour,
            temp_c,
            rain_pct,
            precip_mm,
            desc: desc.to_owned(),
        }
    }

    /// A dry morning does not hide rain later in the day.
    #[test]
    fn an_evening_downpour_is_not_hidden_by_a_clear_morning() {
        let day = [
            hour(3, 18, 0, 0.0, "Clear"),
            hour(9, 21, 5, 0.0, "Sunny"),
            hour(18, 19, 79, 2.4, "Light rain"),
        ];
        let said = verdict(&day, 7, 17, 22).expect("a verdict");
        assert!(said.umbrella);
        assert!(said.line.contains("rain likely"), "{}", said.line);
        assert!(said.line.contains("79% this evening"), "{}", said.line);
        assert!(said.line.contains("18:00"), "{}", said.line);
        assert!(said.line.contains("Take an umbrella"), "{}", said.line);
    }

    /// An already-past downpour does not drive the verdict either.
    #[test]
    fn a_downpour_that_already_happened_does_not_drive_the_verdict() {
        let day = [
            hour(3, 18, 90, 5.0, "Heavy rain"),
            hour(12, 21, 5, 0.0, "Sunny"),
            hour(18, 19, 10, 0.0, "Clear"),
        ];
        let said = verdict(&day, 9, 17, 22).expect("a verdict");
        assert!(!said.umbrella, "{}", said.line);
        assert!(said.line.contains("no umbrella needed"), "{}", said.line);
    }

    #[test]
    fn the_three_bands_read_the_way_a_person_would_say_them() {
        let maybe = verdict(&[hour(15, 20, 35, 0.3, "Cloudy")], 9, 15, 22).expect("a verdict");
        assert!(maybe.umbrella);
        assert!(maybe.line.contains("rain possible"), "{}", maybe.line);
        assert!(maybe.line.contains("safe call"), "{}", maybe.line);

        let dry = verdict(&[hour(15, 20, 10, 0.0, "Partly Cloudy")], 9, 15, 22).expect("a verdict");
        assert!(!dry.umbrella);
        // The feed's own words, lower-cased into the sentence.
        assert!(dry.line.contains("partly cloudy"), "{}", dry.line);
    }

    /// A day that is nearly over still gets a verdict, from its last slot.
    #[test]
    fn a_day_with_no_hours_left_falls_back_to_the_last_one() {
        let day = [hour(9, 20, 10, 0.0, "Sunny"), hour(21, 15, 80, 3.0, "Rain")];
        assert_eq!(remaining(&day, 23), vec![day[1].clone()]);
        let said = verdict(&day, 23, 14, 21).expect("a verdict");
        assert!(said.umbrella, "{}", said.line);
        assert!(said.line.contains("tonight"), "{}", said.line);
    }

    /// The two plausibility thresholds, over a feed that fell from 25 °C to 4 °C in three hours.
    #[test]
    fn a_broken_feed_is_named_rather_than_reported_as_weather() {
        let broken = [
            hour(15, 25, 0, 0.0, "Sunny"),
            hour(18, 4, 0, 0.0, "Clear"),
            hour(21, 3, 0, 0.0, "Clear"),
        ];
        let why = suspect(&broken, 2, 25).expect("a reason");
        assert!(why.contains("spans 23°C"), "{why}");

        // A payload whose daily range is fine but whose slots jump.
        let jumpy = [hour(15, 20, 0, 0.0, "Sunny"), hour(18, 2, 0, 0.0, "Clear")];
        let why = suspect(&jumpy, 2, 20).expect("a reason");
        assert!(why.contains("18°C jump"), "{why}");
        assert!(why.contains("15:00"), "{why}");

        // A real day is not suspect.
        assert_eq!(
            suspect(
                &[hour(9, 18, 0, 0.0, "Sunny"), hour(15, 24, 0, 0.0, "Sunny")],
                17,
                25
            ),
            None
        );
    }

    /// The caveat rides on the line.
    #[test]
    fn an_untrusted_feed_says_so_on_the_line_the_briefing_prints() {
        let broken = [hour(15, 25, 0, 0.0, "Sunny"), hour(18, 4, 0, 0.0, "Clear")];
        let printed = line(&broken, 9, 2, 25);
        assert!(printed.contains("UNTRUSTED FEED"), "{printed}");
        assert!(
            printed.contains("cross-check before reporting"),
            "{printed}"
        );

        let fine = line(&[hour(15, 20, 10, 0.0, "Sunny")], 9, 15, 22);
        assert!(!fine.contains("UNTRUSTED"), "{fine}");
    }

    /// The feed's numbers are strings, and its hours are `"0"`, `"300"`, …
    #[test]
    fn the_payload_parses_into_slots_and_a_range() {
        let payload = serde_json::json!({
            "weather": [{
                "mintempC": "17",
                "maxtempC": "24",
                "hourly": [
                    {"time": "0", "tempC": "18", "chanceofrain": "0",
                     "precipMM": "0.0", "weatherDesc": [{"value": "Clear "}]},
                    {"time": "1800", "tempC": "22", "chanceofrain": "79",
                     "precipMM": "2.4", "weatherDesc": [{"value": "Light rain"}]}
                ]
            }]
        });
        let (hours, low, high) = parse(&payload).expect("parses");
        assert_eq!(low, 17);
        assert_eq!(high, 24);
        assert_eq!(hours.len(), 2);
        assert_eq!(hours[0].hour, 0);
        assert_eq!(hours[0].desc, "Clear");
        assert_eq!(hours[1].hour, 18);
        assert_eq!(hours[1].rain_pct, 79);
        assert!((hours[1].precip_mm - 2.4).abs() < 1e-9);
    }

    /// A payload that is not the shape is a refusal, not a zero that reaches a sentence.
    #[test]
    fn a_payload_of_the_wrong_shape_is_refused_by_name() {
        assert_eq!(
            parse(&serde_json::json!({})),
            Err(WeatherError::Unreadable("no weather[0]"))
        );
        assert_eq!(
            parse(&serde_json::json!({"weather": [{"mintempC": "1", "maxtempC": "2"}]})),
            Err(WeatherError::Unreadable("no hourly"))
        );
        assert_eq!(
            parse(&serde_json::json!({"weather": [{"hourly": [], "maxtempC": "2"}]})),
            Err(WeatherError::Unreadable("no mintempC"))
        );
    }

    #[test]
    fn a_forecast_with_no_hours_says_so_rather_than_inventing_a_day() {
        assert_eq!(verdict(&[], 9, 10, 20), None);
        assert!(line(&[], 9, 10, 20).contains("no forecast"));
    }
}
