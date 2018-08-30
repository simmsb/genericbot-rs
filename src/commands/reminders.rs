use serenity::{
    prelude::*,
    framework::standard::{
        StandardFramework,
        CommandError,
    },
    utils::{
        MessageBuilder,
    },
};
use diesel::prelude::*;
use diesel;
use ::PgConnectionManager;
use regex::Regex;
use chrono::{NaiveDateTime, Utc, Datelike, Duration, NaiveDate};
use itertools::Itertools;
use utils::say;


/// Parse a text message into a datetime and the remaining string.
fn recognise_date(mut base_time: NaiveDateTime, date: &str) -> Result<(NaiveDateTime, String), CommandError> {
    // parse out jan(uary) ... stuff etc
    lazy_static! {
        static ref TDIFF_RE: Regex = Regex::new(concat!(
            r"(?:in\s*)?",
            r"(?P<value>\d+)\s*",
            r"(?P<period>",
            r"y(?:ears?)?|",
            r"M|",
            r"months?|",
            r"w(?:eeks?)?|",
            r"d(?:ays?)?|",
            r"h(?:r|(?:our?))?s?|",
            r"m(?:in(?:ute)?s?)?|",
            r"s(?:ec(?:ond)?s?)?)",
            r"\b"
        )).unwrap();

        static ref TDAY_RE: Regex = Regex::new(r"(?i)(?:on\s*)?(?P<day>monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b").unwrap();

        static ref DMONTH_RE: Regex = Regex::new(concat!(
            r"(?i)(?:on\s*)?",
            r"(?P<month>",
            r"jan(?:uary)?|",
            r"feb(?:ruary)?|",
            r"mar(?:ch)?|",
            r"apr(?:il)?|",
            r"may|",
            r"june?|",
            r"july?|",
            r"aug(?:ust)?|",
            r"sep(?:tember)?|",
            r"oct(?:ober)?|",
            r"nov(?:ember)?|",
            r"dec(?:ember)?)",
            r"\s*(?P<value>\d+)",
            r"(?:st|nd|rd|th)?",
            r"\b"
        )).unwrap();

        static ref TOMORROW_RE: Regex = Regex::new(r"(?i)tomorrow\b").unwrap();
    }

    let mut has_parsed = false;

    if TOMORROW_RE.is_match(date) {
        base_time += Duration::days(1);
        has_parsed = true;
    }

    let mut tdiff_parsed = false;

    for caps in TDIFF_RE.captures_iter(date) {
        if has_parsed {
            return Err("Cannot mix 'tomorrow' and delta times.".into());
        }

        let val = i64::from((&caps["value"]).parse::<u32>()?);
        let per = &caps["period"];

        if per == "M" || per.starts_with("mon") { // special case for months
            let yr = base_time.year();
            let mn = base_time.month0() + (val as u32);

            // muh sign conversions
            let yr = (yr as u32 + mn / 12) as i32;
            let mn = mn % 12;

            base_time = base_time
                .with_year(yr).ok_or("Invalid year value from months.")?
                .with_month0(mn).ok_or("Invalid month value.")?;
        } else {
            base_time = match &per[..1] {
                "y" => {
                    let yr = base_time.year() + (val as i32);
                    base_time.with_year(yr).ok_or("Invalid year value.")?
                },
                "w" => base_time + Duration::weeks(val),
                "d" => base_time + Duration::days(val),
                "h" => base_time + Duration::hours(val),
                "m" => base_time + Duration::minutes(val),
                "s" => base_time + Duration::seconds(val),
                _   => unreachable!(),
            };
        }
        tdiff_parsed = true;
    }

    has_parsed |= tdiff_parsed;

    if let Some(caps) = TDAY_RE.captures(date) {
        if has_parsed {
            return Err("Cannot mix weekday and delta time.".into());
        }

        let day = match &(&caps["day"])[..2] {
            "mo" => 0,
            "tu" => 1,
            "we" => 2,
            "th" => 3,
            "fr" => 4,
            "sa" => 5,
            "su" => 6,
            _     => unreachable!(),
        };

        let current_day = base_time.weekday().num_days_from_monday();

        let delta = (day - current_day) % 7;  // if in past, wrap around

        base_time += Duration::days(i64::from(delta));

        has_parsed = true;
    }

    for caps in DMONTH_RE.captures_iter(date) {
        if has_parsed {
            return Err("Cannot mix deltas or have multiple dates and month values.".into());
        }

        let month = &caps["month"];
        let day = (&caps["value"]).parse::<u32>()?;

        let month_num = match &month[..3] {
            "jan" => 1,
            "feb" => 2,
            "mar" => 3,
            "apr" => 4,
            "may" => 5,
            "jun" => 6,
            "jul" => 7,
            "aug" => 8,
            "sep" => 9,
            "oct" => 10,
            "nov" => 11,
            "dec" => 12,
            _     => unreachable!(),
        };

        let current_month_num = base_time.month();

        let updated_value = if current_month_num <= month_num {
            NaiveDate::from_yo(base_time.year(), 1)
        } else {
            NaiveDate::from_yo(base_time.year() + 1, 1)
        };

        base_time = updated_value.and_hms(0, 0, 0)
            .with_month(month_num).ok_or("Bad month provided.")?
            .with_day(day).ok_or("Bad day number provided for that month.")?;

        has_parsed = true;
    }

    if !has_parsed {
        return Err("Could not parse time.".into());
    }

    let replaced = TDIFF_RE.replace_all(date, "");
    let replaced = TDAY_RE.replace_all(&replaced, "");
    let replaced = DMONTH_RE.replace_all(&replaced, "");
    let replaced = TOMORROW_RE.replace_all(&replaced, "");
    let replaced = replaced
        .trim()
        .to_owned();

    Ok((base_time, replaced))
}


