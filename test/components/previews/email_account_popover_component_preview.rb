# frozen_string_literal: true

class EmailAccountPopoverComponentPreview < ViewComponent::Preview
  # @label Connected · recently synced
  def connected
    render Campbooks::EmailAccountPopover.new(account: synced_account, message_count: 1284)
  end

  # @label Syncing now
  def syncing
    render Campbooks::EmailAccountPopover.new(account: scanning_account, message_count: 312)
  end

  # @label Never synced yet
  def never_synced
    render Campbooks::EmailAccountPopover.new(account: fresh_account, message_count: 0)
  end

  # @label Disconnected
  def disconnected
    render Campbooks::EmailAccountPopover.new(account: disconnected_account, message_count: 904)
  end

  private

  def synced_account
    EmailAccount.new(id: 1, name: "Support", email_address: "support@acme.com", color: "#0584da",
                     provider: :zoho, active: true, last_scanned_at: 12.minutes.ago)
  end

  def scanning_account
    EmailAccount.new(id: 2, name: "Sales", email_address: "sales@acme.com", color: "#2ea55c",
                     provider: :google, active: true, scanning: true, scan_started_at: 20.seconds.ago)
  end

  def fresh_account
    EmailAccount.new(id: 3, email_address: "newinbox@acme.com", color: "#e76e08",
                     provider: :microsoft, active: true)
  end

  def disconnected_account
    EmailAccount.new(id: 4, name: "Old Mailbox", email_address: "old@acme.com", color: "#de3b3d",
                     provider: :zoho, active: false, last_scanned_at: 3.days.ago)
  end
end
