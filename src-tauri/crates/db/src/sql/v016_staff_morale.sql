-- Staff morale and renewal session state (player-parity contract UX)
ALTER TABLE staff ADD COLUMN morale INTEGER NOT NULL DEFAULT 100;
ALTER TABLE staff ADD COLUMN morale_core TEXT NOT NULL DEFAULT '{}';
