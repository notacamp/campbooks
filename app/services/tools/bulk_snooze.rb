module Tools
  class BulkSnooze
    def self.call(args = {})
      snoozed_until = Tools::Snooze.parse_snoozed_until(args["snoozed_until"])
      return { snoozed_count: 0, error: "Invalid snooze time" } unless snoozed_until

      scope = EmailMessage.accessible_to(Current.user).where.not(status: :ignored)
      scope = scope.where(id: args["email_ids"]) if args["email_ids"].present?

      count = 0
      scope.includes(:email_account).find_each.group_by(&:email_account).each do |_account, messages|
        messages.group_by(&:email_thread).each do |thread, msgs|
          next unless thread
          result = Tools::Snooze.call(msgs.first, { "snoozed_until" => snoozed_until.iso8601 })
          count += 1 if result
        end
      end

      { snoozed_count: count }
    end
  end
end
