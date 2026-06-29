module Ai
  # Scout's voice in a task discussion thread. Mirrors Ai::EmailChatService but
  # feeds task context (title/status/priority/due/assignees/linked emails) instead
  # of email bodies. Reuses the email-chat AI config (no dedicated task_chat config
  # needed); the job only calls this when a comment @scout-tagged Scout.
  class TaskChatService
    PURPOSE = "task_chat"
    PURPOSES = %w[task_chat email_chat email_analysis].freeze
    MODEL = "claude-sonnet-4-5-20250929"
    MAX_TOKENS = 1200

    def initialize(task, thread = nil)
      @task = task
      @thread = thread || task.agent_thread
      @comments = @thread.agent_messages.chronological
    end

    def reply_to(latest_comment)
      config = Ai::Configuration.for_any(PURPOSES)
      return nil unless config

      text = config[:adapter].chat(
        system:      system_message,
        messages:    chat_messages(latest_comment),
        model:       config[:model],
        max_tokens:  MAX_TOKENS,
        temperature: 0.3
      )
      return nil if text.blank?

      parsed = Ai::ChatService.parse_json_response(text)
      reply = parsed["reply"].to_s.strip.presence || text.to_s.strip
      {
        reply:             reply,
        auto_actions:      [],
        suggested_actions: [],
        questions:         [],
        provenance:        Ai::Provenance.for_purpose(PURPOSE, legacy_model: MODEL)
      }
    rescue => e
      Rails.logger.error("[TaskChatService] Error for task #{@task.id}: #{e.message}")
      nil
    end

    private

    def chat_messages(latest_comment)
      turns = [ { role: "user", content: task_context } ]

      # Prior comments, labelled by author — a multi-teammate discussion.
      @comments.where.not(id: latest_comment.id).each do |c|
        turns << { role: c.from_user? ? "user" : "assistant", content: "#{c.author_name}: #{c.content}" }
      end

      turns << { role: "user", content: "#{latest_comment.author_name} (tagged you with @scout): #{latest_comment.content}" }
      turns
    end

    def system_message
      Ai::ChatService.base_prompt(PURPOSE) + <<~PROMPT
        You are a participant in a team DISCUSSION about a single TASK (a to-do the
        team needs to complete). Help them move it forward: clarify the next step,
        draft any text they ask for, summarize, or suggest how to approach it. The
        task's details and any linked emails are given below as context.

        You only speak when someone tags you with @scout; otherwise teammates are
        talking among themselves. Reply conversationally and concisely in markdown,
        addressing the person who tagged you. Do not invent task details.

        Respond with valid JSON only, exactly: {"reply": "your markdown reply"}.
      PROMPT
    end

    def task_context
      strip = ActionController::Base.helpers
      lines = [
        "Title: #{@task.title}",
        "Status: #{@task.status}",
        "Priority: #{@task.priority}",
        "Due: #{@task.due_at&.iso8601 || 'none'}",
        "Assignees: #{@task.assignees.map(&:name).join(', ').presence || 'none'}",
        "Description: #{strip.strip_tags(@task.description.to_s).presence || 'none'}"
      ]
      emails = email_lines
      lines << "Linked emails:\n#{emails.join("\n")}" if emails.any?
      "<task>\n#{lines.join("\n")}\n</task>"
    end

    def email_lines
      out = []
      out << "- origin: #{@task.source_email.subject}" if @task.source_email
      @task.task_email_links.includes(:email_message).each { |l| out << "- #{l.relationship}: #{l.email_message.subject}" }
      out
    end
  end
end
