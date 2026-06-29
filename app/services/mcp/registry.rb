# frozen_string_literal: true

module Mcp
  # The MCP tool catalog. Each tool reuses the SAME query/service + Api::V1
  # serializer its REST twin uses, so MCP and REST never drift in shape or
  # permission semantics. Tools declare the Doorkeeper scope they require; the
  # controller filters tools/list to the scopes the token holds and re-checks on
  # tools/call. Handlers run inside an Api::McpController request, so
  # Current.workspace / Current.acting_user are already established and every
  # existing access gate (accessible_to, Current.workspace.<assoc>) applies.
  module Registry
    module_function

    DEFAULT_LIMIT = 20
    MAX_LIMIT = 50

    # All tools, memoized. Tool objects are immutable and handlers read Current.*
    # at call time, so a single shared list is safe across requests.
    def all
      @all ||= definitions.freeze
    end

    def find(name)
      all.find { |tool| tool.name == name }
    end

    # Tools the caller may see: enabled (Features flag on) AND scope granted.
    # `granted` answers token_has_scope?(scope_string).
    def visible_to(granted)
      all.select { |tool| tool.available? && granted.call(tool.scope) }
    end

    # ---- catalog ----------------------------------------------------------

    def definitions
      [
        list_emails, get_email, send_email, reply_email,
        list_documents, list_contacts,
        list_calendar_events, create_calendar_event,
        list_scheduled_emails, create_scheduled_email,
        list_reminders, list_email_templates
      ]
    end

    # ---- email ------------------------------------------------------------

    def list_emails
      build(
        name: "list_emails",
        description: "List the most recent emails the caller can access, newest first. " \
                     "Optional filters: unread only, a text query on subject/sender, and a limit.",
        scope: "emails:read",
        input_schema: object_schema(properties: {
          limit: limit_property,
          unread: { type: "boolean", description: "When true, only unread emails" },
          query: { type: "string", description: "Case-insensitive substring match on subject or sender" }
        })
      ) do |args|
        scope = EmailMessage.accessible_to(Current.user).includes(:tags).order(received_at: :desc)
        scope = scope.where(read: false) if args["unread"] == true
        if args["query"].present?
          like = "%#{args["query"]}%"
          scope = scope.where("email_messages.subject ILIKE :q OR email_messages.from_address ILIKE :q", q: like)
        end
        { emails: scope.limit(clamp_limit(args["limit"])).map { |e| Api::V1::EmailSerializer.new(e).as_json } }
      end
    end

    def get_email
      build(
        name: "get_email",
        description: "Fetch a single email by id, including its body.",
        scope: "emails:read",
        input_schema: object_schema(
          properties: { id: { type: "integer", description: "The email id" } },
          required: [ "id" ]
        )
      ) do |args|
        require_arg(args, "id")
        email = EmailMessage.accessible_to(Current.user).find(args["id"])
        { email: Api::V1::EmailSerializer.new(email, detail: true).as_json }
      end
    end

    def send_email
      build(
        name: "send_email",
        description: "Send a new email from one of the caller's connected accounts.",
        scope: "emails:send",
        input_schema: object_schema(
          properties: {
            email_account_id: { type: "integer", description: "Id of the sending account (must be one the caller may send from)" },
            to_address: { type: "string", description: "Recipient address(es), comma-separated" },
            subject: { type: "string" },
            body: { type: "string", description: "HTML or plain-text body" },
            cc_address: { type: "string" },
            bcc_address: { type: "string" }
          },
          required: [ "email_account_id", "to_address" ]
        )
      ) do |args|
        require_arg(args, "email_account_id", "to_address")
        result = Emails::Sender.call(
          user: Current.user, email_account_id: args["email_account_id"],
          to_address: args["to_address"], subject: args["subject"], body: args["body"],
          cc_address: args["cc_address"], bcc_address: args["bcc_address"]
        )
        raise Mcp::ToolError, (result.error_message || "Could not send the email.") unless result.ok?

        { id: result.email_message&.id, provider_message_id: result.provider_message_id }
      end
    end

    def reply_email
      build(
        name: "reply_email",
        description: "Reply to an existing email. Threads from the source message and sends from its " \
                     "account unless email_account_id is given.",
        scope: "emails:send",
        input_schema: object_schema(
          properties: {
            id: { type: "integer", description: "The email to reply to" },
            body: { type: "string" },
            to_address: { type: "string", description: "Override recipient (defaults to the original sender)" },
            cc_address: { type: "string" },
            bcc_address: { type: "string" },
            email_account_id: { type: "integer", description: "Override sending account" }
          },
          required: [ "id", "body" ]
        )
      ) do |args|
        require_arg(args, "id", "body")
        source = EmailMessage.accessible_to(Current.user).find(args["id"])
        result = Emails::Sender.call(
          user: Current.user, source_message: source, email_account_id: args["email_account_id"],
          to_address: args["to_address"].presence || source.from_address,
          cc_address: args["cc_address"], bcc_address: args["bcc_address"],
          subject: reply_subject(source), body: args["body"]
        )
        raise Mcp::ToolError, (result.error_message || "Could not send the reply.") unless result.ok?

        { id: result.email_message&.id, provider_message_id: result.provider_message_id }
      end
    end

    # ---- documents & contacts --------------------------------------------

    def list_documents
      build(
        name: "list_documents",
        description: "List the workspace's documents, newest first. Optional filters by document type id and review status.",
        scope: "documents:read",
        input_schema: object_schema(properties: {
          limit: limit_property,
          document_type_id: { type: "integer" },
          review_status: { type: "string", description: "e.g. pending, approved, rejected" }
        })
      ) do |args|
        scope = Current.workspace.documents.recent
        scope = scope.where(document_type_id: args["document_type_id"]) if args["document_type_id"].present?
        if args["review_status"].present? && Document.review_statuses.key?(args["review_status"])
          scope = scope.by_review_status(args["review_status"])
        end
        { documents: scope.limit(clamp_limit(args["limit"])).map { |d| Api::V1::DocumentSerializer.new(d).as_json } }
      end
    end

    def list_contacts
      build(
        name: "list_contacts",
        description: "List the workspace's contacts. Optional text query on name/email and a starred-only filter.",
        scope: "contacts:read",
        input_schema: object_schema(properties: {
          limit: limit_property,
          query: { type: "string", description: "Substring match on name or email" },
          starred: { type: "boolean" }
        })
      ) do |args|
        scope = Current.workspace.contacts
        scope = scope.starred if args["starred"] == true
        if args["query"].present?
          like = "%#{args["query"]}%"
          scope = scope.where("contacts.name ILIKE :q OR contacts.email ILIKE :q", q: like)
        end
        { contacts: scope.order(:name).limit(clamp_limit(args["limit"])).map { |c| Api::V1::ContactSerializer.new(c).as_json } }
      end
    end

    # ---- calendar ---------------------------------------------------------

    def list_calendar_events
      build(
        name: "list_calendar_events",
        description: "List calendar events the caller can access, soonest first. Optional start_after / start_before ISO-8601 bounds.",
        scope: "calendar:read",
        input_schema: object_schema(properties: {
          limit: limit_property,
          start_after: { type: "string", description: "ISO-8601; only events starting at/after this" },
          start_before: { type: "string", description: "ISO-8601; only events starting before this" }
        })
      ) do |args|
        scope = CalendarEvent.accessible_to(Current.user).order(start_at: :asc)
        scope = scope.where("calendar_events.start_at >= ?", parse_time(args["start_after"])) if parse_time(args["start_after"])
        scope = scope.where("calendar_events.start_at < ?", parse_time(args["start_before"])) if parse_time(args["start_before"])
        { events: scope.limit(clamp_limit(args["limit"])).map { |e| Api::V1::CalendarEventSerializer.new(e).as_json } }
      end
    end

    def create_calendar_event
      build(
        name: "create_calendar_event",
        description: "Create a calendar event on one of the caller's writable calendars. Times are ISO-8601.",
        scope: "calendar:write",
        input_schema: object_schema(
          properties: {
            calendar_id: { type: "integer", description: "Id of a writable calendar" },
            title: { type: "string" },
            start_at: { type: "string", description: "ISO-8601 start" },
            end_at: { type: "string", description: "ISO-8601 end" },
            description: { type: "string" },
            location: { type: "string" },
            all_day: { type: "boolean" },
            color: { type: "string", description: "Hex color, optional" }
          },
          required: [ "calendar_id", "title", "start_at" ]
        )
      ) do |args|
        require_arg(args, "calendar_id", "title", "start_at")
        calendar = Calendar.where(calendar_account: Current.user.writable_calendar_accounts, is_writable: true, syncing: true)
                           .find_by(id: args["calendar_id"])
        raise Mcp::ToolError, "That calendar does not exist or is not writable." unless calendar

        event = calendar.calendar_events.new(
          title: args["title"], description: args["description"], location: args["location"],
          start_at: args["start_at"], end_at: args["end_at"], all_day: args["all_day"] || false,
          color: args["color"], provider_event_id: "local-#{SecureRandom.uuid}",
          status: :confirmed, outbound_pending: true
        )
        event.save!
        Calendars::EventWriteJob.perform_later(event.id, "create")
        { event: Api::V1::CalendarEventSerializer.new(event, detail: true).as_json }
      end
    end

    # ---- scheduled emails -------------------------------------------------

    def list_scheduled_emails
      build(
        name: "list_scheduled_emails",
        description: "List scheduled (and recurring) emails in the workspace, soonest occurrence first.",
        scope: "scheduled_emails:read",
        input_schema: object_schema(properties: {
          limit: limit_property,
          status: { type: "string", description: "Filter: pending, sent, cancelled, failed" }
        })
      ) do |args|
        scope = ScheduledEmail.accessible_to(Current.user).order(Arel.sql("COALESCE(next_occurrence_at, scheduled_at) ASC"))
        scope = scope.where(status: args["status"]) if args["status"].present? && ScheduledEmail.statuses.key?(args["status"])
        { scheduled_emails: scope.limit(clamp_limit(args["limit"])).map { |s| Api::V1::ScheduledEmailSerializer.new(s).as_json } }
      end
    end

    def create_scheduled_email
      build(
        name: "create_scheduled_email",
        description: "Schedule an email to send later (optionally recurring via an RRULE). Sends from an account the caller may use.",
        scope: "scheduled_emails:write",
        input_schema: object_schema(
          properties: {
            email_account_id: { type: "integer" },
            to_address: { type: "string" },
            subject: { type: "string" },
            body: { type: "string" },
            scheduled_at: { type: "string", description: "ISO-8601 first send time" },
            rrule: { type: "string", description: "Optional iCal RRULE, e.g. FREQ=WEEKLY;INTERVAL=1" },
            cc_address: { type: "string" },
            bcc_address: { type: "string" }
          },
          required: [ "email_account_id", "to_address", "subject", "body", "scheduled_at" ]
        )
      ) do |args|
        require_arg(args, "email_account_id", "to_address", "subject", "body", "scheduled_at")
        ensure_entitled!(:email_scheduling)
        unless Current.user.sendable_email_accounts.exists?(id: args["email_account_id"])
          raise Mcp::ToolError, "You can't send from that email account."
        end

        record = ScheduledEmail.new(
          workspace: Current.workspace, created_by: Current.user,
          email_account_id: args["email_account_id"], to_address: args["to_address"],
          cc_address: args["cc_address"], bcc_address: args["bcc_address"],
          subject: args["subject"], body: args["body"],
          scheduled_at: args["scheduled_at"], rrule: args["rrule"]
        )
        record.save!
        next_at = record.rrule.present? ? ScheduleCalculator.next_occurrence(record.scheduled_at, record.rrule) : record.scheduled_at
        record.update_columns(next_occurrence_at: next_at)
        { scheduled_email: Api::V1::ScheduledEmailSerializer.new(record, detail: true).as_json }
      end
    end

    # ---- reminders & templates -------------------------------------------

    def list_reminders
      build(
        name: "list_reminders",
        description: "List AI-extracted reminders the caller can access. Optional status filter (pending, confirmed, dismissed, snoozed).",
        scope: "reminders:read",
        input_schema: object_schema(properties: {
          limit: limit_property,
          status: { type: "string" }
        })
      ) do |args|
        scope = Reminder.accessible_to(Current.user).order(due_at: :asc)
        scope = scope.where(status: args["status"]) if args["status"].present? && Reminder.statuses.key?(args["status"])
        { reminders: scope.limit(clamp_limit(args["limit"])).map { |r| Api::V1::ReminderSerializer.new(r).as_json } }
      end
    end

    def list_email_templates
      build(
        name: "list_email_templates",
        description: "List the workspace's reusable email templates.",
        scope: "templates:read",
        enabled: -> { Features.email_templates? },
        input_schema: object_schema(properties: { limit: limit_property })
      ) do |args|
        scope = Current.workspace.email_templates.recent
        { email_templates: scope.limit(clamp_limit(args["limit"])).map { |t| Api::V1::EmailTemplateSerializer.new(t).as_json } }
      end
    end

    # ---- helpers ----------------------------------------------------------

    def build(name:, description:, scope:, input_schema:, enabled: -> { true }, &handler)
      Tool.new(name: name, description: description, scope: scope,
               input_schema: input_schema, handler: handler, enabled: enabled)
    end

    def object_schema(properties:, required: [])
      { type: "object", properties: properties, required: required }
    end

    def limit_property
      { type: "integer", description: "Max results, 1-#{MAX_LIMIT} (default #{DEFAULT_LIMIT})" }
    end

    def clamp_limit(value)
      n = value.to_i
      n = DEFAULT_LIMIT if n <= 0
      [ n, MAX_LIMIT ].min
    end

    def require_arg(args, *keys)
      missing = keys.select { |key| args[key].nil? || args[key].to_s.strip.empty? }
      return if missing.empty?

      raise Mcp::RpcError.new(-32_602, "Missing required argument(s): #{missing.join(', ')}")
    end

    def ensure_entitled!(feature_key)
      return if Current.workspace&.entitlements&.feature?(feature_key)

      raise Mcp::ToolError, "Your plan does not include this feature."
    end

    def reply_subject(source)
      subject = source.subject.to_s
      subject.match?(/\Are:/i) ? subject : "Re: #{subject}"
    end

    def parse_time(value)
      return nil if value.blank?

      Time.zone.parse(value.to_s)
    rescue ArgumentError
      nil
    end
  end
end
