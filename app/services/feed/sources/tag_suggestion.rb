module Feed
  module Sources
    # Filing suggestions: Scout proposes a tag and auto-applies it, then shows a
    # notice card ("Filed … under #tag — Undo"). Excludes anything with a
    # draft_reply suggestion so reply-worthy mail stays an action/reminder.
    class TagSuggestion < Feed::Source
      def self.key = "tag_suggestion"

      def candidates
        mem = tag_memory
        workspace_cache = {}

        collapse_by_thread(
          base_scope.reorder(:id).find_each.filter_map do |m|
            tag_name = suggested_tag_name(m)
            next if tag_name.blank?

            # Learn from past filing: drop a tag this user keeps rejecting from this
            # sender, and float an always-accepted one to the top of the row.
            verdict = learned_verdict(mem, m, tag_name)
            next if verdict == "rejected"

            # Auto-apply: apply the tag now so the card becomes an informative
            # notice ("Filed") rather than a question ("File it?"). Idempotent —
            # re-running the generator won't fire events a second time.
            ws = workspace_cache[m.email_account_id] ||= m.email_account&.workspace
            applied = auto_apply_tag(m, tag_name, ws)

            {
              subject: m,
              dedupe_key: "tag_suggestion:#{m.id}",
              sort_at: m.received_at || m.created_at,
              # Low-stakes filing chores: below actionable mail, a learned
              # always-accepted tag floats above the rest.
              score: verdict == "accepted" ? 25 : 15,
              attention: false,
              data: { "tag_name" => tag_name, "applied" => applied }
            }
          end
        )
      end

      def still_valid?(item, m)
        return false if m.nil? || m.skimmed_at.present? || m.ai_todo_dismissed?
        return false unless in_inbox?(m)
        tag_name = item.data["tag_name"].to_s
        return false if tag_name.blank?

        if item.data["applied"]
          # Notice mode: valid while Scout still suggests the tag AND it is still
          # applied (guards against manual removal via another surface).
          suggested_tag_name(m) == tag_name && m.tags.any? { |t| t.name.to_s.casecmp?(tag_name) }
        else
          # Legacy ask-mode: valid while the tag is not yet applied.
          suggested_tag_name(m) == tag_name && m.tags.none? { |t| t.name.to_s.casecmp?(tag_name) }
        end
      end

      private

      # One memory per feed generation (bulk-preloaded), reused across every card.
      def tag_memory
        Learning::Memory.new(source: Learning::Sources::TagSuggestions.new(user))
      rescue => e
        Rails.logger.warn("[Feed::Sources::TagSuggestion] memory build failed: #{e.message}")
        nil
      end

      # The learned verdict ("accepted" / "rejected") for this sender + tag, or nil.
      # Best-effort: a lookup failure just means no learning influence.
      def learned_verdict(mem, m, tag_name)
        return nil unless mem

        mem.suggestion(
          contact_id: m.contact_id,
          sender_domain: Emails::SenderDomain.for(m.from_address),
          tag_name: tag_name
        )&.label
      rescue => e
        Rails.logger.warn("[Feed::Sources::TagSuggestion] learned_verdict failed: #{e.message}")
        nil
      end

      def base_scope
        in_inbox(admitted(
          EmailMessage.accessible_to(user)
            .where("ai_suggested_actions @> ?", add_tag_json)
            .where.not("ai_suggested_actions @> ?", draft_reply_json)
            .where(ai_todo_dismissed: false, skimmed_at: nil)
            .select(:id, :received_at, :created_at, :ai_suggested_actions, :category,
                    :email_account_id, :email_thread_id, :contact_id, :from_address, :subject)
        ))
      end

      def suggested_tag_name(m)
        action = Array(m.ai_suggested_actions).find { |a| a["tool"] == "add_tag" }
        action&.dig("args", "tag_name").to_s.presence
      end

      def add_tag_json = [ { tool: "add_tag" } ].to_json
      def draft_reply_json = [ { tool: "draft_reply" } ].to_json

      # Apply the tag immediately at generation time via the shared EmailActions tool
      # so the card becomes a notice rather than a question.
      # Idempotency: checks whether the tag is already applied before calling the
      # tool, so re-running the generator does not re-publish the "email.tagged"
      # event for an email that was filed on a previous cycle.
      # Returns true if the tag is (or just became) applied; false on any failure.
      def auto_apply_tag(m, tag_name, workspace)
        return false unless workspace

        tag = workspace.tags.find_by("LOWER(name) = ?", tag_name.downcase.strip)
        return false unless tag

        # Already applied — no need to fire the tool (and its event) again.
        return true if m.email_message_tags.where(tag_id: tag.id).exists?

        Tools::AddTag.call(m, { "tag_name" => tag_name }).present?
      rescue => e
        Rails.logger.warn("[Feed::Sources::TagSuggestion] auto_apply_tag failed (m=#{m.id}): #{e.class}: #{e.message}")
        false
      end
    end
  end
end
