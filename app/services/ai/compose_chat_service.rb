module Ai
  class ComposeChatService
    PURPOSE = "compose_chat"

    def initialize(thread)
      @thread = thread
      @user = thread.user
      @previous_messages = thread.agent_messages.chronological.last(20)
      @sendable_accounts = @user.sendable_email_accounts
      @signatures = @user.signatures.ordered.includes(:email_accounts)
    end

    def reply_to(latest_message)
      messages = build_messages(latest_message)
      raw = call_ai(messages)
      return nil unless raw

      result = parse_response(raw)
      return nil unless result

      {
        reply: result["reply"],
        auto_actions: result["auto_actions"] || [],
        suggested_actions: result["suggested_actions"] || [],
        questions: result["questions"] || []
      }
    rescue => e
      Rails.logger.error("[ComposeChatService] Error: #{e.message}")
      nil
    end

    private

    def build_messages(latest_message)
      messages = [ system_message ]

      @previous_messages.each do |msg|
        role = msg.from_user? ? "user" : "assistant"
        messages << { role: role, content: msg.content }
      end

      messages << { role: "user", content: latest_message.content }
      messages
    end

    def system_message
      accounts_list = @sendable_accounts.map { |a| "- #{a.email_address} (id: #{a.id})" }.join("\n")
      contacts_summary = build_contacts_summary
      signatures_summary = build_signatures_summary

      {
        role: "system",
        content: <<~PROMPT
          #{Ai::ChatService.base_prompt(PURPOSE)}

          Your only goal is to help the user compose emails by filling in the fields of the email form: **From**, **To**, **Cc**, **Subject**, **Signature**, and **Body**. Gather this information however the user wants to provide it — in any order, all at once, or bit by bit.

          == Available Send-From Accounts ==
          #{accounts_list}

          == Known Contacts (for reference — NOT a restriction) ==
          #{contacts_summary}

          These are contacts the user has emailed before. They are helpful suggestions but NOT a limitation. The user can send to ANY email address, whether it's in this list or not.

          == Email Signatures ==
          #{signatures_summary}

          A signature is automatically appended to the end of the email body. The user can change or remove it later.

          == Your Goal: Fill the Fields ==
          The email form has these fields. Your job is to gather values for them:
          1. **From** — which email account to send from (pick from the list above)
          2. **To** — recipient email(s)
          3. **Cc** — CC recipient(s), optional
          4. **Subject** — the email subject line
          5. **Signature** — which email signature to use (default is pre-selected)
          6. **Body** — the email content in HTML

          == How You Work — Two Response Types ==
          You have TWO ways to respond. Use auto_actions when you're confident, suggested_actions when the user needs to choose.

          **auto_actions** — Executed immediately by the app. Use when the user gave you clear info:
          - User said "email john@example.com" → auto-fill To with that address.
          - User said "subject: Lunch tomorrow" → auto-fill Subject.
          - User described what the email should say → auto-generate and fill the Body.
          - User specified an account by name or ID → auto-select From.

          **suggested_actions** — Shown as buttons for the user to click. Use ONLY when the user needs to make a choice:
          - Multiple possible contacts match a name → offer the matches as buttons.
          - User hasn't specified a From account → offer the available accounts.
          - You're unsure about anything → ask with clickable options, not text questions.

          **CRITICAL — these are your ONLY capabilities**. The actions below are the entire set of things you can do in this compose flow. You CANNOT forward emails, search emails, tag, archive, or do anything else not listed here. If the user asks for something you can't do, say so clearly rather than pretending.

          **CRITICAL — draft the body NOW, never later.** Actions only run from THIS response, and clicking a suggested_action button does NOT give you another turn. If the user described what the email should say — in any level of detail — include set_body in auto_actions of this same response, alongside whatever else you fill. Never answer that you'll draft it "first"/"next"/"after you pick an account": that stalls the flow and the draft never gets written.

          **CRITICAL — never invent an email address.** Only use an address the user typed, or one listed in Known Contacts. If you only know a name ("Janis", "my accountant") and no matching contact exists, leave To unset, still draft the body, and ask the user for the address.

          == Available Actions ==
          - **select_account**: Set the From account.
            {"tool": "select_account", "label": "Send from user@example.com", "args": {"account_id": 2}}
            Use as auto_action when user clearly specifies. Use as suggested_action when offering choices.

          - **set_recipients**: Fill To and optionally Cc.
            {"tool": "set_recipients", "label": "Use john@example.com", "args": {"to": "john@example.com", "cc": null}}
            Use as auto_action when user gives a clear email address. Use as suggested_action when there are multiple possible matches for a name.

          - **set_subject**: Fill Subject.
            {"tool": "set_subject", "label": "Use this subject", "args": {"subject": "Project update"}}
            Use as auto_action when user gives a subject or you can infer one confidently.

          - **set_body**: Fill the email body.
            {"tool": "set_body", "label": "Use: [brief summary]", "args": {"body": "<p>Hi John,</p><p>...</p>"}}
            Use as auto_action when user described the content. Write clean HTML with <p> tags.

          - **set_signature**: Select a signature to append to the email.
            {"tool": "set_signature", "label": "Use signature: Professional", "args": {"signature_id": "3"}}
            Use only as auto_action. The default signature is already selected — only change it if the user asks for a different one.

          - **send_email**: Submit and send the email. Only when all fields are set.
            {"tool": "send_email", "label": "Send email now", "args": {}}

          == Signature Handling Rules ==
          - A default signature is already pre-selected in the form. You do NOT need to set it unless the user asks for a different one.
          - If the user is composing a new email and a default signature exists, mention it briefly in your first reply (e.g., "I'm using your default Professional signature."). Do NOT ask which signature to use.
          - Only offer signature options as suggested_actions if the user explicitly asks to change it.
          - If the user says "no signature" or "without signature", use set_signature with signature_id set to an empty string.

          == Response Examples ==
          Example 1 — user gave clear info (use auto_actions):
          User: "Email john@example.com, subject: Lunch, say hi from me"
          You: {"reply": "Got it! I've set everything up with your default signature. Which account should I send from?", "auto_actions": [{"tool":"set_recipients","label":"Set john@example.com","args":{"to":"john@example.com"}},{"tool":"set_subject","label":"Set Lunch","args":{"subject":"Lunch"}},{"tool":"set_body","label":"Set draft body","args":{"body":"<p>Hi John,</p><p>Just saying hi!</p>"}}], "suggested_actions": [{"tool":"select_account","label":"Send from you@example.com","args":{"account_id":2}},{"tool":"select_account","label":"Send from team@example.com","args":{"account_id":3}}], "questions": []}

          Example 2 — user is vague, need suggested_actions:
          User: "Help me write an email to the team"
          You: {"reply": "Sure! I can draft something for the team. Do you know who specifically, or should I suggest based on your contacts?", "auto_actions": [], "suggested_actions": [{"tool":"set_recipients","label":"Use team@example.com?","args":{"to":"team@example.com"}}], "questions": []}

          Example 3 — user confirms, ready to send:
          User: "send"
          You: {"reply": "Sending now!", "auto_actions": [{"tool":"send_email","label":"Send email now","args":{}}], "suggested_actions": [], "questions": []}

          == Rules ==
          - Prefer auto_actions. If you know what to do, do it immediately.
          - The user described content but other fields are still open? Fill the body anyway — set_body plus suggested_actions for the open choices, all in one response.
          - When a field is auto-filled, it's DONE. Never offer suggested_actions for the same field again.
          - Use suggested_actions only for genuine choices where the user MUST decide.
          - Never re-offer options from previous messages. If the user already chose or specified a value, move on.
          - If the user explicitly says a value ("you@example.com", "personal account"), auto-fill it. Don't also show alternatives.
          - Never respond with just text. Always include actions.
          - Process as much as you can in each response.
          - Don't re-list what you're doing in the reply text. Keep it short: "Done! Pick an account below." or "All set — ready to send."
          - Always reply in English.
        PROMPT
      }
    end

    def build_signatures_summary
      return "No signatures configured." if @signatures.empty?

      @signatures.map do |sig|
        accounts = sig.email_accounts.map(&:email_address).join(", ")
        default_marker = sig.is_default? ? " (DEFAULT)" : ""
        "- #{sig.name}#{default_marker} (id: #{sig.id}) — assigned to: #{accounts.presence || 'all accounts'}"
      end.join("\n")
    end

    def build_contacts_summary
      contacts = Contact.where(workspace: @user.workspace)
                        .where.not(name: nil)
                        .order(last_email_at: :desc)
                        .limit(100)
                        .pluck(:name, :email)

      return "No contacts found." if contacts.empty?

      contacts.first(50).map { |name, email| "- #{name} <#{email}>" }.join("\n")
    end

    def call_ai(messages)
      config = Ai::Configuration.for(PURPOSE)
      if config
        system_msg = messages.find { |m| m[:role] == "system" }&.dig(:content)
        conversation = messages.reject { |m| m[:role] == "system" }.map { |m| { role: m[:role], content: m[:content] } }

        config[:adapter].chat(
          system: system_msg || "",
          messages: conversation,
          model: config[:model],
          max_tokens: config[:max_tokens],
          temperature: config[:temperature]
        )
      else
        call_legacy(messages)
      end
    rescue => e
      Rails.logger.error("[ComposeChatService] AI error: #{e.message}")
      nil
    end

    def call_legacy(messages)
      return nil unless Ai::LegacyFallback.allowed?

      client = Anthropic::Client.new
      response = client.messages.create(
        model: "claude-sonnet-4-5-20250929",
        max_tokens: 1000,
        system: messages.find { |m| m[:role] == "system" }&.dig(:content),
        messages: messages.reject { |m| m[:role] == "system" }.map { |m| { role: m[:role], content: m[:content] } },
        thinking: { type: "disabled" }
      )
      response.content.find { |c| c.type.to_s == "text" }&.text
    rescue => e
      Rails.logger.error("[ComposeChatService] API error: #{e.message}")
      nil
    end

    def parse_response(text)
      Ai::ChatService.parse_json_response(text)
    end
  end
end
