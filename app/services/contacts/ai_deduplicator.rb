module Contacts
  class AiDeduplicator
    MODEL = "claude-sonnet-4-5-20250929"

    def initialize
      @contacts = Contact.where.not(person_id: nil).order(email_count: :desc)
    end

    def scan!
      return [] if @contacts.count < 2

      clear_previous_flags
      result = call_claude
      return [] unless result

      matches = result["potential_duplicates"] || []
      flagged = flag_matches(matches)

      create_notifications if flagged > 0
      matches
    rescue => e
      Rails.logger.error("[AiDeduplicator] Scan failed: #{e.message}")
      []
    end

    private

    def clear_previous_flags
      Contact.where.not(suggested_person_id: nil).update_all(
        suggested_person_id: nil, suggested_reason: nil, suggested_confidence: nil
      )
    end

    def flag_matches(matches)
      flagged = 0
      matches.each do |match|
        ids = match["contact_ids"]
        reason = match["reason"]
        confidence = match["confidence"].to_f
        next unless ids.is_a?(Array) && ids.length >= 2

        contacts = Contact.where(id: ids).order(email_count: :desc)
        next unless contacts.length >= 2

        primary_person = contacts.first.person
        next unless primary_person

        contacts[1..].each do |secondary|
          next if secondary.suggested_person_id.present?
          next if secondary.person_id == primary_person.id

          secondary.update!(
            suggested_person: primary_person,
            suggested_reason: reason,
            suggested_confidence: confidence
          )
          flagged += 1
        end
      end
      flagged
    end

    def create_notifications
      workspace = Current.workspace
      return unless workspace

      # Action-required, scoped to this workspace's users. notifiable: workspace
      # lets the merge flow auto-resolve it once no duplicates remain.
      workspace.users.find_each do |user|
        Notification.notify(
          user: user,
          category: :contact,
          priority: :action_required,
          title: "Possible duplicate contacts found",
          body: "Scout found contacts that might be the same person. Review and merge them.",
          link_url: "/contacts",
          group_key: "duplicate_contacts/#{workspace.id}",
          notifiable: workspace,
          respect_preferences: false
        )
      end
    rescue => e
      Rails.logger.error("[AiDeduplicator] Notification error: #{e.message}")
    end

    def call_claude
      contact_list = @contacts.map.with_index do |c|
        person = c.person
        "- ID #{c.id}: #{person&.name || c.name || "unnamed"} | #{c.email} | #{person&.organization || c.organization || "no org"} | #{person&.relationship_type || c.relationship_type || "?"} | #{c.email_count} emails | Person##{c.person_id}"
      end.join("\n")

      user_message = <<~MSG
        <contact_list>
        #{contact_list}
        </contact_list>

        Identify groups of contacts that are likely the SAME PERSON using different email addresses or with slight name variations. Look for:

        1. **Different email addresses for the same person** — personal + work email, same person at different companies
        2. **Name variations** — "John Smith" vs "Johnny Smith" vs "J. Smith" vs "John S."
        3. **Initials** — "J.D." matching "John Doe" based on context
        4. **First-name-only matches with organizational overlap** — same first name AND same organization
        5. **Identical names** — two contacts with exactly the same name but different Person records

        Only flag when reasonably confident. Do NOT flag:
        - Different people sharing a common first name (e.g., two different "Joãos")
        - Automated/system senders with generic names
        - Names that only match on first name with no other evidence

        For each match, provide a clear reason and a confidence score (0.0 to 1.0).
        These will be reviewed by a human — flag anything that looks suspicious.
      MSG

      text = generate_text(dedup_system_prompt, user_message, 300)
      return nil unless text

      JSON.parse(text)
    rescue JSON::ParserError => e
      Rails.logger.error("[AiDeduplicator] Invalid JSON: #{e.message}")
      nil
    rescue => e
      Rails.logger.error("[AiDeduplicator] API error: #{e.message}")
      nil
    end

    # Prefer the workspace's configured text provider; fall back to the global
    # Anthropic key (legacy) when none is configured, so this honors a configured
    # provider but still works where it used to. Returns the raw model text.
    def generate_text(system_prompt, user_message, max_tokens)
      config = Ai::Configuration.for_any(AiConfiguration::TEXT_PURPOSES)
      if config
        config[:adapter].chat(
          system: system_prompt,
          messages: [ { role: "user", content: user_message } ],
          model: config[:model],
          max_tokens: max_tokens,
          temperature: config[:temperature]
        )
      elsif Ai::LegacyFallback.allowed?
        client = Anthropic::Client.new
        response = client.messages.create(
          model: MODEL,
          max_tokens: max_tokens,
          system: system_prompt,
          messages: [ { role: "user", content: user_message } ],
          thinking: { type: "disabled" }
        )
        response.content.find { |c| c.type.to_s == "text" }&.text
      end
    end

    def dedup_system_prompt
      org = Current.workspace
      org_context = org&.workspace_context
      company_name = org&.company_name || "the workspace"

      <<~PROMPT
        You are Scout, an AI assistant that flags potential duplicate contacts for manual review.

        #{org_context ? "<workspace_context>\n#{org_context}\n</workspace_context>\n" : ""}

        Your task: examine a list of contacts and identify which ones MIGHT be the same person using different email addresses or name variations. These will be flagged for HUMAN REVIEW — flag anything that looks suspect.

        Be thorough but reasonable:
        - "Maria Silva" and "Maria S." at the same organization → flag
        - "João Santos" and "João Silva" → do NOT flag (different last names, no evidence)
        - "J. Ferreira" and "João Ferreira" → flag (initials match)
        - "john@personal.com" and "john@company.com" with similar name patterns → flag
        - First name at company X and full name at same company X → flag
        - Two different "info@..." addresses → do NOT flag (automated senders)

        Respond with valid JSON only:
        {"potential_duplicates": [
          {
            "contact_ids": [5, 12],
            "reason": "Same person: 'J. Smith' and 'John Smith' appear to be the same person, both associated with #{company_name}",
            "confidence": 0.85
          }
        ]}
        Return an empty array if no duplicates found. Sort by confidence descending.
      PROMPT
    end
  end
end