fn insert_reminder(ctx: &Context, u_id: i64, c_id: i64, when: NaiveDateTime, now: NaiveDateTime, msg: &str) {
    use models::NewReminder;
    use schema::reminder;

    let reminder = NewReminder {
        user_id: u_id,
        channel_id: c_id,
        text: msg,
        started: &now,
        when: &when,
    };

    let pool = extract_pool!(&ctx);

    diesel::insert_into(reminder::table)
        .values(&reminder)
        .execute(pool)
        .expect("Could not insert reminder");
}


fn list_reminders(ctx: &Context, u_id: i64) -> Vec<(NaiveDateTime, String)> {
    use schema::reminder::dsl::*;

    let pool = extract_pool!(&ctx);

    reminder.filter(user_id.eq(u_id))
        .order(when)
        .select((when, text))
        .load(pool)
        .unwrap()
}


fn delete_reminder(ctx: &Context, u_id: i64, idx: i64) -> bool {
    use diesel::sql_types::BigInt;

    let pool = extract_pool!(&ctx);

    // row_number() is 1 indexed
    let amount = diesel::sql_query(r#"
        DELETE FROM "reminder" WHERE id in (
            SELECT id FROM (
                SELECT id, row_number() OVER (ORDER BY "when" ASC) as row_num
                FROM "reminder" WHERE "user_id" = $1
            ) AS s WHERE s.row_num = $2)
   "#)
        .bind::<BigInt, i64>(u_id)
        .bind::<BigInt, i64>(idx)
        .execute(pool);

    amount.unwrap() > 0
}


pub fn human_timedelta(delta: &Duration) -> String {
    use utils::and_comma_split;

    let days = delta.num_days();
    let (years, days) = (days / 365, days % 365);
    let (weeks, days) = (days / 7, days % 7);
    let hours = delta.num_hours() % 24;
    let minutes = delta.num_minutes() % 60;
    let seconds = delta.num_seconds() % 60;

    let formats = &[(years, "year"),
                    (weeks, "week"),
                    (days, "day"),
                    (hours, "hour"),
                    (minutes, "minute"),
                    (seconds, "second")];

    let parts: Vec<_> = formats.into_iter()
        .filter(|&(x, _)| *x != 0)
        .map(|&(t, s)| {
            format!("{} {}", t, s) + (if t != 1 { "s" } else { "" })
        })
        .collect();

    and_comma_split(&parts)
}


command!(remind_cmd(ctx, msg, args) {
    let time = args.full();

    let now = Utc::now().naive_utc();
    let (when, replaced) = recognise_date(now, &time)?;

    insert_reminder(&ctx, msg.author.id.0 as i64,
                    msg.channel_id.0 as i64,
                    when, now, &replaced);

    let delta = when.signed_duration_since(now);

    void!(say(msg.channel_id, format!("Okay, I'll remind you about '{}' in {}", replaced, human_timedelta(&delta))));
});


command!(remind_list(ctx, msg) {
    let reminders = list_reminders(&ctx, msg.author.id.0 as i64);

    if reminders.is_empty() {
        void!(say(msg.channel_id, "No reminders for this user"));
        return Ok(());
    }

    let lines = reminders
        .into_iter()
        .zip(1..)
        .map(|((w, t), i)| format!("{:>3} | {} | {}", i, w, t))
        .join("\n");

    let message = MessageBuilder::new()
        .push("Reminders for ")
        .mention(&msg.author)
        .push_line(": ")
        .push_codeblock_safe(lines, None);

    void!(say(msg.channel_id, message));
});


command!(delete_reminder_cmd(ctx, msg, args) {
    let index = get_arg!(args, single, usize, index) as i64;

    if delete_reminder(&ctx, msg.author.id.0 as i64, index) {
        void!(say(msg.channel_id, "Deleted that reminder."));
    } else {
        void!(say(msg.channel_id, "That reminder didn't exist."));
    };

});


pub fn setup_reminders(_client: &mut Client, frame: StandardFramework) -> StandardFramework {
    frame.group("Reminders",
                |g| g
                .command("remind", |c| c
                         .cmd(remind_cmd)
                         .desc(r#"Create a reminder to remind you of something at a point in time.
You can specify deltas, days of the week or months and days.
For example: "Tomorrow", "3 hours", "july 4th".
Valid formats are: ```md
Time Difference
===============
- (num) y | years
- (num) M | months
- (num) w | weeks
- (num) d | days
- (num) h | hours
- (num) m | minutes
- (num) s | seconds

Specific time
=============
- Day of Week (friday)
- Month + day (july 4th)
- Tomorrow
```"#)
                         .example("\"3 hours\" Something")
                         .usage("{when} {message}"))
                .command("reminder_list", |c| c
                         .cmd(remind_list)
                         .desc("List your reminders.")
                         .batch_known_as(&["reminders_list", "list_reminders"])
                )
                .command("reminder_delete", |c| c
                         .cmd(delete_reminder_cmd)
                         .desc("Delete a reminder by index")
                         .batch_known_as(&["reminders_delete", "delete_reminder"])
                )
    )
}


#[cfg(test)]
mod tests {
    use super::*;

    lazy_static! {
        static ref BASE_TIME: NaiveDateTime = NaiveDateTime::from_timestamp(0, 0);
    }

    #[test]
    fn test_date_parser_delta() {
        let parsed_result = recognise_date(*BASE_TIME, "in 3min do something");

        assert!(parsed_result.is_ok());

        let parsed_result = parsed_result.unwrap();

        assert_eq!(parsed_result, (
            NaiveDateTime::from_timestamp(60 * 3, 0),
            "do something".to_owned()
        ));
    }

    #[test]
    fn test_date_parser_tomorrow() {
        let parsed_result = recognise_date(*BASE_TIME, "tomorrow do something");

        assert!(parsed_result.is_ok());

        let parsed_result = parsed_result.unwrap();

        assert_eq!(parsed_result, (
            NaiveDateTime::from_timestamp(60 * 60 * 24, 0),
            "do something".to_owned()
        ));
    }

    #[test]
    fn test_date_parser_day() {
        // Epoch is thursday, friday is +1 day.
        let parsed_result = recognise_date(*BASE_TIME, "on friday do something");

        assert!(parsed_result.is_ok());

        let parsed_result = parsed_result.unwrap();

        assert_eq!(parsed_result, (
            NaiveDateTime::from_timestamp(60 * 60 * 24, 0),
            "do something".to_owned()
        ));
    }


    #[test]
    fn test_date_parser_date() {
        let parsed_result = recognise_date(*BASE_TIME, "on july 4th do something");

        assert!(parsed_result.is_ok());

        let (parsed_date, remaining) = parsed_result.unwrap();

        assert_eq!(&remaining, "do something");

        assert_eq!(parsed_date.month(), 7);

        assert_eq!(parsed_date.day(), 4);
    }
}
