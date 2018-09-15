-- Your SQL goes here

CREATE TABLE IF NOT EXISTS "blocked_guilds_channels" (
       id SERIAL PRIMARY KEY,
       guild_id BIGINT,
       channel_id BIGINT
);
