-- Your SQL goes here

ALTER TABLE "guild"
      ADD CONSTRAINT "guild_commands_from_nonnegative" CHECK (commands_from >= 0);
