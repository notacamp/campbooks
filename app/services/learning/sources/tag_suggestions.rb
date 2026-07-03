module Learning
  module Sources
    # Feed tag-suggestion accept/reject verdicts, read from the shared
    # learning_decisions table (domain "tag_suggestion"). These suggestions are
    # ephemeral (no durable record), so the decision row IS the memory. Keyed per
    # (sender, tag) — "I keep rejecting this tag from this sender" — with the tag name
    # pulled from the decision's signals jsonb. User-scoped, matching the per-user
    # home feed; recent only (WINDOW), since filing habits drift. Bulk-preloaded.
    class TagSuggestions < Base
      DOMAIN = "tag_suggestion"
      WINDOW = 90.days

      def initialize(user, now: Time.current)
        @user = user
        @now = now
      end

      def signal_cascade = %i[contact domain]

      def tally_for(signal, contact_id: nil, sender_domain: nil, tag_name: nil, **)
        tag = tag_name.to_s.downcase.presence
        return nil if tag.blank?

        key =
          case signal
          when :contact then contact_id.presence && "#{contact_id}:#{tag}"
          when :domain then (d = sender_domain&.downcase&.presence) && "#{d}:#{tag}"
          end
        return nil if key.blank?

        tallies.dig(signal, key)
      end

      private

      def tallies
        @tallies ||= begin
          acc = { contact: {}, domain: {} }

          rows.each do |contact_id, sender_domain, label, signals|
            tag = signals.is_a?(Hash) ? signals["tag_name"].to_s.downcase.presence : nil
            next if tag.blank? || label.blank?

            bump(acc[:contact], "#{contact_id}:#{tag}", label) if contact_id.present?
            domain = sender_domain&.downcase&.presence
            bump(acc[:domain], "#{domain}:#{tag}", label) if domain
          end
          acc
        end
      end

      def rows
        LearningDecision
          .where(domain: DOMAIN, user_id: @user.id)
          .where(created_at: (@now - WINDOW)..)
          .pluck(:contact_id, :sender_domain, :label, :signals)
      end

      def bump(bucket, key, label)
        (bucket[key] ||= Hash.new(0))[label] += 1
      end
    end
  end
end
