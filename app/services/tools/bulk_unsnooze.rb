module Tools
  # Reverse of Tools::BulkSnooze: clear the snooze on the given threads and move
  # them back to the inbox. Backs the bulk-snooze Undo snackbar.
  class BulkUnsnooze
    def self.call(args = {})
      scope = EmailMessage.accessible_to(Current.user)
      scope = scope.where(id: args["email_ids"]) if args["email_ids"].present?

      count = 0
      scope.includes(:email_account).find_each.group_by(&:email_account).each do |_account, messages|
        messages.group_by(&:email_thread).each do |thread, msgs|
          next unless thread

          count += 1 if Tools::Unsnooze.call(msgs.first)
        end
      end

      { unsnoozed_count: count }
    end
  end
end
