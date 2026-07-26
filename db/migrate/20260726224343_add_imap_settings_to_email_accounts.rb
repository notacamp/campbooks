class AddImapSettingsToEmailAccounts < ActiveRecord::Migration[8.1]
  def change
    # IMAP accounts authenticate with a password, not an OAuth grant.
    change_column_null :email_accounts, :refresh_token, true

    add_column :email_accounts, :imap_host, :string
    add_column :email_accounts, :imap_port, :integer
    add_column :email_accounts, :imap_security, :string, comment: "ssl or starttls"
    add_column :email_accounts, :imap_username, :string, comment: "IMAP/SMTP login when it differs from email_address; nil = use email_address"
    add_column :email_accounts, :imap_password, :text, comment: "ActiveRecord-encrypted app password, shared by IMAP and SMTP"
    add_column :email_accounts, :smtp_host, :string
    add_column :email_accounts, :smtp_port, :integer
    add_column :email_accounts, :smtp_security, :string, comment: "ssl or starttls"
    add_column :email_accounts, :backfill_since, :datetime, comment: "Only ingest mail received after this time (connect-time sync-history choice); nil = full history"
  end
end
