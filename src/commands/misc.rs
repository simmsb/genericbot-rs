use serenity::{
    prelude::*,
    framework::standard::{
        StandardFramework,
    },
    utils::{
        with_cache,
    },
};
use chrono::Utc;
use procinfo;
use std::time;


fn process_usage() -> f64 {
    use std::thread;
    let start_measure = procinfo::pid::stat_self().unwrap().utime;
    thread::sleep(time::Duration::from_millis(100));
    let end_measure = procinfo::pid::stat_self().unwrap().utime;

    let diff = end_measure - start_measure;
    return diff as f64 / 0.1; // util seconds / 100ms per second
}


command!(status_cmd(ctx, msg) {
    use ::{StartTime, CmdCounter};

    let mem_usage = procinfo::pid::statm_self().ok().map_or(0, |p| p.resident);
    let cpu_usage = process_usage();
    let uptime = {
        let &start = ctx.data.lock().get::<StartTime>().unwrap();
        let now = Utc::now().naive_utc();

        now.signed_duration_since(start)
    };

    let cmd_count = {
        let lock = ctx.data.lock();
        let count = *lock.get::<CmdCounter>().unwrap().read().unwrap();
        count
    };

    let (g_c, c_c, u_c, s_c) = with_cache(
        |c| {
            let g_c = c.guilds.len();
            let c_c = c.channels.len();
            let u_c = c.users.len();
            let s_c = c.shard_count;
            (g_c, c_c, u_c, s_c)
        });

    let (u_days, u_hours, u_min, u_sec) = (
        uptime.num_days(),
        uptime.num_hours() % 24,
        uptime.num_minutes() % 60,
        uptime.num_seconds() % 60,
    );

    let uptime_str = format!("{}d, {}h, {}m, {}s", u_days, u_hours, u_min, u_sec);

    msg.channel_id.send_message(
        |m| m.embed(
            |e| e
                .title("genericbot stats")
                .colour(0x2C78C8)
                .field("Uptime", uptime_str, true)
                .field("Guild count", g_c, true)
                .field("Channel count", c_c, true)
                .field("User count", u_c, true)
                .field("Commands executed", cmd_count, true)
                .field("Shard count", s_c, true)
                .field("Cpu usage", format!("{:.1}%", cpu_usage), true)
                .field("Mem usage", format!("{:.2}MB", mem_usage), true)
        ))?;
});


pub fn setup_misc(_client: &mut Client, frame: StandardFramework) -> StandardFramework {
    frame.group("Misc",
                |g| g
                .command("stats", |c| c
                         .cmd(status_cmd)
                         .desc("Bot stats")
                         .batch_known_as(&["status"])
                ))
}
