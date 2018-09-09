-- Your SQL goes here

CREATE TABLE IF NOT EXISTS "tea_count" (
       user_id BIGINT PRIMARY KEY NOT NULL,
       count INTEGER DEFAULT 0 NOT NULL,
       constraint count_nonnegative check (count >= 0)
);
