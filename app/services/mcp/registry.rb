# frozen_string_literal: true

module Mcp
  # The MCP tool catalog. Each tool reuses the SAME query/service + Api::V1
  # serializer its REST twin uses, so MCP and REST never drift in shape or
  # permission semantics. Tools declare the Doorkeeper scope they require; the
  # controller filters tools/list to the scopes the token holds and re-checks on
  # tools/call. Handlers run inside an Api::McpController request, so
  # Current.workspace / Current.acting_user are already established and every
  # existing access gate (accessible_to, Current.workspace.<assoc>, entitlements)
  # applies. The surface mirrors the public REST API; to add a capability, add a
  # tool here that calls the same code the REST controller does.
  module Registry
    module_function

    DEFAULT_LIMIT = 20
    MAX_LIMIT = 50

    # Editable document fields (mirror Api::V1::DocumentsController#document_params).
    DOCUMENT_FIELDS = %w[
      document_type_id vendor_name vendor_nif document_date due_date invoice_number
      amount_cents currency buyer_nif tax_amount_cents tax_rate description
      expense_category company_vat_present client_name client_nif bank_name
      account_number period_start period_end opening_balance_cents
      closing_balance_cents receipt_number payment_method metadata
    ].freeze

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
        # email
        list_emails, get_email, send_email, reply_email, mark_email_read, mark_email_unread,
        add_email_tag, remove_email_tag,
        # documents
        list_documents, get_document, upload_document, update_document,
        approve_document, reject_document, reclassify_document,
        # contacts / tags / document types
        list_contacts, get_contact, update_contact, set_contact_state,
        list_tags, list_document_types,
        # workflows (flag-gated)
        list_workflows, trigger_workflow, list_workflow_executions,
        # scout
        list_scout_threads, create_scout_thread, list_scout_messages, send_scout_message,
        # scheduled emails
        list_scheduled_emails, get_scheduled_email, create_scheduled_email,
        update_scheduled_email, cancel_scheduled_email,
        # calendar events
        list_calendar_events, get_calendar_event, create_calendar_event,
        update_calendar_event, delete_calendar_event, rsvp_calendar_event,
        # reminders
        list_reminders, get_reminder, confirm_reminder, dismiss_reminder, snooze_reminder,
        # folders
        list_folders, get_folder, file_document, unfile_document
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
        input_schema: id_schema("The email id")
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

    def mark_email_read
      build(
        name: "mark_email_read",
        description: "Mark an email as read and sync the flag to the provider mailbox.",
        scope: "emails:write",
        input_schema: id_schema("The email id")
      ) do |args|
        require_arg(args, "id")
        email = EmailMessage.accessible_to(Current.user).find(args["id"])
        email.update!(read: true)
        MarkReadJob.perform_later(email.email_account_id, [ email.provider_message_id ])
        { email: Api::V1::EmailSerializer.new(email, detail: true).as_json }
      end
    end

    def mark_email_unread
      build(
        name: "mark_email_unread",
        description: "Mark an email as unread (local only).",
        scope: "emails:write",
        input_schema: id_schema("The email id")
      ) do |args|
        require_arg(args, "id")
        email = EmailMessage.accessible_to(Current.user).find(args["id"])
        email.update!(read: false)
        { email: Api::V1::EmailSerializer.new(email, detail: true).as_json }
      end
    end

    def add_email_tag
      build(
        name: "add_email_tag",
        description: "Attach an existing workspace tag to an email (by tag_id or name). Tags are not created here.",
        scope: "tags:write",
        input_schema: object_schema(
          properties: {
            email_id: { type: "integer" },
            tag_id: { type: "integer", description: "The tag to attach (or pass name)" },
            name: { type: "string", description: "Tag name (case-insensitive) if tag_id is not given" }
          },
          required: [ "email_id" ]
        )
      ) do |args|
        require_arg(args, "email_id")
        email = EmailMessage.accessible_to(Current.user).find(args["email_id"])
        tag = resolve_tag(args)
        email.tags << tag unless email.tags.include?(tag)
        { tag: Api::V1::TagSerializer.new(tag).as_json }
      end
    end

    def remove_email_tag
      build(
        name: "remove_email_tag",
        description: "Detach a tag from an email.",
        scope: "tags:write",
        input_schema: object_schema(
          properties: {
            email_id: { type: "integer" },
            tag_id: { type: "integer" }
          },
          required: [ "email_id", "tag_id" ]
        )
      ) do |args|
        require_arg(args, "email_id", "tag_id")
        email = EmailMessage.accessible_to(Current.user).find(args["email_id"])
        tag = email.tags.find(args["tag_id"])
        email.tags.delete(tag)
        { ok: true }
      end
    end

    # ---- documents --------------------------------------------------------

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

    def get_document
      build(
        name: "get_document",
        description: "Fetch a document by id with its extracted fields and file info (download via the file's download_path with the same token).",
        scope: "documents:read",
        input_schema: id_schema("The document id")
      ) do |args|
        require_arg(args, "id")
        document = Current.workspace.documents.find(args["id"])
        { document: Api::V1::DocumentSerializer.new(document, detail: true).as_json }
      end
    end

    def upload_document
      build(
        name: "upload_document",
        description: "Upload a new document from base64 content. AI classification runs asynchronously (returns ai_status pending).",
        scope: "documents:write",
        input_schema: object_schema(
          properties: {
            filename: { type: "string" },
            content_base64: { type: "string", description: "The file bytes, Base64-encoded" },
            content_type: { type: "string", description: "Optional MIME type" }
          },
          required: [ "filename", "content_base64" ]
        )
      ) do |args|
        require_arg(args, "filename", "content_base64")
        document = Current.workspace.documents.new(source: :manual_upload, ai_status: :pending, review_status: :pending)
        document.original_file.attach(
          io: StringIO.new(Base64.decode64(args["content_base64"])),
          filename: args["filename"],
          content_type: args["content_type"].presence
        )
        document.save!
        DocumentProcessJob.perform_later(document.id)
        { document: Api::V1::DocumentSerializer.new(document, detail: true).as_json }
      end
    end

    def update_document
      build(
        name: "update_document",
        description: "Edit a document's extracted fields. Does not change its review state (use approve/reject/reclassify).",
        scope: "documents:write",
        input_schema: object_schema(
          properties: {
            id: { type: "integer" },
            document_type_id: { type: "integer" },
            vendor_name: { type: "string" },
            client_name: { type: "string" },
            invoice_number: { type: "string" },
            amount_cents: { type: "integer" },
            currency: { type: "string" },
            document_date: { type: "string", format: "date" },
            description: { type: "string" },
            metadata: { type: "object", additionalProperties: true }
          },
          required: [ "id" ]
        )
      ) do |args|
        require_arg(args, "id")
        document = Current.workspace.documents.find(args["id"])
        document.update!(args.slice(*DOCUMENT_FIELDS))
        document.generate_canonical_filename!
        { document: Api::V1::DocumentSerializer.new(document.reload, detail: true).as_json }
      end
    end

    def approve_document
      build(
        name: "approve_document",
        description: "Approve (sign off) a document.",
        scope: "documents:write",
        input_schema: id_schema("The document id")
      ) do |args|
        require_arg(args, "id")
        document = Current.workspace.documents.find(args["id"])
        document.approve!(by: Current.user)
        Notifier.documents_need_review(document.workspace, bump: false)
        Documents::FinalizeApprovalJob.perform_later(document.id)
        { document: Api::V1::DocumentSerializer.new(document.reload, detail: true).as_json }
      end
    end

    def reject_document
      build(
        name: "reject_document",
        description: "Reject a document.",
        scope: "documents:write",
        input_schema: id_schema("The document id")
      ) do |args|
        require_arg(args, "id")
        document = Current.workspace.documents.find(args["id"])
        document.reject!
        Notifier.documents_need_review(document.workspace, bump: false)
        { document: Api::V1::DocumentSerializer.new(document.reload, detail: true).as_json }
      end
    end

    def reclassify_document
      build(
        name: "reclassify_document",
        description: "Change a document's type (also approves it).",
        scope: "documents:write",
        input_schema: object_schema(
          properties: {
            id: { type: "integer" },
            document_type_id: { type: "integer" }
          },
          required: [ "id", "document_type_id" ]
        )
      ) do |args|
        require_arg(args, "id", "document_type_id")
        document = Current.workspace.documents.find(args["id"])
        type = Current.workspace.document_types.find(args["document_type_id"])
        document.reclassify!(type, by: Current.user)
        Notifier.documents_need_review(document.workspace, bump: false)
        Documents::FinalizeApprovalJob.perform_later(document.id)
        { document: Api::V1::DocumentSerializer.new(document.reload, detail: true).as_json }
      end
    end

    # ---- contacts / tags / document types --------------------------------

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

    def get_contact
      build(
        name: "get_contact",
        description: "Fetch a single contact by id.",
        scope: "contacts:read",
        input_schema: id_schema("The contact id")
      ) do |args|
        require_arg(args, "id")
        contact = Current.workspace.contacts.find(args["id"])
        { contact: Api::V1::ContactSerializer.new(contact).as_json }
      end
    end

    def update_contact
      build(
        name: "update_contact",
        description: "Update a contact's name and/or relationship type.",
        scope: "contacts:write",
        input_schema: object_schema(
          properties: {
            id: { type: "integer" },
            name: { type: "string" },
            relationship_type: { type: "string" }
          },
          required: [ "id" ]
        )
      ) do |args|
        require_arg(args, "id")
        contact = Current.workspace.contacts.find(args["id"])
        person = contact.person || Person.create!(workspace: Current.workspace)
        contact.update!(person: person) unless contact.person_id == person.id
        person.update!({ name: args["name"], relationship_type: args["relationship_type"] }.compact)
        contact.update_columns(name: person.name, relationship_type: person.relationship_type)
        { contact: Api::V1::ContactSerializer.new(contact.reload).as_json }
      end
    end

    def set_contact_state
      build(
        name: "set_contact_state",
        description: "Star/unstar, allow, block, or unblock a contact.",
        scope: "contacts:write",
        input_schema: object_schema(
          properties: {
            id: { type: "integer" },
            state: { type: "string", enum: %w[star unstar allow block unblock] }
          },
          required: [ "id", "state" ]
        )
      ) do |args|
        require_arg(args, "id", "state")
        contact = Current.workspace.contacts.find(args["id"])
        case args["state"]
        when "star"    then contact.star!
        when "unstar"  then contact.unstar!
        when "allow"   then contact.allow!
        when "block"   then Contacts::Block.call(contact, user: Current.user)
        when "unblock" then Contacts::Unblock.call(contact, user: Current.user)
        else raise Mcp::ToolError, "state must be one of: star, unstar, allow, block, unblock."
        end
        { contact: Api::V1::ContactSerializer.new(contact.reload).as_json }
      end
    end

    def list_tags
      build(
        name: "list_tags",
        description: "List the workspace's tags (tags apply to emails).",
        scope: "tags:read",
        input_schema: object_schema(properties: { limit: limit_property })
      ) do |args|
        { tags: Current.workspace.tags.by_name.limit(clamp_limit(args["limit"])).map { |t| Api::V1::TagSerializer.new(t).as_json } }
      end
    end

    def list_document_types
      build(
        name: "list_document_types",
        description: "List the workspace's document types (used to classify documents).",
        scope: "document_types:read",
        input_schema: object_schema(properties: {})
      ) do |_args|
        { document_types: Current.workspace.document_types.order(:category, :name).map { |t| Api::V1::DocumentTypeSerializer.new(t).as_json } }
      end
    end

    # ---- workflows (gated behind Features.workflows?) ---------------------

    def list_workflows
      build(
        name: "list_workflows",
        description: "List the workspace's automation workflows.",
        scope: "workflows:read",
        enabled: -> { Features.workflows? },
        input_schema: object_schema(properties: { limit: limit_property })
      ) do |args|
        scope = Current.workspace.workflows.order(created_at: :desc)
        { workflows: scope.limit(clamp_limit(args["limit"])).map { |w| Api::V1::WorkflowSerializer.new(w).as_json } }
      end
    end

    def trigger_workflow
      build(
        name: "trigger_workflow",
        description: "Trigger an enabled webhook workflow with an optional JSON payload.",
        scope: "workflows:trigger",
        enabled: -> { Features.workflows? },
        input_schema: object_schema(
          properties: {
            id: { type: "integer" },
            payload: { type: "object", additionalProperties: true, description: "Exposed to the workflow's Liquid templates" }
          },
          required: [ "id" ]
        )
      ) do |args|
        require_arg(args, "id")
        workflow = Current.workspace.workflows.find(args["id"])
        raise Mcp::ToolError, "This workflow is disabled." unless workflow.enabled?
        raise Mcp::ToolError, "Only webhook workflows can be triggered." unless workflow.webhook?

        payload = args["payload"].is_a?(Hash) ? args["payload"] : {}
        WorkflowWebhookJob.perform_later(workflow.id, payload: payload, headers: {}, query: {}, source_ip: nil)
        { ok: true, workflow_id: workflow.id }
      end
    end

    def list_workflow_executions
      build(
        name: "list_workflow_executions",
        description: "List a workflow's run history (newest first).",
        scope: "workflows:read",
        enabled: -> { Features.workflows? },
        input_schema: object_schema(
          properties: { workflow_id: { type: "integer" }, limit: limit_property },
          required: [ "workflow_id" ]
        )
      ) do |args|
        require_arg(args, "workflow_id")
        workflow = Current.workspace.workflows.find(args["workflow_id"])
        { executions: workflow.executions.limit(clamp_limit(args["limit"])).map { |e| Api::V1::WorkflowExecutionSerializer.new(e).as_json } }
      end
    end

    # ---- scout ------------------------------------------------------------

    def list_scout_threads
      build(
        name: "list_scout_threads",
        description: "List the caller's Scout chat threads, newest first.",
        scope: "scout:read",
        input_schema: object_schema(properties: { limit: limit_property })
      ) do |args|
        scope = Current.user.agent_threads.scout_visible.recent
        { threads: scope.limit(clamp_limit(args["limit"])).map { |t| Api::V1::AgentThreadSerializer.new(t).as_json } }
      end
    end

    def create_scout_thread
      build(
        name: "create_scout_thread",
        description: "Start a new Scout chat thread.",
        scope: "scout:write",
        input_schema: object_schema(properties: { title: { type: "string" } })
      ) do |args|
        thread = Current.user.agent_threads.create!(
          title: args["title"].presence || "New chat", workspace_id: Current.user.workspace_id
        )
        { thread: Api::V1::AgentThreadSerializer.new(thread).as_json }
      end
    end

    def list_scout_messages
      build(
        name: "list_scout_messages",
        description: "List messages in a Scout thread. Pass after_message_id to poll for the async AI reply.",
        scope: "scout:read",
        input_schema: object_schema(
          properties: {
            thread_id: { type: "integer" },
            after_message_id: { type: "integer", description: "Only messages created after this one" }
          },
          required: [ "thread_id" ]
        )
      ) do |args|
        require_arg(args, "thread_id")
        thread = Current.user.agent_threads.find(args["thread_id"])
        scope = thread.agent_messages.chronological
        if args["after_message_id"].present? && (pivot = thread.agent_messages.find_by(id: args["after_message_id"]))
          scope = scope.where("agent_messages.created_at > ?", pivot.created_at)
        end
        { messages: scope.map { |m| Api::V1::AgentMessageSerializer.new(m).as_json } }
      end
    end

    def send_scout_message
      build(
        name: "send_scout_message",
        description: "Post a user message to a Scout thread. The AI reply is generated asynchronously; poll list_scout_messages(after_message_id) for it.",
        scope: "scout:write",
        input_schema: object_schema(
          properties: {
            thread_id: { type: "integer" },
            content: { type: "string" }
          },
          required: [ "thread_id", "content" ]
        )
      ) do |args|
        require_arg(args, "thread_id", "content")
        unless Ai::ProviderSetup.available?(Current.workspace, :text)
          raise Mcp::ToolError, "This workspace has no AI provider configured for chat."
        end

        thread = Current.user.agent_threads.find(args["thread_id"])
        message = thread.agent_messages.create!(content: args["content"], author_type: :user, user: Current.user)
        AgentChatReplyJob.perform_later(message.id)
        { message: Api::V1::AgentMessageSerializer.new(message).as_json }
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

    def get_scheduled_email
      build(
        name: "get_scheduled_email",
        description: "Fetch a scheduled email by id.",
        scope: "scheduled_emails:read",
        input_schema: id_schema("The scheduled email id")
      ) do |args|
        require_arg(args, "id")
        record = ScheduledEmail.accessible_to(Current.user).find(args["id"])
        { scheduled_email: Api::V1::ScheduledEmailSerializer.new(record, detail: true).as_json }
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
        ensure_sendable!(args["email_account_id"])
        record = ScheduledEmail.new(
          workspace: Current.workspace, created_by: Current.user,
          email_account_id: args["email_account_id"], to_address: args["to_address"],
          cc_address: args["cc_address"], bcc_address: args["bcc_address"],
          subject: args["subject"], body: args["body"],
          scheduled_at: args["scheduled_at"], rrule: args["rrule"]
        )
        record.save!
        stamp_next_occurrence(record)
        { scheduled_email: Api::V1::ScheduledEmailSerializer.new(record, detail: true).as_json }
      end
    end

    def update_scheduled_email
      build(
        name: "update_scheduled_email",
        description: "Update a pending scheduled email (recipient, subject, body, time, rrule).",
        scope: "scheduled_emails:write",
        input_schema: object_schema(
          properties: {
            id: { type: "integer" },
            email_account_id: { type: "integer" },
            to_address: { type: "string" },
            subject: { type: "string" },
            body: { type: "string" },
            scheduled_at: { type: "string" },
            rrule: { type: "string" },
            cc_address: { type: "string" },
            bcc_address: { type: "string" }
          },
          required: [ "id" ]
        )
      ) do |args|
        require_arg(args, "id")
        ensure_entitled!(:email_scheduling)
        record = ScheduledEmail.accessible_to(Current.user).find(args["id"])
        ensure_sendable!(args["email_account_id"]) if args.key?("email_account_id")
        record.update!(args.slice("email_account_id", "to_address", "cc_address", "bcc_address",
                                  "subject", "body", "scheduled_at", "rrule"))
        stamp_next_occurrence(record)
        { scheduled_email: Api::V1::ScheduledEmailSerializer.new(record, detail: true).as_json }
      end
    end

    def cancel_scheduled_email
      build(
        name: "cancel_scheduled_email",
        description: "Cancel a scheduled email (soft: sets status to cancelled).",
        scope: "scheduled_emails:write",
        input_schema: id_schema("The scheduled email id")
      ) do |args|
        require_arg(args, "id")
        ensure_entitled!(:email_scheduling)
        record = ScheduledEmail.accessible_to(Current.user).find(args["id"])
        record.update!(status: :cancelled)
        { scheduled_email: Api::V1::ScheduledEmailSerializer.new(record, detail: true).as_json }
      end
    end

    # ---- calendar events --------------------------------------------------

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

    def get_calendar_event
      build(
        name: "get_calendar_event",
        description: "Fetch a calendar event by id.",
        scope: "calendar:read",
        input_schema: id_schema("The calendar event id")
      ) do |args|
        require_arg(args, "id")
        event = CalendarEvent.accessible_to(Current.user).find(args["id"])
        { event: Api::V1::CalendarEventSerializer.new(event, detail: true).as_json }
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
        calendar = writable_calendars.find_by(id: args["calendar_id"])
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

    def update_calendar_event
      build(
        name: "update_calendar_event",
        description: "Update a calendar event (you must have write access to its calendar). recurrence_scope: this|all.",
        scope: "calendar:write",
        input_schema: object_schema(
          properties: {
            id: { type: "integer" },
            title: { type: "string" }, description: { type: "string" }, location: { type: "string" },
            start_at: { type: "string" }, end_at: { type: "string" },
            all_day: { type: "boolean" }, color: { type: "string" },
            recurrence_scope: { type: "string", enum: %w[this all] }
          },
          required: [ "id" ]
        )
      ) do |args|
        require_arg(args, "id")
        event = writable_event(args["id"])
        event.update!(args.slice("title", "description", "location", "start_at", "end_at", "all_day", "color")
                          .merge(outbound_pending: true))
        Calendars::EventWriteJob.perform_later(event.id, "update", recurrence_scope(args))
        { event: Api::V1::CalendarEventSerializer.new(event, detail: true).as_json }
      end
    end

    def delete_calendar_event
      build(
        name: "delete_calendar_event",
        description: "Delete a calendar event (async provider delete). recurrence_scope: this|all.",
        scope: "calendar:write",
        input_schema: object_schema(
          properties: { id: { type: "integer" }, recurrence_scope: { type: "string", enum: %w[this all] } },
          required: [ "id" ]
        )
      ) do |args|
        require_arg(args, "id")
        event = writable_event(args["id"])
        event.update_columns(outbound_pending: true)
        Calendars::EventWriteJob.perform_later(event.id, "delete", recurrence_scope(args))
        { ok: true, id: event.id }
      end
    end

    def rsvp_calendar_event
      build(
        name: "rsvp_calendar_event",
        description: "Set your RSVP on an event (needs_action, accepted, declined, tentative).",
        scope: "calendar:write",
        input_schema: object_schema(
          properties: {
            id: { type: "integer" },
            rsvp_status: { type: "string", enum: %w[needs_action accepted declined tentative] }
          },
          required: [ "id", "rsvp_status" ]
        )
      ) do |args|
        require_arg(args, "id", "rsvp_status")
        raise Mcp::ToolError, "Invalid rsvp_status." unless CalendarEvent.rsvp_statuses.key?(args["rsvp_status"])

        event = writable_event(args["id"])
        event.update_columns(rsvp_status: CalendarEvent.rsvp_statuses[args["rsvp_status"]], outbound_pending: true)
        Calendars::EventWriteJob.perform_later(event.id, "rsvp")
        { event: Api::V1::CalendarEventSerializer.new(event, detail: true).as_json }
      end
    end

    # ---- reminders --------------------------------------------------------

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

    def get_reminder
      build(
        name: "get_reminder",
        description: "Fetch a reminder by id.",
        scope: "reminders:read",
        input_schema: id_schema("The reminder id")
      ) do |args|
        require_arg(args, "id")
        reminder = Reminder.accessible_to(Current.user).find(args["id"])
        { reminder: Api::V1::ReminderSerializer.new(reminder, detail: true).as_json }
      end
    end

    def confirm_reminder
      build(
        name: "confirm_reminder",
        description: "Confirm a reminder into a calendar event. Optionally pass due_at to adjust the time first.",
        scope: "reminders:write",
        input_schema: object_schema(
          properties: { id: { type: "integer" }, due_at: { type: "string", description: "ISO-8601" } },
          required: [ "id" ]
        )
      ) do |args|
        require_arg(args, "id")
        reminder = Reminder.accessible_to(Current.user).find(args["id"])
        if args["due_at"].present?
          parsed = parse_time(args["due_at"])
          raise Mcp::ToolError, "Invalid due_at." unless parsed

          reminder.update!(due_at: parsed)
        end
        result = Reminders::Confirm.call(reminder, user: Current.user)
        raise Mcp::ToolError, (result.error || "Could not confirm the reminder.") unless result.success?

        Api::V1::ReminderSerializer.new(reminder.reload, detail: true).as_json.merge(calendar_event_id: result.calendar_event&.id)
      end
    end

    def dismiss_reminder
      build(
        name: "dismiss_reminder",
        description: "Dismiss a reminder.",
        scope: "reminders:write",
        input_schema: id_schema("The reminder id")
      ) do |args|
        require_arg(args, "id")
        reminder = Reminder.accessible_to(Current.user).find(args["id"])
        reminder.dismissed!
        { reminder: Api::V1::ReminderSerializer.new(reminder, detail: true).as_json }
      end
    end

    def snooze_reminder
      build(
        name: "snooze_reminder",
        description: "Snooze a reminder until the given time, or one week out when omitted.",
        scope: "reminders:write",
        input_schema: object_schema(
          properties: { id: { type: "integer" }, until: { type: "string", description: "ISO-8601" } },
          required: [ "id" ]
        )
      ) do |args|
        require_arg(args, "id")
        reminder = Reminder.accessible_to(Current.user).find(args["id"])
        reminder.update!(status: :snoozed, snoozed_until: parse_time(args["until"]) || 1.week.from_now)
        { reminder: Api::V1::ReminderSerializer.new(reminder, detail: true).as_json }
      end
    end

    # ---- folders ----------------------------------------------------------

    def list_folders
      build(
        name: "list_folders",
        description: "List the workspace's custom folders.",
        scope: "folders:read",
        input_schema: object_schema(properties: {})
      ) do |_args|
        { folders: Current.workspace.mail_folders.ordered.map { |f| Api::V1::FolderSerializer.new(f).as_json } }
      end
    end

    def get_folder
      build(
        name: "get_folder",
        description: "Fetch a folder and the documents filed into it.",
        scope: "folders:read",
        input_schema: id_schema("The folder id")
      ) do |args|
        require_arg(args, "id")
        folder = Current.workspace.mail_folders.find(args["id"])
        { folder: Api::V1::FolderSerializer.new(folder, detail: true).as_json }
      end
    end

    def file_document
      build(
        name: "file_document",
        description: "File a document into a folder.",
        scope: "folders:write",
        input_schema: object_schema(
          properties: { mail_folder_id: { type: "integer" }, document_id: { type: "integer" } },
          required: [ "mail_folder_id", "document_id" ]
        )
      ) do |args|
        require_arg(args, "mail_folder_id", "document_id")
        folder = Current.workspace.mail_folders.find(args["mail_folder_id"])
        document = Current.workspace.documents.find(args["document_id"])
        membership = folder.folder_memberships.find_or_create_by!(folderable: document)
        { id: membership.id, folder_id: folder.id, document_id: document.id }
      end
    end

    def unfile_document
      build(
        name: "unfile_document",
        description: "Remove a document from a folder (by membership id).",
        scope: "folders:write",
        input_schema: id_schema("The folder_membership id")
      ) do |args|
        require_arg(args, "id")
        membership = FolderMembership.joins(:mail_folder)
                                     .where(mail_folders: { workspace_id: Current.workspace.id })
                                     .find(args["id"])
        membership.destroy
        { ok: true }
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

    def id_schema(description)
      object_schema(properties: { id: { type: "integer", description: description } }, required: [ "id" ])
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

    def ensure_sendable!(account_id)
      return if account_id.present? && Current.user.sendable_email_accounts.exists?(id: account_id)

      raise Mcp::ToolError, "You can't send from that email account."
    end

    def resolve_tag(args)
      if args["tag_id"].present?
        Current.workspace.tags.find(args["tag_id"])
      elsif args["name"].present?
        Current.workspace.tags.find_by!("LOWER(name) = ?", args["name"].to_s.downcase.strip)
      else
        raise Mcp::RpcError.new(-32_602, "Provide tag_id or name.")
      end
    end

    def writable_calendars
      Calendar.where(calendar_account: Current.user.writable_calendar_accounts, is_writable: true, syncing: true)
    end

    def writable_event(id)
      event = CalendarEvent.accessible_to(Current.user).find(id)
      unless event.calendar.is_writable && event.calendar_account.writable_by?(Current.user)
        raise Mcp::ToolError, "You do not have write access to this calendar event."
      end

      event
    end

    def recurrence_scope(args)
      %w[this all].include?(args["recurrence_scope"]) ? args["recurrence_scope"] : "this"
    end

    def stamp_next_occurrence(record)
      next_at = record.rrule.present? ? ScheduleCalculator.next_occurrence(record.scheduled_at, record.rrule) : record.scheduled_at
      record.update_columns(next_occurrence_at: next_at)
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
