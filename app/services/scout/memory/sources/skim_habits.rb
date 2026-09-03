# frozen_string_literal: true

require "base64"

module Scout
  module Memory
    module Sources
      # Learned skim habits: "You usually archive mail from **@newsletter.com**
      # (9 of your last 12)." Derived from the email_skim LearningDecision rows the
      # skim deck already records, using the same consensus thresholds as
      # Emails::SkimActionMemory (>= 3 examples, >= 60% share) across the
      # contact / sender-domain / category tiers.
      #
      # Confirm records another supporting decision (reinforces the habit); remove
      # deletes the decisions for that key (forgets the habit — the mail stays put).
      class SkimHabits < Base
        DOMAIN = "email_skim"
        WINDOW = 90.days
        MIN_EXAMPLES = 3
        MIN_SHARE = 0.6

        def entries
          consensus_rows.filter_map { |row| entry_for(row) }
        end

        def confirm(entry)
          row = decode(entry.id)
          return false unless row

          Learning::Recorder.record(
            domain: DOMAIN, user: user, workspace_id: workspace.id, label: row[:label],
            signals: { confirmed: true }, **tier_key_args(row)
          )
          true
        end

        def remove(entry)
          row = decode(entry.id)
          return false unless row

          scope = LearningDecision.where(domain: DOMAIN, user_id: user.id)
          case row[:tier]
          when :contact  then scope.where(contact_id: row[:key]).delete_all
          when :domain   then scope.where("LOWER(sender_domain) = ?", row[:key].to_s.downcase).delete_all
          when :category then scope.where(category: row[:key]).delete_all
          end
          true
        end

        private

        def consensus_rows
          decisions = LearningDecision
            .where(domain: DOMAIN, user_id: user.id)
            .where(created_at: WINDOW.ago..)
            .to_a

          tier_consensus(decisions, :contact, :contact_id) +
            tier_consensus(decisions, :domain, :sender_domain) +
            tier_consensus(decisions, :category, :category)
        end

        def tier_consensus(decisions, tier, column)
          grouped = Hash.new { |hash, key| hash[key] = Hash.new(0) }
          decisions.each do |decision|
            key = decision.public_send(column)
            key = key.to_s.downcase.presence if column == :sender_domain
            next if key.blank?

            grouped[key][decision.label] += 1
          end

          grouped.filter_map do |key, labels|
            total = labels.values.sum
            next if total < MIN_EXAMPLES

            label, count = labels.max_by { |_, value| value }
            next if count.to_f / total < MIN_SHARE

            { tier: tier, key: key, label: label, count: count, total: total }
          end
        end

        def entry_for(row)
          display = display_key(row)
          return nil if display.blank?

          variant = row[:tier] == :category ? "category" : "sender"
          build(
            id: "skim:#{row[:tier]}:#{row[:label]}:#{Scout::Memory::Entry.token(row[:key])}",
            facet: :stack,
            sentence: sentence("scout_memory.sources.skim_habits.#{row[:label]}.#{variant}",
              key: display, count: row[:count], total: row[:total]),
            origin: :learned,
            origin_detail: I18n.t("scout_memory.origins.learned_skim"),
            record: nil,
            actions: %i[confirm remove]
          )
        end

        def display_key(row)
          case row[:tier]
          when :contact
            Contact.where(id: row[:key]).pick(:name).presence ||
              Contact.where(id: row[:key]).first&.display_name
          when :domain
            row[:key].start_with?("@") ? row[:key] : "@#{row[:key]}"
          when :category
            row[:key].to_s.humanize
          end
        end

        # id shape: "skim:<tier>:<label>:<base64url(key)>"
        def decode(id)
          _, tier, label, token = id.to_s.split(":", 4)
          return nil unless tier && label && token

          { tier: tier.to_sym, label: label, key: Base64.urlsafe_decode64(token) }
        rescue ArgumentError
          nil
        end

        def tier_key_args(row)
          case row[:tier].to_sym
          when :contact  then { contact_id: row[:key] }
          when :domain   then { sender_domain: row[:key] }
          when :category then { category: row[:key] }
          else {}
          end
        end
      end
    end
  end
end
