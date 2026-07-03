module Learning
  module Sources
    # Reminder confirm/dismiss verdicts, read straight off the reminders table
    # ("the records ARE the memory" — no separate decisions row). The signal is per
    # (sender, reminder_type): a sender's payment_due reminders may be consistently
    # confirmed while their event reminders are dismissed, so reminder_type is baked
    # into every key. Labels are "confirmed" / "dismissed".
    #
    # Workspace-scoped — reminders have no per-user owner, and every member's
    # confirm/dismiss is valid signal for the workspace (mirrors ClassificationMemory).
    #
    # Bulk: one query over the workspace's acted-on reminders, left-joined to the
    # source email for its contact/domain, tallied once and reused across the many
    # per-type lookups the extractor and builder make for a single email.
    class Reminders < Base
      def initialize(workspace)
        @workspace = workspace
      end

      def signal_cascade = %i[contact domain type]

      def tally_for(signal, contact_id: nil, sender_domain: nil, reminder_type: nil, **)
        type = reminder_type.to_s
        return nil if type.blank?

        key =
          case signal
          when :contact then contact_id.presence && "#{contact_id}:#{type}"
          when :domain then (d = sender_domain&.downcase.presence) && "#{d}:#{type}"
          when :type then type
          end
        return nil if key.blank?

        tallies.dig(signal, key)
      end

      private

      # Bucket every acted-on reminder into per-tier { key => { "confirmed"/"dismissed" => n } }.
      def tallies
        @tallies ||= begin
          type_names = Reminder.reminder_types.invert
          status_names = Reminder.statuses.invert
          acc = { contact: {}, domain: {}, type: {} }

          verdicts.each do |contact_id, from_address, type_raw, status_raw|
            # pluck may return the enum as its cast string or its raw integer; accept both.
            type = type_raw.is_a?(Integer) ? type_names[type_raw] : type_raw.to_s.presence
            label = status_raw.is_a?(Integer) ? status_names[status_raw] : status_raw.to_s.presence
            next if type.blank? || label.blank?

            bump(acc[:type], type, label)
            bump(acc[:contact], "#{contact_id}:#{type}", label) if contact_id.present?
            domain = from_address.present? ? Emails::SenderDomain.for(from_address)&.downcase&.presence : nil
            bump(acc[:domain], "#{domain}:#{type}", label) if domain
          end
          acc
        end
      end

      def verdicts
        Reminder
          .where(workspace: @workspace, status: %i[confirmed dismissed])
          .joins("LEFT JOIN email_messages em ON em.id = reminders.source_id AND reminders.source_type = 'EmailMessage'")
          .pluck(Arel.sql("em.contact_id"), Arel.sql("em.from_address"),
                 Arel.sql("reminders.reminder_type"), Arel.sql("reminders.status"))
      end

      def bump(bucket, key, label)
        (bucket[key] ||= Hash.new(0))[label] += 1
      end
    end
  end
end
