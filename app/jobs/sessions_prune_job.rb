# Deletes sessions idle past Session::INACTIVITY_LIMIT so we don't retain their
# ip_address / user_agent rows indefinitely (GDPR storage limitation). Expired
# sessions are also rejected on resume; this sweeps the rows that never come back.
class SessionsPruneJob < ApplicationJob
  queue_as :default

  def perform
    Session.expired.in_batches.delete_all
  end
end
