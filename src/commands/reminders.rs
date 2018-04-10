use serenity::{
    prelude::*,
    model::{
        channel::Message,
        permissions::Permissions,
    },
    framework::standard::{
        StandardFramework,
        CommandError,
    },
    utils::{
        with_cache,
        MessageBuilder,
    },
};
use serenity;
use diesel::prelude::*;
use diesel;
use ::PgConnectionManager;
use models::Reminder;
use regex::Regex;
use chrono::{NaiveDateTime, Utc, Datelike, Duration, NaiveDate};


// TODO: test this function properly.
fn recognise_date(mut base_time: NaiveDateTime, date: &str) -> Result<NaiveDateTime, CommandError> {
    // parse out jan(uary) ... stuff etc
    lazy_static! {
        static ref TDIFF_RE: Regex = Regex::new(concat!(
            r"(?P<value>\d+)\s*",
            r"(?P<period>",
            r"y(?:ears?)?|",
            r"M|",
            r"months?|",
            r"w(?:eeks?)?|",
            r"d(?:ays?)?|",
            r"h(?:ours?)?|",
            r"m(?:inutes?)?|",
            r"s(?:econds?)?)"
        )).unwrap();

        static ref TDAY_RE: Regex = Regex::new(r"(monday|tuesday|wednesday|thursday|friday|saturday|sunday)").unwrap();

        static ref DMONTH_RE: Regex = Regex::new(concat!(
            r"(?P<month>",
            r"jan(:?uary)?|",
            r"feb(:?ruary)?|",
            r"mar(:?ch)?|",
            r"apr(:?il)?|",
            r"may|",
            r"june?|",
            r"july?|",
            r"aug(:?ust)?|",
            r"sep(:?tember)?|",
            r"oct(:?ober)?|",
            r"nov(:?ember)?|",
            r"dec(:?ember)?)",
            r"\s*(?P<value>\d+)"
        )).unwrap();
    }

    let mut has_parsed = false;

    if date.contains("tomorrow") {
        base_time += Duration::days(1);
        has_parsed = true;
    }

    let mut has_parsed_diff = false;

    for caps in TDIFF_RE.captures_iter(date) {
        if has_parsed {
            return Err(CommandError::from("Cannot mix 'tomorrow' and delta times."));
        }

        let val = (&caps["value"]).parse::<u32>()? as i64;
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
        has_parsed_diff = true;
    }

    has_parsed |= has_parsed_diff;

    if let Some(caps) = TDAY_RE.captures(date) {
        if has_parsed {
            return Err(CommandError::from("Cannot mix weekday and delta time."));
        }

        let day = match &(&caps[0])[..2] {
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

        base_time += Duration::days(delta as i64);

        has_parsed = true;
    }

    for caps in DMONTH_RE.captures_iter(date) {
        if has_parsed {
            return Err(CommandError::from("Cannot mix deltas or have multiple dates and month values."));
        }

        let month = &caps["month"];
        let day = (&caps["value"]).parse::<u32>()?;

        let month_num = match &month[..3] {
            "jan" => 0,
            "feb" => 1,
            "mar" => 2,
            "apr" => 3,
            "may" => 4,
            "jun" => 5,
            "jul" => 6,
            "aug" => 7,
            "sep" => 8,
            "oct" => 9,
            "nov" => 10,
            "dec" => 11,
            _     => unreachable!(),
        };

        let current_month_num = base_time.month0();

        let updated_value = if current_month_num <= month_num {
            NaiveDate::from_yo(base_time.year(), 1)
        } else {
            NaiveDate::from_yo(base_time.year() + 1, 1)
        };

        base_time = updated_value.and_hms(0, 0, 0)
            .with_month0(month_num).ok_or("Bad month provided.")?
            .with_day(day).ok_or("Bad day number provided for that month.")?;

        has_parsed = true;
    }

    if !has_parsed {
        return Err(CommandError::from("Could not parse time."));
    }

    Ok(base_time)
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


pub fn human_timedelta(delta: &Duration) -> String {
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

    let nonzero_count = formats.iter().filter(|&(x, _)| *x != 0).count() as isize;

    let mut res = String::new();
    for (n, &(t, s)) in formats.into_iter()
                               .filter(|&(x, _)| *x != 0)
                               .enumerate()
    {
        res.push_str(&format!("{} {}", t, s));

        if t != 1 {
            res.push_str("s");
        }

        // nonzero_count will be the largest value n will reach
        // if n == nonzero - 2, add in 'and'
        // else if n < nonzero - 2, add in a comma
        if n as isize == nonzero_count - 2 {
            res.push_str(" and ")
        } else if (n as isize) < nonzero_count - 2 {
            res.push_str(", ")
        }
    }

    return res;
}


command!(remind_cmd(ctx, msg, args) {
    let time = get_arg!(args, single_quoted, String, time);
    let remind_msg = args.full();

    let now = Utc::now().naive_utc();
    let when = recognise_date(now, &time)?;

    insert_reminder(&ctx, msg.author.id.0 as i64,
                    msg.channel_id.0 as i64,
                    when, now, remind_msg);

    let delta = when.signed_duration_since(now);

    msg.channel_id.say(format!("Okay, I'll remind you about {} in {}", remind_msg, human_timedelta(&delta)))?;
});


pub fn setup_reminders(_client: &mut Client, frame: StandardFramework) -> StandardFramework {
    frame.group("Reminders",
                |g| g
                .command("remind", |c| c
                         .cmd(remind_cmd)
                         .desc("Create a reminder to remind you of something at a point in time.")
                         .example("\"3 hours\" Something")
                         .usage("{when} {message}")))
}
