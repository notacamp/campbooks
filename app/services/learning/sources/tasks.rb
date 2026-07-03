module Learning
  module Sources
    # Task accept/dismiss verdicts, read off the tasks table. Only AI-suggested tasks
    # the user has already triaged carry a signal: "accepted" (moved onto the board —
    # todo/in_progress/blocked/done) vs "dismissed" (cancelled). Still-suggested tasks
    # are pending and don't count. Keyed per sender only — tasks have no stable type
    # axis the way reminders do (priority is AI-inferred, not a sender property).
    # Workspace-scoped; bulk-preloaded like Sources::Reminders.
    class Tasks < Base
      ACCEPTED = %w[todo in_progress blocked done].freeze

      def initialize(workspace)
        @workspace = workspace
      end

      def signal_cascade = %i[contact domain]

      def tally_for(signal, contact_id: nil, sender_domain: nil, **)
        key =
          case signal
          when :contact then contact_id.presence
          when :domain then sender_domain&.downcase&.presence
          end
        return nil if key.blank?

        tallies.dig(signal, key)
      end

      private

      def tallies
        @tallies ||= begin
          status_names = Task.statuses.invert
          acc = { contact: {}, domain: {} }

          verdicts.each do |contact_id, from_address, status_raw|
            status = status_raw.is_a?(Integer) ? status_names[status_raw] : status_raw.to_s.presence
            label = label_for(status)
            next if label.nil?

            bump(acc[:contact], contact_id, label) if contact_id.present?
            domain = from_address.present? ? Emails::SenderDomain.for(from_address)&.downcase&.presence : nil
            bump(acc[:domain], domain, label) if domain
          end
          acc
        end
      end

      def label_for(status)
        return "dismissed" if status == "cancelled"

        "accepted" if ACCEPTED.include?(status)
      end

      def verdicts
        Task
          .where(workspace: @workspace, ai_suggested: true)
          .where.not(status: :suggested)
          .joins("LEFT JOIN email_messages em ON em.id = tasks.source_id AND tasks.source_type = 'EmailMessage'")
          .pluck(Arel.sql("em.contact_id"), Arel.sql("em.from_address"), Arel.sql("tasks.status"))
      end

      def bump(bucket, key, label)
        (bucket[key] ||= Hash.new(0))[label] += 1
      end
    end
  end
end
