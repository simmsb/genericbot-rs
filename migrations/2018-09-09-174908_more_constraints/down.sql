-- This file should undo anything in `up.sql`

ALTER TABLE "guild"
      DROP CONSTRAINT "guild_commands_from_nonnegative";
