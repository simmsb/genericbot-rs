-- Your SQL goes here

CREATE TABLE command_alias (
       id BIGSERIAL PRIMARY KEY,
       owner_id BIGINT NOT NULL,
       alias_name VARCHAR(2000) NOT NULL,
       alias_value VARCHAR(2000) NOT NULL,
       UNIQUE (owner_id, alias_name)
);

CREATE INDEX ON "command_alias" ("owner_id");
