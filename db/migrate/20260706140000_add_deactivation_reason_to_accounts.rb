class AddDeactivationReasonToAccounts < ActiveRecord::Migration[8.1]
  # Why an account was auto-deactivated by the sync pipeline (e.g. the Google
  # identity has no Gmail / no Calendar). NULL for a plain token disconnect or a
  # user-initiated one, so we can tell "reconnect will fix it" apart from
  # "there is no service behind this login". Backfills to NULL — safe on deploy.
  def change
    add_column :email_accounts, :deactivation_reason, :string
    add_column :calendar_accounts, :deactivation_reason, :string
  end
end
