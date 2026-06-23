class AddPasswordSetByUserToUsers < ActiveRecord::Migration[8.1]
  # Distinguishes a real, user-chosen password from the synthetic random one that
  # OAuth-created users get (SecureRandom.hex(32)). Without it, password_digest is
  # always present and Auth::OauthSignIn can't tell a password account (which must
  # NEVER be enterable via OAuth without an explicit link) from an OAuth-only one.
  def up
    add_column :users, :password_set_by_user, :boolean, default: false, null: false

    # Data backfill runs only on a real migrate (dev/prod). The test DB loads from
    # schema.rb and never executes this, so it can't interfere with factories.
    backfill_zoho_identities
    backfill_owned_mailbox_identities
    mark_real_password_users
  end

  def down
    remove_column :users, :password_set_by_user
  end

  private

  # Legacy Zoho sign-ins stored the provider id on users.zoho_uid; promote it.
  def backfill_zoho_identities
    execute(<<~SQL.squish)
      INSERT INTO identities (user_id, provider, uid, email, created_at, updated_at)
      SELECT u.id, 'zoho', u.zoho_uid, u.email_address, NOW(), NOW()
      FROM users u
      WHERE u.zoho_uid IS NOT NULL AND u.zoho_uid <> ''
        AND NOT EXISTS (
          SELECT 1 FROM identities i WHERE i.provider = 'zoho' AND i.uid = u.zoho_uid
        )
    SQL
  end

  # Google/Microsoft sign-ins never stored a uid on the user, but if the user owns
  # a connected mailbox at their own login address we can recover it from
  # email_accounts.provider_account_id. provider is an integer enum on
  # email_accounts (zoho:0, google:1, microsoft:2) — map it to the string form.
  def backfill_owned_mailbox_identities
    execute(<<~SQL.squish)
      INSERT INTO identities (user_id, provider, uid, email, created_at, updated_at)
      SELECT eau.user_id,
             CASE ea.provider WHEN 0 THEN 'zoho' WHEN 1 THEN 'google' WHEN 2 THEN 'microsoft' END,
             ea.provider_account_id, ea.email_address, NOW(), NOW()
      FROM email_accounts ea
      JOIN email_account_users eau
        ON eau.email_account_id = ea.id AND eau.owner = TRUE
      JOIN users u
        ON u.id = eau.user_id AND LOWER(u.email_address) = LOWER(ea.email_address)
      WHERE ea.provider_account_id IS NOT NULL AND ea.provider_account_id <> ''
        AND NOT EXISTS (
          SELECT 1 FROM identities i
          WHERE i.provider = CASE ea.provider WHEN 0 THEN 'zoho' WHEN 1 THEN 'google' WHEN 2 THEN 'microsoft' END
            AND i.uid = ea.provider_account_id
        )
    SQL
  end

  # Anyone we could NOT identify as OAuth-origin (no identity after backfill, no
  # legacy zoho_uid) is assumed to hold a genuine password. Marking them protects
  # those accounts from OAuth takeover via the email-match path. OAuth-origin users
  # stay false so the migration-window self-heal can still adopt them if needed.
  def mark_real_password_users
    execute(<<~SQL.squish)
      UPDATE users SET password_set_by_user = TRUE
      WHERE id NOT IN (SELECT DISTINCT user_id FROM identities)
        AND (zoho_uid IS NULL OR zoho_uid = '')
    SQL
  end
end
