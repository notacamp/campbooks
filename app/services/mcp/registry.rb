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

    # Default palette used when a caller omits the color for a new tag or document type.
    DEFAULT_COLORS = %w[#595dec #0584da #00a8a8 #2ea55c #dca81c #e76e08 #de3b3d #d44996 #767988].freeze

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
    # `granted` answers token_has_scope?(scope_string). A nil scope means the
    # tool is available to any authenticated client (the meta tools).
    def visible_to(granted)
      all.select { |tool| tool.available? && (tool.scope.nil? || granted.call(tool.scope)) }
    end

    # ---- catalog ----------------------------------------------------------

    def definitions
      [
        # meta (scope: nil — any authenticated client)
        get_overview, get_setup_status, guide,
        # email
        list_emails, search_emails, get_email, send_email, reply_email, mark_email_read, mark_email_unread,
        add_email_tag, remove_email_tag,
        # inbox actions
        update_emails, move_emails_to_folder, tag_emails, forward_email,
        # skim
        get_skim_deck, skim_decide,
        # accounts
        list_email_accounts, connect_email_account,
        # documents
        list_documents, get_document, upload_document, update_document,
        approve_document, reject_document, reclassify_document,
        # contacts / tags / document types
        list_contacts, get_contact, update_contact, set_contact_state,
        list_tags, list_document_types,
        # taxonomy (create)
        create_tag, create_document_type, create_folder,
        # tasks (all gated on Features.tasks?)
        list_tasks, get_task, create_task, update_task, complete_task, create_task_from_email,
        # calendar helpers
        list_calendars, create_event_from_email,
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
        # email templates
        list_email_templates,
        # folders
        list_folders, get_folder, file_document, unfile_document
      ]
    end

    # ---- meta ----------------------------------------------------------------

    def get_overview
      build(
        name: "get_overview",
        description: "Cheap snapshot of what needs attention. Call this first. " \
                     "Returns only the sections whose scope the token holds.",
        scope: nil,
        input_schema: object_schema(properties: {})
      ) do |_args|
        result = { hint: "Use guide(topic) to learn workflows; get_setup_status if something looks unconfigured." }

        if scope_granted?("emails:read")
          awaiting = Emails::AwaitingReply.new(Current.user)
          # SkimScope's relation carries a custom SELECT list that breaks COUNT —
          # strip it (the relation's LIMIT still caps the count at SkimScope::MAX).
          skim_count = Emails::SkimScope.for(Current.user).except(:select, :includes, :order).count
          accessible = EmailMessage.accessible_to(Current.user)
          result[:emails] = {
            unread_count: accessible.where(read: false).count,
            pinned_count: accessible.pinned.count,
            awaiting_reply_count: awaiting.count,
            skim_pending_count: skim_count
          }
        end

        if scope_granted?("documents:read")
          result[:documents] = {
            needs_review_count: Current.workspace.documents.needs_review.count,
            ai_failed_count: Current.workspace.documents.ai_failed_attention.count
          }
        end

        if scope_granted?("tasks:read") && Features.tasks? &&
            Current.workspace.entitlements.feature?(:tasks)
          tasks = Task.accessible_to(Current.user)
          result[:tasks] = {
            active_count: tasks.active.count,
            suggested_count: tasks.triage.count,
            due_today_count: tasks.active.where(due_at: Time.current.beginning_of_day..Time.current.end_of_day).count
          }
        end

        if scope_granted?("calendar:read")
          today_events = CalendarEvent.accessible_to(Current.user)
                                      .where(start_at: Time.current.beginning_of_day..Time.current.end_of_day)
                                      .order(:start_at)
          result[:calendar] = {
            today_count: today_events.count,
            today: today_events.limit(3).map { |e| { id: e.id, title: e.title, start_at: e.start_at&.iso8601 } }
          }
        end

        if scope_granted?("reminders:read")
          reminders = Reminder.accessible_to(Current.user)
          next_due = reminders.pending.order(:due_at).limit(3)
          result[:reminders] = {
            pending_count: reminders.pending.count,
            overdue_count: reminders.overdue.count,
            next: next_due.map { |r| { id: r.id, title: r.title, due_at: r.due_at&.iso8601, reminder_type: r.reminder_type } }
          }
        end

        result
      end
    end

    def get_setup_status
      build(
        name: "get_setup_status",
        description: "Workspace setup snapshot for onboarding and diagnostics. " \
                     "Lists gaps as next_steps to guide the setup flow.",
        scope: nil,
        input_schema: object_schema(properties: {})
      ) do |_args|
        resolver = Current.workspace.entitlements
        accounts = Current.workspace.email_accounts.active.to_a

        detail_rows = if scope_granted?("email_accounts:read")
          accounts.map do |a|
            { id: a.id, email_address: a.email_address, provider: a.provider,
              active: a.active?, scanning: a.actively_scanning?,
              last_scanned_at: a.last_scanned_at&.iso8601 }
          end
        end

        email_section = {
          count: accounts.size,
          active_count: accounts.count(&:active?),
          scanning_now: accounts.any?(&:actively_scanning?)
        }
        email_section[:details] = detail_rows if detail_rows

        ai_setup = Ai::ProviderSetup.new(Current.workspace)
        ai_section = {
          text_configured: ai_setup.configured?(:text),
          documents_configured: ai_setup.configured?(:documents),
          managed_available: Ai::Platform.available?,
          processing_enabled: Current.workspace.ai_processing_enabled?
        }

        taxonomy = {
          tags: Current.workspace.tags.count,
          document_types: Current.workspace.document_types.count,
          folders: Current.workspace.mail_folders.count
        }

        feat_keys = %i[workflows tasks email_board microsoft email_templates document_templates]
        features_hash = feat_keys.each_with_object({}) { |k, h| h[k] = Features.public_send(:"#{k}?") }

        ent_keys = %i[email_accounts tasks email_scheduling email_templates workflows]
        entitlements_hash = ent_keys.each_with_object({}) do |k, h|
          h[k.to_s] = resolver.allow?(k).to_s
        end

        next_steps = []
        next_steps << "Connect a mailbox via connect_email_account to start receiving email." if accounts.empty?
        unless ai_setup.configured?(:text)
          next_steps << "Configure an AI provider in Settings → AI to enable triage and Scout."
        end
        if Current.workspace.document_types.none?
          next_steps << "Create document types (create_document_type) so AI can classify your attachments."
        end
        if Current.workspace.tags.none?
          next_steps << "Create a few tags (create_tag) to label and filter important emails."
        end
        if Current.workspace.mail_folders.none?
          next_steps << "Create folders (create_folder) to organise emails and documents."
        end

        {
          workspace: {
            name: Current.workspace.name,
            version: Campbooks::VERSION,
            self_hosted: Rails.application.config.self_hosted
          },
          email_accounts: email_section,
          ai: ai_section,
          taxonomy: taxonomy,
          features: features_hash,
          entitlements: entitlements_hash,
          next_steps: next_steps.first(5)
        }
      end
    end

    def guide
      build(
        name: "guide",
        description: "Narrative guides for working with Campbooks over MCP. " \
                     "No topic → list of available topics. With topic → full markdown guide.",
        scope: nil,
        input_schema: object_schema(properties: {
          topic: {
            type: "string",
            description: "One of the topic names returned by a topic-less call",
            enum: Mcp::Guides::TOPICS.map { |t| t[:name] }
          }
        })
      ) do |args|
        topic = args["topic"].to_s.strip.presence
        if topic.nil?
          { topics: Mcp::Guides::TOPICS }
        else
          content = Mcp::Guides.load(topic)
          raise Mcp::ToolError, "Unknown topic: #{topic}. Call guide() with no args for the list." unless content

          { topic: topic, content: content }
        end
      end
    end

    # ---- email ---------------------------------------------------------------

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
        rows = scope.limit(clamp_limit(args["limit"])).map { |e| Api::V1::EmailSerializer.new(e).as_json }
        { emails: rows, count: rows.size }
      end
    end

    def search_emails
      build(
        name: "search_emails",
        description: "Search emails with filters. Uses semantic + keyword blend when a text query " \
                     "is given. Prefer this over list_emails when the caller wants to find specific messages.",
        scope: "emails:read",
        input_schema: object_schema(
          properties: {
            query: { type: "string", description: "Full-text search query (subject, body, sender)" },
            unread: { type: "boolean" },
            category: { type: "string", description: "Email category e.g. personal, promotions, updates" },
            sender: { type: "string", description: "Sender address or name fragment" },
            date_from: { type: "string", description: "ISO-8601 date; only emails received on or after" },
            date_to: { type: "string", description: "ISO-8601 date; only emails received before" },
            has_attachment: { type: "boolean" },
            limit: limit_property
          },
          required: %w[query]
        )
      ) do |args|
        require_arg(args, "query")
        search_params = { q: args["query"] }.tap do |p|
          p[:sender]         = args["sender"]    if args["sender"].present?
          p[:category]       = args["category"]  if args["category"].present?
          p[:date_from]      = args["date_from"] if args["date_from"].present?
          p[:date_to]        = args["date_to"]   if args["date_to"].present?
          p[:has_attachment] = "1"               if args["has_attachment"] == true
          p[:unread]         = "1"               if args["unread"] == true
        end

        searcher = Emails::Search.new(user: Current.user, params: search_params)
        records = if searcher.text_query?
          # results is a bounded Array (semantic + keyword blend), not a relation.
          searcher.results.first(clamp_limit(args["limit"]))
        else
          searcher.scope.order(received_at: :desc).limit(clamp_limit(args["limit"]))
        end

        rows = records.map do |email|
          snippet = email.ai_summary.presence ||
                    Emails::PlainText.of(email.body.to_s).truncate(200)
          {
            id: email.id,
            subject: email.subject,
            from: email.from_address,
            received_at: email.received_at&.iso8601,
            category: email.category,
            read: email.read,
            thread_id: email.email_thread_id,
            snippet: snippet
          }
        end

        { emails: rows, count: rows.size }
      end
    end

    def get_email
      build(
        name: "get_email",
        description: "Fetch a single email by id. format=text (default) returns plain text body, " \
                     "truncated at 20 000 chars; format=html returns the raw provider HTML. " \
                     "Also returns linked document/task/tag ids.",
        scope: "emails:read",
        input_schema: object_schema(
          properties: {
            id: { type: "string", description: "The email id" },
            format: { type: "string", enum: %w[text html], description: "Body format (default: text)" }
          },
          required: [ "id" ]
        )
      ) do |args|
        require_arg(args, "id")
        email = EmailMessage.accessible_to(Current.user).find(args["id"])
        data = Api::V1::EmailSerializer.new(email, detail: true).as_json

        if args["format"] == "html"
          # keep raw body as-is
        else
          plain = Emails::PlainText.of(email.body.to_s, strip_quotes: false)
          truncated = plain.length > 20_000
          data[:body] = plain.first(20_000)
          data[:body_truncated] = truncated
        end

        linked = {
          document_ids: email.documents.ids,
          tag_names: email.tag_names
        }
        linked[:task_ids] = email.linked_tasks.ids if Features.tasks?
        data[:linked] = linked

        { email: data }
      end
    end

    def send_email
      build(
        name: "send_email",
        description: "Send a new email from one of the caller's connected accounts.",
        scope: "emails:send",
        input_schema: object_schema(
          properties: {
            email_account_id: { type: "string", description: "Id of the sending account (must be one the caller may send from)" },
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
            id: { type: "string", description: "The email to reply to" },
            body: { type: "string" },
            to_address: { type: "string", description: "Override recipient (defaults to the original sender)" },
            cc_address: { type: "string" },
            bcc_address: { type: "string" },
            email_account_id: { type: "string", description: "Override sending account" }
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
            email_id: { type: "string" },
            tag_id: { type: "string", description: "The tag to attach (or pass name)" },
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
            email_id: { type: "string" },
            tag_id: { type: "string" }
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

    # ---- inbox actions -------------------------------------------------------

    def update_emails
      build(
        name: "update_emails",
        description: "Bulk-act on emails. archive/unarchive/trash/snooze/unsnooze act on whole threads " \
                     "(mirrors inbox UI). trash moves messages to the provider trash folder. " \
                     "mark_read/mark_unread flag is applied to all messages in the thread.",
        scope: "emails:write",
        input_schema: object_schema(
          properties: {
            ids: { type: "array", items: { type: "string" }, description: "Email ids (1–100 UUIDs)", minItems: 1, maxItems: 100 },
            action: { type: "string", enum: %w[archive unarchive trash snooze unsnooze mark_read mark_unread] },
            snoozed_until: { type: "string", description: "ISO-8601 (required for snooze action)" }
          },
          required: %w[ids action]
        )
      ) do |args|
        require_arg(args, "ids", "action")
        ids = Array(args["ids"]).first(100)
        action = args["action"].to_s

        result = case action
        when "archive"
          Tools::BulkArchive.call("email_ids" => ids)
        when "unarchive"
          Tools::BulkUnarchive.call("email_ids" => ids)
        when "mark_read"
          Tools::BulkMarkRead.call(email_ids: ids, read: true)
        when "mark_unread"
          Tools::BulkMarkRead.call(email_ids: ids, read: false)
        when "snooze"
          raise Mcp::ToolError, "snoozed_until is required for snooze." if args["snoozed_until"].blank?
          until_time = parse_time(args["snoozed_until"])
          raise Mcp::ToolError, "Invalid snoozed_until time." unless until_time

          Tools::BulkSnooze.call("email_ids" => ids, "snoozed_until" => until_time.iso8601)
        when "unsnooze"
          Tools::BulkUnsnooze.call("email_ids" => ids)
        when "trash"
          # Trash acts per-thread like BulkSnooze — one call per thread, using the
          # first accessible message as the representative (mirrors Tools::BulkSnooze).
          scope = EmailMessage.accessible_to(Current.user).where(id: ids)
          count = 0
          scope.includes(:email_account).find_each.group_by(&:email_account).each do |_account, messages|
            messages.group_by(&:email_thread).each do |thread, msgs|
              next unless thread

              result = Tools::Trash.call(msgs.first)
              count += 1 if result
            end
          end
          { trashed_count: count }
        else
          raise Mcp::ToolError, "Unknown action: #{action}."
        end

        result.merge(action: action)
      end
    end

    def move_emails_to_folder
      build(
        name: "move_emails_to_folder",
        description: "Move emails (and their full threads) to a folder. Pass folder_name to work " \
                     "cross-account — it provisions the provider folder on each mailbox if missing. " \
                     "Pass folder_id when you already have a provider folder id for a single account.",
        scope: "emails:write",
        input_schema: object_schema(
          properties: {
            ids: { type: "array", items: { type: "string" }, description: "Email ids (UUIDs)" },
            folder_name: { type: "string", description: "Folder name (cross-account; provisions if missing)" },
            folder_id: { type: "string", description: "Provider folder id (single-account)" }
          },
          required: %w[ids]
        )
      ) do |args|
        require_arg(args, "ids")
        unless args["folder_name"].present? || args["folder_id"].present?
          raise Mcp::RpcError.new(-32_602, "Provide folder_name or folder_id.")
        end

        Tools::BulkMoveToFolder.call(
          email_ids: Array(args["ids"]),
          folder_id: args["folder_id"],
          folder_name: args["folder_name"]
        )
      end
    end

    def tag_emails
      build(
        name: "tag_emails",
        description: "Add or remove a tag on a set of emails. The tag must exist — " \
                     "use create_tag to make new ones.",
        scope: "tags:write",
        input_schema: object_schema(
          properties: {
            ids: { type: "array", items: { type: "string" }, description: "Email ids (UUIDs)" },
            tag_name: { type: "string" },
            action: { type: "string", enum: %w[add remove], description: "Default: add" }
          },
          required: %w[ids tag_name]
        )
      ) do |args|
        require_arg(args, "ids", "tag_name")
        result = Tools::BulkTag.call(
          "email_ids" => Array(args["ids"]),
          "tag_name" => args["tag_name"],
          "action" => args["action"].presence || "add"
        )
        raise Mcp::ToolError, "#{result[:error]} — use create_tag to create it first." if result[:error]

        result
      end
    end

    def forward_email
      build(
        name: "forward_email",
        description: "Forward an email to another address.",
        scope: "emails:send",
        input_schema: object_schema(
          properties: {
            id: { type: "string", description: "Email id" },
            to_address: { type: "string", description: "Recipient address" }
          },
          required: %w[id to_address]
        )
      ) do |args|
        require_arg(args, "id", "to_address")
        msg = EmailMessage.accessible_to(Current.user).find(args["id"])
        result = EmailActions.run("forward_email", email_message: msg,
                                  args: { "to_address" => args["to_address"] }, user: Current.user)
        raise Mcp::ToolError, (result[:message] || "Could not forward the email.") unless result[:success]

        result.slice(:success, :tool, :message)
      end
    end

    # ---- skim ----------------------------------------------------------------

    def get_skim_deck
      build(
        name: "get_skim_deck",
        description: "Return the Skim inbox deck as compact rings and cluster cards. " \
                     "Apply decisions with skim_decide(action, email_ids). " \
                     "keep=dismiss from tray, archive=move out of inbox, promote=pin.",
        scope: "emails:read",
        input_schema: object_schema(properties: {
          theme: { type: "string", description: "Filter to a single ring theme (optional)" }
        })
      ) do |args|
        memory = Emails::SkimActionMemory.new(Current.user)
        rings = Emails::SkimDeck.for(
          Current.user,
          whitelist_mode: Current.workspace.whitelist_mode?,
          memory: memory
        )

        theme_filter = args["theme"].to_s.strip.presence
        rings = rings.select { |r| r[:theme].to_s == theme_filter } if theme_filter

        compact_rings = rings.map do |ring|
          {
            theme: ring[:theme],
            label: ring[:label],
            clusters: ring[:clusters].map { |c| compact_cluster(c) }
          }
        end

        {
          rings: compact_rings,
          hint: "Apply decisions with skim_decide(action, email_ids). " \
                "keep=dismiss from tray, archive=move out of inbox, promote=pin."
        }
      end
    end

    def skim_decide
      build(
        name: "skim_decide",
        description: "Apply a Skim triage decision to a cluster's emails. " \
                     "Mirrors the inbox UI learning loop — decisions train future suggestions.",
        scope: "emails:write",
        input_schema: object_schema(
          properties: {
            action: { type: "string", enum: %w[keep archive promote restore unpromote] },
            email_ids: { type: "array", items: { type: "string" }, description: "Email ids from the cluster" }
          },
          required: %w[action email_ids]
        )
      ) do |args|
        require_arg(args, "action", "email_ids")
        action = args["action"].to_s
        ids = Array(args["email_ids"])

        affected = case action
        when "keep"
          result = Emails::SkimDismiss.new(Current.user, ids).call
          Emails::SkimDecisionRecorder.record(Current.user, ids, action: "keep")
          result
        when "archive"
          result = Emails::SkimArchive.new(Current.user, ids).call
          Emails::SkimDecisionRecorder.record(Current.user, ids, action: "archive")
          result
        when "promote"
          result = Emails::SkimPromote.new(Current.user, ids).call
          Emails::SkimDecisionRecorder.record(Current.user, ids, action: "promote")
          result
        when "restore"
          Emails::SkimRestore.new(Current.user, ids).call
        when "unpromote"
          Emails::SkimUnpromote.new(Current.user, ids).call
        else
          raise Mcp::ToolError, "Unknown action: #{action}."
        end

        { action: action, affected: affected }
      end
    end

    # ---- accounts -----------------------------------------------------------

    def list_email_accounts
      build(
        name: "list_email_accounts",
        description: "List connected email accounts visible to the caller. " \
                     "Use the id as email_account_id for send_email / create_scheduled_email.",
        scope: "email_accounts:read",
        input_schema: object_schema(properties: {})
      ) do |_args|
        rows = Current.user.email_account_users.includes(:email_account).map do |eau|
          a = eau.email_account
          {
            id: a.id,
            email_address: a.email_address,
            provider: a.provider,
            active: a.active?,
            scanning: a.actively_scanning?,
            last_scanned_at: a.last_scanned_at&.iso8601,
            name: a.name,
            color: a.color,
            can_read: eau.can_read,
            can_send: eau.can_send,
            can_manage: eau.can_manage,
            owner: eau.owner
          }
        end
        { email_accounts: rows, count: rows.size }
      end
    end

    def connect_email_account
      build(
        name: "connect_email_account",
        description: "Connect a new email account. mode=web returns a URL to open in a browser " \
                     "(normal OAuth flow). mode=token is for self-hosted setups: supply a refresh_token " \
                     "minted with THIS server's configured OAuth client — tokens from a different " \
                     "client will fail refresh. Never echoes the refresh_token back.",
        scope: "email_accounts:write",
        input_schema: object_schema(
          properties: {
            mode: { type: "string", enum: %w[web token], description: "Default: web" },
            provider: { type: "string", enum: %w[zoho google microsoft] },
            refresh_token: { type: "string", description: "Required for token mode" },
            email_address: { type: "string", description: "Optional: verified server-side; error if mismatch" }
          }
        )
      ) do |args|
        mode = args["mode"].presence || "web"

        if mode == "web"
          next { connect_path: "/email_accounts/new",
                 note: "Open this path on your Campbooks server in a browser; the OAuth consent flow finishes there." }
        end

        # token mode
        provider = args["provider"].to_s.presence
        raise Mcp::RpcError.new(-32_602, "provider is required for token mode.") unless provider.present?
        raise Mcp::RpcError.new(-32_602, "refresh_token is required for token mode.") if args["refresh_token"].blank?
        raise Mcp::ToolError, "Microsoft accounts are not available on this server." if provider == "microsoft" && !Features.microsoft?

        # entitlement cap (mirrors EmailAccountCapGuard)
        unless Current.workspace.entitlements.allow?(:email_accounts) == :ok
          limit_val = Current.workspace.entitlements.limit(:email_accounts)
          plan_name = Current.workspace.entitlements.plan_name
          raise Mcp::ToolError, "Your plan (#{plan_name}) allows #{limit_val} connected mailbox(es). Upgrade to connect more."
        end

        # Validate token + resolve identity server-side (never trust caller-supplied email alone)
        identity = resolve_oauth_identity(provider, args["refresh_token"])

        if args["email_address"].present? && identity[:email].to_s.downcase != args["email_address"].to_s.downcase
          raise Mcp::ToolError, "The resolved email (#{identity[:email]}) does not match the supplied email_address."
        end

        # Create or reactivate exactly like the OAuth callbacks
        existing = EmailAccount.find_by(email_address: identity[:email])
        account = if existing
          existing.update!(refresh_token: args["refresh_token"], active: true)
          existing
        else
          EmailAccount.create!(
            email_address: identity[:email],
            provider: provider,
            provider_account_id: identity[:account_id],
            refresh_token: args["refresh_token"],
            workspace: Current.workspace
          )
        end

        account.email_account_users.find_or_create_by!(user: Current.user) do |entry|
          entry.owner = true
          entry.can_read = true
          entry.can_send = true
          entry.can_manage = true
        end

        # Calendar provisioning best-effort (rescue+log like the callbacks do)
        begin
          Calendars::AccountProvisioner.call(
            email_address: identity[:email],
            provider: provider.to_sym,
            refresh_token: args["refresh_token"],
            workspace: Current.workspace,
            owner: Current.user,
            provider_account_id: identity[:account_id]
          )
        rescue => e
          Rails.logger.warn("[MCP connect_email_account] calendar provisioning failed: #{e.message}")
        end

        Events.publish("email_account.connected", subject: account,
                       payload: { "email_address" => account.email_address, "provider" => account.provider })
        EmailScanJob.perform_later(account.id, "delta")

        { account: { id: account.id, email_address: account.email_address,
                     provider: account.provider, active: account.active? },
          scan_enqueued: true }
      end
    end

    # ---- documents ----------------------------------------------------------

    def list_documents
      build(
        name: "list_documents",
        description: "List the workspace's documents, newest first. Optional filters by document type id and review status.",
        scope: "documents:read",
        input_schema: object_schema(properties: {
          limit: limit_property,
          document_type_id: { type: "string" },
          review_status: { type: "string", description: "e.g. pending, approved, rejected" }
        })
      ) do |args|
        scope = Current.workspace.documents.recent
        scope = scope.where(document_type_id: args["document_type_id"]) if args["document_type_id"].present?
        if args["review_status"].present? && Document.review_statuses.key?(args["review_status"])
          scope = scope.by_review_status(args["review_status"])
        end
        rows = scope.limit(clamp_limit(args["limit"])).map { |d| Api::V1::DocumentSerializer.new(d).as_json }
        { documents: rows, count: rows.size }
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
            id: { type: "string" },
            document_type_id: { type: "string" },
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
            id: { type: "string" },
            document_type_id: { type: "string" }
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

    # ---- contacts / tags / document types ----------------------------------

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
        rows = scope.order(:name).limit(clamp_limit(args["limit"])).map { |c| Api::V1::ContactSerializer.new(c).as_json }
        { contacts: rows, count: rows.size }
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
            id: { type: "string" },
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
            id: { type: "string" },
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
        tags = Current.workspace.tags.visible.by_name.limit(clamp_limit(args["limit"])).to_a
        counts = EmailMessageTag.where(tag_id: tags.map(&:id)).group(:tag_id).count
        rows = tags.map { |t| Api::V1::TagSerializer.new(t, email_count: counts[t.id] || 0).as_json }
        { tags: rows, count: rows.size }
      end
    end

    def list_document_types
      build(
        name: "list_document_types",
        description: "List the workspace's document types (used to classify documents).",
        scope: "document_types:read",
        input_schema: object_schema(properties: {})
      ) do |_args|
        rows = Current.workspace.document_types.order(:category, :name).map { |t| Api::V1::DocumentTypeSerializer.new(t).as_json }
        { document_types: rows, count: rows.size }
      end
    end

    # ---- taxonomy (create) --------------------------------------------------

    def create_tag
      build(
        name: "create_tag",
        description: "Create a new workspace tag. Tags apply to emails and can be used for filtering.",
        scope: "tags:write",
        input_schema: object_schema(
          properties: {
            name: { type: "string" },
            color: { type: "string", description: "Hex color (optional; a default is assigned)" }
          },
          required: %w[name]
        )
      ) do |args|
        require_arg(args, "name")
        color = args["color"].presence || DEFAULT_COLORS.sample
        tag = Current.workspace.tags.build(
          name: args["name"], color: color, source: :local, kind: :user
        )
        tag.save!
        { tag: Api::V1::TagSerializer.new(tag).as_json }
      rescue ActiveRecord::RecordInvalid => e
        raise Mcp::ToolError, "Tag '#{args["name"]}' already exists." if e.message =~ /unique|already taken/i

        raise
      end
    end

    def create_document_type
      build(
        name: "create_document_type",
        description: "Create a new document type for classifying attachments.",
        scope: "document_types:write",
        input_schema: object_schema(
          properties: {
            name: { type: "string" },
            category: { type: "string", description: "One of: #{DocumentType::CATEGORIES.join(', ')}" },
            auto_star: { type: "boolean" },
            color: { type: "string", description: "Hex color (optional; a default is assigned)" }
          },
          required: %w[name]
        )
      ) do |args|
        require_arg(args, "name")
        category = args["category"].presence
        category = DocumentType.default_category_for(args["name"]) if category.blank?
        category = nil unless DocumentType::CATEGORIES.include?(category)

        doc_type = Current.workspace.document_types.build(
          name: args["name"],
          category: category,
          color: args["color"].presence || DEFAULT_COLORS.sample,
          auto_star: args["auto_star"] || false
        )
        doc_type.save!
        { document_type: Api::V1::DocumentTypeSerializer.new(doc_type).as_json }
      rescue ActiveRecord::RecordInvalid => e
        raise Mcp::ToolError, "Document type '#{args["name"]}' already exists." if e.message =~ /unique|already taken/i

        raise
      end
    end

    def create_folder
      build(
        name: "create_folder",
        description: "Create a custom folder. When provision: true, the folder is created on " \
                     "every connected mailbox the caller manages — useful for inbox organisation. " \
                     "Provisioning creates the folder on each provider; failures on individual " \
                     "accounts are reported but do not abort the others.",
        scope: "folders:write",
        input_schema: object_schema(
          properties: {
            name: { type: "string" },
            parent_id: { type: "string", description: "Optional parent folder id" },
            icon: { type: "string", description: "Optional emoji icon" },
            provision: { type: "boolean", description: "Create on all connected mailboxes (default: false)" }
          },
          required: %w[name]
        )
      ) do |args|
        require_arg(args, "name")
        folder = Current.workspace.mail_folders.build(
          name: args["name"],
          parent_id: args["parent_id"],
          icon: args["icon"]
        )
        folder.save!

        result = { folder: Api::V1::FolderSerializer.new(folder).as_json }

        if args["provision"]
          provision_result = MailFolders::Provisioner.provision_all(folder, Current.user)
          result[:provision] = {
            created_count: provision_result[:created].size,
            failed_count: provision_result[:failed].size
          }
        end

        result
      end
    end

    # ---- tasks (all gated on Features.tasks?) --------------------------------

    def list_tasks
      build(
        name: "list_tasks",
        description: "List workspace tasks. Optional status filter; include_archived to see archived tasks.",
        scope: "tasks:read",
        enabled: -> { Features.tasks? },
        input_schema: object_schema(properties: {
          status: { type: "string", description: "Filter by status (suggested/todo/in_progress/blocked/done/cancelled)" },
          include_archived: { type: "boolean" },
          limit: limit_property
        })
      ) do |args|
        ensure_entitled!(:tasks)
        scope = Task.accessible_to(Current.user).includes(:assignees, :tags)
        scope = args["include_archived"] ? scope : scope.not_archived
        scope = scope.where(status: args["status"]) if args["status"].present? && Task.statuses.key?(args["status"])
        { tasks: scope.order(created_at: :desc).limit(clamp_limit(args["limit"])).map { |t| Api::V1::TaskSerializer.new(t).as_json },
          count: scope.count }
      end
    end

    def get_task
      build(
        name: "get_task",
        description: "Fetch a task by id with full detail.",
        scope: "tasks:read",
        enabled: -> { Features.tasks? },
        input_schema: id_schema("The task id")
      ) do |args|
        ensure_entitled!(:tasks)
        require_arg(args, "id")
        task = Task.accessible_to(Current.user).find(args["id"])
        { task: Api::V1::TaskSerializer.new(task, detail: true).as_json }
      end
    end

    def create_task
      build(
        name: "create_task",
        description: "Create a task in the workspace.",
        scope: "tasks:write",
        enabled: -> { Features.tasks? },
        input_schema: object_schema(
          properties: {
            title: { type: "string" },
            description: { type: "string" },
            due_at: { type: "string", description: "ISO-8601" },
            all_day: { type: "boolean" },
            priority: { type: "string", enum: %w[low normal high urgent] },
            status: { type: "string", enum: %w[todo in_progress blocked], description: "Default: todo" }
          },
          required: %w[title]
        )
      ) do |args|
        ensure_entitled!(:tasks)
        require_arg(args, "title")
        status = Task.statuses.key?(args["status"].to_s) ? args["status"] : "todo"
        task = Current.workspace.tasks.new(
          title: args["title"], description: args["description"],
          due_at: parse_time(args["due_at"]), all_day: args["all_day"] ? true : false,
          priority: args["priority"].presence || "normal",
          status: status, created_by: Current.user
        )
        task.save!
        { task: Api::V1::TaskSerializer.new(task, detail: true).as_json }
      end
    end

    def update_task
      build(
        name: "update_task",
        description: "Update a task's fields. Status changes use the proper transition (publishes events).",
        scope: "tasks:write",
        enabled: -> { Features.tasks? },
        input_schema: object_schema(
          properties: {
            id: { type: "string" },
            title: { type: "string" },
            description: { type: "string" },
            due_at: { type: "string" },
            all_day: { type: "boolean" },
            priority: { type: "string" },
            status: { type: "string" }
          },
          required: %w[id]
        )
      ) do |args|
        ensure_entitled!(:tasks)
        require_arg(args, "id")
        task = Task.accessible_to(Current.user).find(args["id"])
        permitted = args.slice("title", "description", "due_at", "all_day", "priority")
        permitted["due_at"] = parse_time(args["due_at"]) if args.key?("due_at")
        task.update!(permitted.compact)
        if args["status"].present?
          unless Task.statuses.key?(args["status"])
            raise Mcp::ToolError, "Invalid status '#{args["status"]}'. Valid: #{Task.statuses.keys.join(", ")}."
          end
          task.move_to_status!(args["status"], by: Current.user) if args["status"] != task.status
        end
        { task: Api::V1::TaskSerializer.new(task.reload, detail: true).as_json }
      end
    end

    def complete_task
      build(
        name: "complete_task",
        description: "Mark a task as done.",
        scope: "tasks:write",
        enabled: -> { Features.tasks? },
        input_schema: id_schema("The task id")
      ) do |args|
        ensure_entitled!(:tasks)
        require_arg(args, "id")
        task = Task.accessible_to(Current.user).find(args["id"])
        task.move_to_status!(:done, by: Current.user)
        { task: Api::V1::TaskSerializer.new(task, detail: true).as_json }
      end
    end

    def create_task_from_email
      build(
        name: "create_task_from_email",
        description: "Extract and create a task from an email via the action registry.",
        scope: "tasks:write",
        enabled: -> { Features.tasks? },
        input_schema: object_schema(
          properties: {
            email_id: { type: "string" },
            title: { type: "string", description: "Override the extracted title" }
          },
          required: %w[email_id]
        )
      ) do |args|
        ensure_entitled!(:tasks)
        require_arg(args, "email_id")
        msg = EmailMessage.accessible_to(Current.user).find(args["email_id"])
        result = EmailActions.run("create_task_from_email", email_message: msg,
                                  args: { "title" => args["title"] }.compact, user: Current.user)
        raise Mcp::ToolError, (result[:message] || "Could not create task from email.") unless result[:success]

        result.slice(:success, :tool, :message, :result)
      end
    end

    # ---- calendar helpers ---------------------------------------------------

    def list_calendars
      build(
        name: "list_calendars",
        description: "List calendars visible to the caller. Use the id as calendar_id in create_calendar_event.",
        scope: "calendar:read",
        input_schema: object_schema(properties: {})
      ) do |_args|
        all_cals = Calendar.where(calendar_account: Current.user.readable_calendar_accounts)
                           .includes(:calendar_account)
        rows = all_cals.map do |cal|
          writable = cal.is_writable && cal.calendar_account.writable_by?(Current.user)
          {
            id: cal.id,
            name: cal.name,
            provider: cal.provider,
            primary: cal.is_primary,
            writable: writable,
            syncing: cal.syncing,
            color: cal.display_color
          }
        end
        { calendars: rows, count: rows.size }
      end
    end

    def create_event_from_email
      build(
        name: "create_event_from_email",
        description: "Extract and create a calendar event from an email. " \
                     "AI infers the event details; supply overrides to refine them.",
        scope: "calendar:write",
        input_schema: object_schema(
          properties: {
            email_id: { type: "string" },
            title: { type: "string" },
            start_time: { type: "string", description: "ISO-8601 override" },
            end_time: { type: "string", description: "ISO-8601 override" },
            calendar_id: { type: "string" }
          },
          required: %w[email_id]
        )
      ) do |args|
        require_arg(args, "email_id")
        msg = EmailMessage.accessible_to(Current.user).find(args["email_id"])
        tool_args = args.slice("title", "start_time", "end_time", "calendar_id").compact
        event = Tools::CreateCalendarEvent.call(msg, tool_args, user: Current.user)
        raise Mcp::ToolError, "Could not determine event details from this email." unless event

        { event: Api::V1::CalendarEventSerializer.new(event, detail: true).as_json }
      end
    end

    # ---- workflows (gated behind Features.workflows?) -----------------------

    def list_workflows
      build(
        name: "list_workflows",
        description: "List the workspace's automation workflows.",
        scope: "workflows:read",
        enabled: -> { Features.workflows? },
        input_schema: object_schema(properties: { limit: limit_property })
      ) do |args|
        scope = Current.workspace.workflows.order(created_at: :desc)
        rows = scope.limit(clamp_limit(args["limit"])).map { |w| Api::V1::WorkflowSerializer.new(w).as_json }
        { workflows: rows, count: rows.size }
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
            id: { type: "string" },
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
          properties: { workflow_id: { type: "string" }, limit: limit_property },
          required: [ "workflow_id" ]
        )
      ) do |args|
        require_arg(args, "workflow_id")
        workflow = Current.workspace.workflows.find(args["workflow_id"])
        rows = workflow.executions.limit(clamp_limit(args["limit"])).map { |e| Api::V1::WorkflowExecutionSerializer.new(e).as_json }
        { executions: rows, count: rows.size }
      end
    end

    # ---- scout --------------------------------------------------------------

    def list_scout_threads
      build(
        name: "list_scout_threads",
        description: "List the caller's Scout chat threads, newest first.",
        scope: "scout:read",
        input_schema: object_schema(properties: { limit: limit_property })
      ) do |args|
        scope = Current.user.agent_threads.scout_visible.recent
        rows = scope.limit(clamp_limit(args["limit"])).map { |t| Api::V1::AgentThreadSerializer.new(t).as_json }
        { threads: rows, count: rows.size }
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
            thread_id: { type: "string" },
            after_message_id: { type: "string", description: "Only messages created after this one" }
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
        rows = scope.map { |m| Api::V1::AgentMessageSerializer.new(m).as_json }
        { messages: rows, count: rows.size }
      end
    end

    def send_scout_message
      build(
        name: "send_scout_message",
        description: "Post a user message to a Scout thread. The AI reply is generated asynchronously; poll list_scout_messages(after_message_id) for it.",
        scope: "scout:write",
        input_schema: object_schema(
          properties: {
            thread_id: { type: "string" },
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

    # ---- scheduled emails ---------------------------------------------------

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
        rows = scope.limit(clamp_limit(args["limit"])).map { |s| Api::V1::ScheduledEmailSerializer.new(s).as_json }
        { scheduled_emails: rows, count: rows.size }
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
            email_account_id: { type: "string" },
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
            id: { type: "string" },
            email_account_id: { type: "string" },
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
        ensure_editable!(record)
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
        ensure_editable!(record)
        record.update!(status: :cancelled)
        { scheduled_email: Api::V1::ScheduledEmailSerializer.new(record, detail: true).as_json }
      end
    end

    # ---- calendar events ----------------------------------------------------

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
        rows = scope.limit(clamp_limit(args["limit"])).map { |e| Api::V1::CalendarEventSerializer.new(e).as_json }
        { events: rows, count: rows.size }
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
            calendar_id: { type: "string", description: "Id of a writable calendar" },
            title: { type: "string" },
            start_at: { type: "string", description: "ISO-8601 start" },
            end_at: { type: "string", description: "ISO-8601 end" },
            description: { type: "string" },
            location: { type: "string" },
            all_day: { type: "boolean" }
          },
          required: [ "calendar_id", "title", "start_at" ]
        )
      ) do |args|
        require_arg(args, "calendar_id", "title", "start_at")
        calendar = writable_calendars.find_by(id: args["calendar_id"])
        raise Mcp::ToolError, "That calendar does not exist or is not writable." unless calendar

        # An event has no color of its own — it renders in its calendar's color
        # (CalendarEvent#display_color), so the tool does not expose a color field.
        event = calendar.calendar_events.new(
          title: args["title"], description: args["description"], location: args["location"],
          start_at: args["start_at"], end_at: args["end_at"], all_day: args["all_day"] || false,
          provider_event_id: "local-#{SecureRandom.uuid}",
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
            id: { type: "string" },
            title: { type: "string" }, description: { type: "string" }, location: { type: "string" },
            start_at: { type: "string" }, end_at: { type: "string" },
            all_day: { type: "boolean" },
            recurrence_scope: { type: "string", enum: %w[this all] }
          },
          required: [ "id" ]
        )
      ) do |args|
        require_arg(args, "id")
        event = writable_event(args["id"])
        event.update!(args.slice("title", "description", "location", "start_at", "end_at", "all_day")
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
          properties: { id: { type: "string" }, recurrence_scope: { type: "string", enum: %w[this all] } },
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
            id: { type: "string" },
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

    # ---- reminders ----------------------------------------------------------

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
        rows = scope.limit(clamp_limit(args["limit"])).map { |r| Api::V1::ReminderSerializer.new(r).as_json }
        { reminders: rows, count: rows.size }
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
          properties: { id: { type: "string" }, due_at: { type: "string", description: "ISO-8601" } },
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
          properties: { id: { type: "string" }, until: { type: "string", description: "ISO-8601" } },
          required: [ "id" ]
        )
      ) do |args|
        require_arg(args, "id")
        reminder = Reminder.accessible_to(Current.user).find(args["id"])
        reminder.update!(status: :snoozed, snoozed_until: parse_time(args["until"]) || 1.week.from_now)
        { reminder: Api::V1::ReminderSerializer.new(reminder, detail: true).as_json }
      end
    end

    # ---- email templates ---------------------------------------------------

    def list_email_templates
      build(
        name: "list_email_templates",
        description: "List the workspace's reusable email templates.",
        scope: "templates:read",
        enabled: -> { Features.email_templates? },
        input_schema: object_schema(properties: { limit: limit_property })
      ) do |args|
        scope = Current.workspace.email_templates.recent
        rows = scope.limit(clamp_limit(args["limit"])).map { |t| Api::V1::EmailTemplateSerializer.new(t).as_json }
        { email_templates: rows, count: rows.size }
      end
    end

    # ---- folders ------------------------------------------------------------

    def list_folders
      build(
        name: "list_folders",
        description: "List the workspace's custom folders.",
        scope: "folders:read",
        input_schema: object_schema(properties: {})
      ) do |_args|
        rows = Current.workspace.mail_folders.ordered.map { |f| Api::V1::FolderSerializer.new(f).as_json }
        { folders: rows, count: rows.size }
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
          properties: { mail_folder_id: { type: "string" }, document_id: { type: "string" } },
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

    # ---- helpers ------------------------------------------------------------

    def build(name:, description:, scope:, input_schema:, enabled: -> { true }, &handler)
      Tool.new(name: name, description: description, scope: scope,
               input_schema: input_schema, handler: handler, enabled: enabled)
    end

    def object_schema(properties:, required: [])
      { type: "object", properties: properties, required: required }
    end

    def id_schema(description)
      object_schema(properties: { id: { type: "string", description: description } }, required: [ "id" ])
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

    # Mailbox readers can see a queued send (accessible_to) but only the
    # creator or someone with send access on the account may change it.
    def ensure_editable!(scheduled_email)
      return if scheduled_email.editable_by?(Current.user)

      raise Mcp::ToolError, "You can't modify that scheduled email."
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

    # True when the current request's credential carries the named scope.
    # Handlers use this to gate optional sections in get_overview / get_setup_status.
    def scope_granted?(name)
      Current.api_scopes.to_a.include?(name.to_s)
    end

    # Compact a skim cluster for MCP output — drop the heavy `emails` array,
    # keep only the fields an agent needs to propose a decision.
    def compact_cluster(card)
      {
        category: card[:category],
        title: card[:title],
        summary: card[:summary],
        count: card[:count],
        unread_count: card[:unread_count],
        bucket: card[:bucket],
        importance: card[:importance],
        priority_suggested: card[:priority_suggested],
        scout_suggestion: card[:scout_suggestion],
        follow_up: card[:follow_up],
        follow_up_reason: card[:follow_up_reason],
        latest_received_at: card[:latest_received_at]&.iso8601,
        email_ids: card[:email_ids],
        # The builder already loaded these — reuse its per-email detail, no refetch.
        samples: (card[:emails] || []).first(3).map { |e| e.slice(:id, :sender, :subject) }
      }
    end

    # Validate a refresh_token with the provider and return the resolved identity.
    # Raises Mcp::ToolError on permanent auth failure (wrong client, revoked token).
    def resolve_oauth_identity(provider, refresh_token)
      case provider.to_s
      when "zoho"
        access_token = Zoho::OauthClient.new(refresh_token: refresh_token).refresh!
        Zoho::AccountDiscovery.new(access_token).discover_identity ||
          raise(Mcp::ToolError, "Could not determine Zoho account identity from the token.")
      when "google"
        access_token = Google::OauthClient.new(refresh_token: refresh_token).refresh!
        Google::AccountDiscovery.new(access_token).discover_identity ||
          raise(Mcp::ToolError, "Could not determine Google account identity from the token.")
      else
        raise Mcp::ToolError, "Provider #{provider} does not support token mode."
      end
    rescue KeyError => e
      raise Mcp::ToolError, "#{provider.to_s.capitalize} OAuth is not configured on this server " \
                            "(missing #{e.message}). Token mode requires the server's own " \
                            "OAuth client credentials (#{provider.to_s.upcase}_CLIENT_ID / _SECRET)."
    rescue PermanentAuthError => e
      raise Mcp::ToolError, "Token refresh failed: #{e.message}. The refresh_token must be " \
                            "minted with THIS server's configured OAuth client credentials — " \
                            "tokens from a different client_id will not work."
    rescue AuthenticationError => e
      raise Mcp::ToolError, "Token validation failed (transient): #{e.message}. Try again."
    end

    # iso helper: format a time as ISO-8601 string, nil-safe.
    def iso(t)
      t&.iso8601
    end
  end
end
