module Ai
  class EmailChatService
    MODEL = "claude-sonnet-4-5-20250929"
    PURPOSE = "email_chat"

    def initialize(email_thread)
      @thread = email_thread
      @messages = email_thread.email_messages.order(received_at: :asc)
      @comments = email_thread.agent_messages.chronological
    end

    def reply_to(latest_comment)
      result = call_claude(chat_messages(latest_comment))
      return nil unless result

      {
        reply: result["reply"],
        auto_actions: result["auto_actions"] || [],
        suggested_actions: result["suggested_actions"] || [],
        questions: result["questions"] || [],
        provenance: Ai::Provenance.for_purpose(PURPOSE, legacy_model: MODEL)
      }
    rescue => e
      Rails.logger.error("[EmailChatService] Error for thread #{@thread.id}: #{e.message}")
      nil
    end

    private

    def chat_messages(latest_comment)
      messages = [ system_message ]

      @messages.each do |msg|
        messages << {
          role: "user",
          content: "<email_message>\nSubject: #{msg.subject}\nFrom: #{msg.from_address}\nYour email: #{@thread.email_account.email_address}\nReceived: #{msg.received_at}\n\n#{truncate_body(msg.body.to_s)}\n</email_message>"
        }
      end

      # Add previous comments as conversation. This is a team discussion, so each
      # comment is labelled with its author's name — multiple teammates may be
      # talking, and Scout needs to tell them apart.
      @comments.where.not(id: latest_comment.id).each do |c|
        messages << {
          role: c.from_user? ? "user" : "assistant",
          content: "#{c.author_name}: #{c.content}"
        }
      end

      # Add the latest comment — the one that tagged @scout. Name the author so
      # Scout addresses the right person.
      messages << {
        role: "user",
        content: "#{latest_comment.author_name} (tagged you with @scout): #{latest_comment.content}"
      }

      messages
    end

    def system_message
      analysis_text = ""
      analyzed = @messages.select { |m| m.ai_summary.present? }
      if analyzed.any?
        parts = analyzed.map { |m| "Summary: #{m.ai_summary}\nAction: #{m.ai_action_prompt}" }
        analysis_text = "\n\nMy previous analysis of email(s) in this thread:\n#{parts.join("\n---\n")}"
      end

      contact_contexts = @messages.map(&:from_address).uniq.map { |addr|
        Contacts::ContactContextBuilder.new(addr).context_for_prompt
      }.compact.uniq.join("\n")

      # Ground tagging in real workspace state: Scout can only attach tags that
      # already exist (same as the manual UI), so it must see the actual list
      # instead of inventing names it then can't apply.
      workspace = @thread.email_account&.workspace
      tag_names = workspace ? workspace.tags.by_name.pluck(:name).uniq : []
      tags_text = tag_names.any? ? tag_names.join(", ") : "(no tags exist yet — do not suggest add_tag)"

      {
        role: "system",
        content: <<~PROMPT
          #{Ai::ChatService.base_prompt(PURPOSE)}

          You are Scout, a participant in a team discussion about an email thread. Several teammates from the same workspace may be commenting — each comment is labelled with its author's name. You are only speaking now because a teammate tagged you with "@scout"; address that person by name and answer them directly.

          This workspace owns the inbox — all email messages shown were received by it. "From" is the sender.
          The inbox address is #{@thread.email_account.email_address}.

          **CC awareness**: Check email headers (To:, CC:) to determine if the user is the primary recipient or just CC'd. If the user is only CC'd on every message in the thread, be less proactive — fewer tool suggestions, lower urgency. CC = informational, not actionable.

          The conversation above contains:
          - The email thread messages (enclosed in <email_message> tags)
          - The team discussion: each comment is prefixed with its author's name (your own past replies are prefixed "Scout:")
          #{analysis_text}

          #{contact_contexts}
          #{contact_contexts.present? ? "Use the contact context above to understand who each participant is and tailor your responses accordingly." : ""}

          **How to respond:**
          1. Answer the user's latest message directly, in 1-2 sentences. Do NOT summarize the thread unless asked.
          2. Then determine if the user is explicitly commanding an action. If so, use auto_actions. If recommending, use suggested_actions.
          3. Match tools to the user's exact request. Don't suggest unrelated tools.

          **auto_actions vs suggested_actions:**
          - `auto_actions`: For safe, reversible actions (add_tag, remove_tag, archive, trash, star_sender, unstar_sender, block_sender, unblock_sender, allow_sender). User explicitly commands → execute immediately. Confirm in past tense.
          - `suggested_actions`: For actions that send email (forward_email, draft_reply, send_reply) OR when you're proactively recommending. Render as clickable buttons the user must click to confirm.
          - **CRITICAL — sending email**: forward_email, draft_reply, send_reply, and send_draft MUST go in suggested_actions on FIRST mention. Even when the user explicitly commands them. Your reply should say you're ready and ask the user to click the button — do NOT use past tense ("Forwarded to...") because the action hasn't happened yet.
          - **Confirmation flow**: Read the conversation history. If you previously suggested an action (forward_email, draft_reply, send_reply) and the user's latest message confirms it — e.g. "yes", "go ahead", "do it", "OK", or clicking a suggested action button — then put the tool in auto_actions to execute it NOW (with past-tense confirmation in your reply). If the user hasn't confirmed yet or this is a new request, use suggested_actions.
          - When the user's request is ambiguous, prefer suggested_actions.

          **IMPORTANT about actions**: suggested_actions render as clickable buttons below your reply — do NOT list or repeat button labels in your reply text. auto_actions execute immediately — your reply text should confirm what was done in past tense.

          **CRITICAL — these are your ONLY capabilities**. The tools listed below are the entire set of actions you can perform. You CANNOT send emails, reply to emails, delete emails, create labels, move emails to folders, or do anything else not listed. If the user asks for something not in this list, say "I can't do that — I can only [list relevant tools]." Never pretend to have completed an action you cannot perform.

          **Existing tags in this workspace** — tagging works exactly like the manual UI: you attach a tag that already exists, you cannot invent one. `add_tag`/`remove_tag` may ONLY use a name from this list (copied exactly). If none fit, do not suggest tagging.
          #{tags_text}
          #{existing_commitments_text}

          Available tools (only suggest when directly relevant):
          - `add_tag`: {"tool": "add_tag", "args": {"tag_name": "name"}} — `tag_name` MUST exactly match one of the existing tags listed above.
          - `remove_tag`: {"tool": "remove_tag", "args": {"tag_name": "name"}} — only a tag currently applied to this email.
          - `draft_reply`: {"tool": "draft_reply", "args": {"summary": "what the reply should cover"}}
            **Before drafting**: If you need dates, amounts, or specifics the user hasn't provided, ask questions FIRST. Only suggest draft_reply when you have enough info for a complete, sendable reply.
          - `forward_email`: {"tool": "forward_email", "args": {"to_address": "email@example.com", "note": "optional message"}}
            **Before forwarding**: If the user doesn't specify a destination address, ask for it. The note is optional — if the user provides context like "FYI" or "please handle this", include it as the note.
          - `archive`: {"tool": "archive", "args": {}}
          - `trash`: {"tool": "trash", "args": {}}
          - `create_calendar_event`: {"tool": "create_calendar_event", "args": {"title": "...", "start_time": "ISO8601 (optional)", "end_time": "ISO8601 (optional)"}} — add an event to the user's calendar from this email. Suggest when the email implies a meeting or deadline; omit any time you're unsure of (the user can adjust it).
          - `create_task_from_email`: {"tool": "create_task_from_email", "args": {"title": "imperative action", "due_at": "ISO8601 (optional)", "priority": "low|normal|high|urgent (optional)"}} — create a to-do task from this email. Suggest when the email asks the reader to DO something (send, review, approve, follow up). The email becomes the task's origin.
          - `link_task_to_email`: {"tool": "link_task_to_email", "args": {"task_id": "...", "relationship": "related|reference|follow_up|blocked_by (optional)"}} — link this email to an EXISTING task. Only use when you know the task_id (e.g. the user named a task).
          - `star_sender`: {"tool": "star_sender", "args": {}} — promote this sender; their emails get prominence in Skim and the feed and are never grouped with others.
          - `block_sender`: {"tool": "block_sender", "args": {}} — block this sender; their existing and future mail is archived out of the inbox. Reversible with unblock_sender.
          - `allow_sender`: {"tool": "allow_sender", "args": {}} — allow this sender into the inbox (relevant in whitelist mode).
          - `unstar_sender` / `unblock_sender`: {"tool": "unstar_sender", "args": {}} — reverse a star / block.

          You can ask clarifying questions: {"question": "...", "options": ["A", "B", "C"]}

          Respond with JSON only:
          {"reply": "your reply", "auto_actions": [...], "suggested_actions": [...], "questions": [...]}
        PROMPT
      }
    end

    # Surface calendar commitments already extracted from this thread so Scout
    # acknowledges them instead of re-suggesting create_calendar_event on every @scout
    # (the duplicate it used to produce). "" when there are none — mirrors analysis_text.
    def existing_commitments_text
      message_ids = @messages.map(&:id)
      return "" if message_ids.empty?

      events    = CalendarEvent.where(source_email_message_id: message_ids).where.not(status: :cancelled).order(:start_at)
      reminders = Reminder.where(source_type: "EmailMessage", source_id: message_ids, status: :pending).order(:due_at)
      return "" if events.empty? && reminders.empty?

      lines  = events.map { |e| "- Calendar event already created: #{e.title.inspect} (#{commitment_time(e.start_at)})" }
      lines += reminders.map { |r| "- Pending reminder awaiting the user's confirmation: #{r.title.inspect} (due #{commitment_time(r.due_at)})" }

      "\n\n**Commitments already extracted from this thread** — do NOT suggest create_calendar_event for these again; just acknowledge they're already set. Only suggest a new event if the user explicitly asks for a different, additional one.\n#{lines.join("\n")}"
    end

    def commitment_time(time)
      time ? time.strftime("%b %-d, %Y at %H:%M") : "time unspecified"
    end

    def truncate_body(body)
      body = body.to_s
      body = body[0..4000] if body.length > 4000
      body
    end

    def call_claude(messages)
      config = Ai::Configuration.for(PURPOSE)
      if config
        call_adapter(config, messages)
      else
        call_legacy(messages)
      end
    end

    def call_adapter(config, messages)
      system_msg = messages.find { |m| m[:role] == "system" }&.dig(:content)
      conversation = messages.reject { |m| m[:role] == "system" }.map { |m| { role: m[:role], content: m[:content] } }

      text = config[:adapter].chat(
        system: system_msg || "",
        messages: conversation,
        model: config[:model],
        max_tokens: config[:max_tokens],
        temperature: config[:temperature]
      )
      return nil unless text

      parse_response(text)
    rescue JSON::ParserError => e
      Rails.logger.warn("[EmailChatService] Response was not JSON, treating as plain text")
      { "reply" => strip_name_tag(text.to_s.strip), "suggested_actions" => [], "questions" => [] }
    rescue => e
      Rails.logger.error("[EmailChatService] Adapter error: #{e.message}")
      nil
    end

    def parse_response(text)
      result = Ai::ChatService.parse_json_response(text)
      result["reply"] = strip_name_tag(result["reply"]) if result["reply"]
      result
    end

    def strip_name_tag(text)
      text.to_s.sub(/\A\s*\[Scout\]:?\s*/, "")
    end

    def call_legacy(messages)
      return nil unless Ai::LegacyFallback.allowed?

      client = Anthropic::Client.new
      response = client.messages.create(
        model: MODEL,
        max_tokens: 500,
        system: messages.find { |m| m[:role] == "system" }&.dig(:content),
        messages: messages.reject { |m| m[:role] == "system" }.map { |m| { role: m[:role], content: m[:content] } },
        thinking: { type: "disabled" }
      )

      text = response.content.find { |c| c.type.to_s == "text" }&.text
      return nil unless text

      parse_response(text)
    rescue JSON::ParserError => e
      Rails.logger.warn("[EmailChatService] Invalid JSON, treating as plain text")
      { "reply" => strip_name_tag(text.to_s.strip), "suggested_actions" => [], "questions" => [] }
    rescue => e
      Rails.logger.error("[EmailChatService] API error: #{e.message}")
      nil
    end
  end
end
