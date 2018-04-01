CREATE TABLE guild (
       id BIGINT PRIMARY KEY,
       markov_on BOOLEAN NOT NULL DEFAULT false,
       tag_prefix_on BOOLEAN NOT NULL DEFAULT false,
       commands_from BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE message (
       id BIGINT PRIMARY KEY,
       guild_id BIGINT NOT NULL REFERENCES guild (id),
       user_id BIGINT NOT NULL,
       msg VARCHAR(2000) NOT NULL,
       created_at TIMESTAMP NOT NULL
);

CREATE TABLE "prefix" (
       id BIGSERIAL PRIMARY KEY,
       guild_id BIGINT NOT NULL REFERENCES guild (id),
       pre VARCHAR(2000) NOT NULL,
       UNIQUE (guild_id, pre)
);


CREATE TABLE reminder (
       id BIGSERIAL PRIMARY KEY,
       user_id BIGINT NOT NULL,
       channel_id BIGINT NOT NULL,
       text VARCHAR(2000) NOT NULL,
       started TIMESTAMP NOT NULL,
       "when" TIMESTAMP NOT NULL
);

CREATE TABLE tag (
       id BIGSERIAL PRIMARY KEY,
       author_id BIGINT NOT NULL,
       guild_id BIGINT NOT NULL REFERENCES guild (id),
       "key" VARCHAR(2000) NOT NULL,
       text VARCHAR(2000) NOT NULL
);

CREATE INDEX ON "message" ("guild_id", "user_id");
CREATE INDEX ON "prefix" ("guild_id");
CREATE INDEX ON "reminder" ("when" ASC);
CREATE INDEX ON "tag" ("key", "guild_id");

