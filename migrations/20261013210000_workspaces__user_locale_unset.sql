-- A user's language is unset (NULL) until they choose one, so the language
-- their client asks for (`Accept-Language`) applies meanwhile; see the
-- kernel's `i18n` module. No endpoint could set `locale` before this
-- migration, so every stored value is the old default, not a choice.
ALTER TABLE workspaces.users ALTER COLUMN locale DROP DEFAULT;
ALTER TABLE workspaces.users ALTER COLUMN locale DROP NOT NULL;
UPDATE workspaces.users SET locale = NULL;
