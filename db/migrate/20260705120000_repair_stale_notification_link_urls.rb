# frozen_string_literal: true

# Repair notification deep links left stale by the 2026-06-29 bigint->uuid primary
# key migration (MigratePrimaryKeysToUuid).
#
# `notifications.link_url` is a free-text column that bakes a record id into a URL
# at creation time (e.g. "/documents/189"). The UUID migration rewrote id *columns*
# -- including the polymorphic `notifiable_id` -- but could not reach inside this
# string, so every notification created before it still points at a now-nonexistent
# integer id and 404s when clicked (show 302 -> "/documents/189" -> 404).
#
# Two-tier, idempotent repair (safe to re-run; a no-op once every link holds a uuid,
# since a uuid never matches the integer-only `[0-9]+` pattern):
#
#   1. REBUILD the links we can prove point at the notification's OWN notifiable
#      (whose id was correctly migrated), by reading the new uuid from notifiable_id:
#        Document    "/documents/<int>"     -> "/documents/<uuid>"      (Notifier#document_failed)
#        AgentThread "/scout/threads/<int>" -> "/scout/threads/<uuid>"  (Notifier#scout_reply)
#
#   2. NEUTRALISE the rest -- links with no notifiable (e.g. the "new document
#      uploaded" activity notice, "email tagged"), or whose stored notifiable is a
#      DIFFERENT record than the URL targets (thread mention/activity store an
#      AgentThread notifiable but deep-link to an EmailThread) -- to their collection
#      index, so a click lands somewhere useful instead of dead-ending on a 404.
#      These integer ids are unrecoverable: the bigint->uuid map was dropped.
class RepairStaleNotificationLinkUrls < ActiveRecord::Migration[8.1]
  def up
    return unless connection.table_exists?(:notifications)

    # 1. Rebuild provably-correct deep links from the migrated polymorphic id.
    execute(<<~SQL.squish)
      UPDATE notifications SET link_url = '/documents/' || notifiable_id::text
      WHERE notifiable_type = 'Document' AND notifiable_id IS NOT NULL
        AND link_url ~ '^/documents/[0-9]+$'
    SQL
    execute(<<~SQL.squish)
      UPDATE notifications SET link_url = '/scout/threads/' || notifiable_id::text
      WHERE notifiable_type = 'AgentThread' AND notifiable_id IS NOT NULL
        AND link_url ~ '^/scout/threads/[0-9]+$'
    SQL

    # 2. Neutralise the unrecoverable stale integer links to a safe index page.
    #    Runs after the rebuilds, so links repointed above (now uuids) are skipped.
    execute("UPDATE notifications SET link_url = '/documents'      WHERE link_url ~ '^/documents/[0-9]+$'")
    execute("UPDATE notifications SET link_url = '/email_messages' WHERE link_url ~ '^/email_messages/[0-9]+$'")
    execute("UPDATE notifications SET link_url = '/email_messages' WHERE link_url ~ '^/email_threads/[0-9]+$'")
    execute("UPDATE notifications SET link_url = '/scout'          WHERE link_url ~ '^/scout/threads/[0-9]+$'")
  end

  def down
    # The original integer ids were destroyed by the uuid migration, so the prior
    # stale link_urls cannot be reconstructed. Nothing to undo.
    raise ActiveRecord::IrreversibleMigration
  end
end
