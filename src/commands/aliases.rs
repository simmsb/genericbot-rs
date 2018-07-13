use serenity::{
    prelude::*,
    framework::standard::{
        StandardFramework,
        CommandError,
    },
};

use diesel;
use diesel::prelude::*;
use ::PgConnectionManager;
use itertools::Itertools;
use utils::say;


pub fn get_alias(ctx: &Context, name: &str, u_id: i64) -> Option<String> {
    use schema::command_alias::dsl::*;

    let pool = extract_pool!(&ctx);

    command_alias
        .filter(owner_id.eq(u_id))
        .filter(alias_name.eq(name))
        .select(alias_value)
        .first(pool).ok()
}


fn insert_alias(ctx: &Context, u_id: i64, name: &str, alias: &str) {
    use schema::command_alias::dsl::*;
    use models::NewCommandAlias;

    let new_alias =  NewCommandAlias {
        owner_id: u_id,
        alias_name: name,
        alias_value: alias,
    };

    let pool = extract_pool!(&ctx);

    diesel::insert_into(command_alias)
        .values(&new_alias)
        .on_conflict((owner_id, alias_name))
        .do_update()
        .set(alias_value.eq(alias))
        .execute(pool)
        .expect("Couldn't save alias");
}


fn delete_alias(ctx: &Context, u_id: i64, name: &str) {
    use schema::command_alias::dsl::*;

    let pool = extract_pool!(&ctx);

    diesel::delete(command_alias
                   .filter(alias_name.eq(name))
                   .filter(owner_id.eq(u_id)))
        .execute(pool)
        .unwrap();
}

fn alias_exists(ctx: &Context, u_id: i64, name: &str) -> bool {
    use schema::command_alias::dsl::*;

    let pool = extract_pool!(&ctx);

    diesel::select(diesel::dsl::exists(command_alias
        .filter(owner_id.eq(u_id))
        .filter(alias_name.eq(name))))
        .get_result(pool).expect("Failed to get alias existence")
}


// TODO: list aliases, just copy code from reminders

command!(add_alias_cmd(ctx, msg, args) {
    let alias_name = get_arg!(args, single, String, alias_name);
    let alias_value = args.iter::<String>().map(|s| s.unwrap()).join(" ");

    let u_id = msg.author.id.0 as i64;

    let exists_already = alias_exists(&ctx, u_id, &alias_name);

    insert_alias(&ctx, u_id, &alias_name, &alias_value);

    let response_msg = if exists_already { "Overwrote existing alias" } else { "Inserted new alias" };

    void!(say(msg.channel_id, response_msg));
});


command!(delete_alias_cmd(ctx, msg, args) {
    let alias_name = args.full();

    let u_id = msg.author.id.0 as i64;

    let exists = alias_exists(&ctx, u_id, &alias_name);

    if !exists {
        void!(say(msg.channel_id, "No alias with that name exists!"));
    } else {
        delete_alias(&ctx, u_id, &alias_name);
        void!(say(msg.channel_id, "Deleted that alias!"));
    }
});


pub fn setup_aliases(_client: &mut Client, frame: StandardFramework) -> StandardFramework {
    frame.group("Aliases",
                |g| g
                .command(
                    "add_alias", |c| c
                        .cmd(add_alias_cmd)
                        .desc("Create or overwrite an alias for a command, usable only by you.")
                        .example("\"something\" remind 3m tea is ready")
                        .usage("{alias name} {alias value}")
                )
                .command(
                    "delete_alias", |c| c
                        .cmd(delete_alias_cmd)
                        .desc("Deletes an alias for a command")
                        .example("\"something\"")
                        .usage("{alias name}")
                )
    )
}
