table! {
    blocked_guilds_channels (id) {
        id -> Int4,
        guild_id -> Nullable<Int8>,
        channel_id -> Nullable<Int8>,
    }
}

table! {
    command_alias (id) {
        id -> Int8,
        owner_id -> Int8,
        alias_name -> Varchar,
        alias_value -> Varchar,
    }
}

table! {
    guild (id) {
        id -> Int8,
        markov_on -> Bool,
        tag_prefix_on -> Bool,
        commands_from -> Int8,
    }
}

table! {
    message (id) {
        id -> Int8,
        guild_id -> Int8,
        user_id -> Int8,
        msg -> Varchar,
        created_at -> Timestamp,
    }
}

table! {
    prefix (id) {
        id -> Int8,
        guild_id -> Int8,
        pre -> Varchar,
    }
}

table! {
    reminder (id) {
        id -> Int8,
        user_id -> Int8,
        channel_id -> Int8,
        text -> Varchar,
        started -> Timestamp,
        when -> Timestamp,
    }
}

table! {
    tag (id) {
        id -> Int8,
        author_id -> Int8,
        guild_id -> Int8,
        key -> Varchar,
        text -> Varchar,
    }
}

table! {
    tea_count (user_id) {
        user_id -> Int8,
        count -> Int4,
    }
}

joinable!(message -> guild (guild_id));
joinable!(prefix -> guild (guild_id));
joinable!(tag -> guild (guild_id));

allow_tables_to_appear_in_same_query!(
    blocked_guilds_channels,
    command_alias,
    guild,
    message,
    prefix,
    reminder,
    tag,
    tea_count,
);
