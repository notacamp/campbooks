class EmailAccountRemovalJob < ApplicationJob
  queue_as :default

  # Tears down a single email account off-request (see Accounts::Remover).
  # Idempotent: if the account is already gone (double-enqueue / retry after a
  # partial run), there's nothing to do.
  def perform(account_id)
    account = EmailAccount.find_by(id: account_id)
    return unless account

    Accounts::Remover.new(account).remove!
  end
end
