-- Your SQL goes here

CREATE TABLE IF NOT EXISTS command_alias (
       id BIGSERIAL PRIMARY KEY,
       owner_id BIGINT NOT NULL,
       alias_name VARCHAR(2000) NOT NULL,
       alias_value VARCHAR(2000) NOT NULL,
       UNIQUE (owner_id, alias_name)
);

CREATE INDEX IF NOT EXISTS "command_alias_owner_id_idx" ON "command_alias" ("owner_id");
