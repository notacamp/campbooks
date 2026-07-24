class EmailProcessJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  def perform(email_message_id)
    email = EmailMessage.find(email_message_id)

    # AI model resolution (Ai::Configuration.for) reads Current.workspace. Jobs run
    # outside a request, so it must be set explicitly here — otherwise triage's
    # cheap-model rung and Ai::EmailClassifier resolve no adapter and fall back to a
    # keyless Anthropic client, which 401s and is swallowed: categories get set but
    # NO tags ever do. Every other AI job sets this; reset in `ensure` below.
    Current.workspace = email.email_account.workspace

    was_already_processed = nil

    email.with_lock do
      return if email.ignored?
      return if email.processed? && (!email.has_attachment? || email.files.attached?)

      was_already_processed = email.processed?
      email.processing! unless was_already_processed
    end

    mail_client = email.email_account.mail_client

    if email.provider_folder_id.present? && email.body.blank?
      body = sanitize_string(mail_client.get_message_content(email.provider_message_id, email.provider_folder_id))
      if body.present?
        email.update!(body: body)
      end
    end

    if email.body.present? && email.provider_folder_id.present?
      process_inline_images(email, mail_client)
    end

    if email.body.present?
      process_base64_images(email)
    end

    if email.body.present?
      new_body = rewrite_relative_urls(email.body, email.email_account_id)
      email.update!(body: new_body) if new_body != email.body
    end

    # Our OWN outbound mail (digests, notifications, transactional) is delivered to
    # the user's mailbox and so gets re-ingested here. Keep it readable — the body is
    # fetched above and it stays a visible inbox row below — but run NONE of the AI
    # pipeline on it: extracting reminders/tasks/contacts from a digest that itself
    # lists the user's reminders just manufactures duplicates, and firing workflow /
    # event triggers on our own mail is wrong. Flagged at ingest by MessageUpserter.
    if email.self_generated?
      finalize_without_analysis(email, was_already_processed)
      return
    end

    if email.has_attachment? && email.files.blank? && email.provider_folder_id.present?
      process_attachments(email, mail_client)
    end

    # Ingest direct document links from the email body (issue #56).
    # Skipped on re-process to avoid redundant fetches; re-runs are
    # idempotent (content-hash dedup) regardless.
    Emails::DocumentLinkIngester.new(email).call unless was_already_processed

    # Until the workspace has set up a text provider, ingest the message but run
    # NO analysis — triage, classification, contact analysis, and reminder
    # extraction are all skipped. Uses the STRICT gate (Ai::ProviderSetup.configured?)
    # so a freshly-synced inbox isn't auto-analysed on the shared platform keys
    # before the user opts into AI. (Interactive AI still works via #available?.)
    text_ai_available = Ai::ProviderSetup.configured?(email.email_account.workspace, :text)

    if text_ai_available && email.tags.empty?
      begin
        decision = Emails::Triage.new(email).call
        email.update!(category: decision.category, category_confidence: decision.confidence)
        email.email_message_tags.find_or_create_by!(tag: decision.tag) if decision.tag
        # Only the slice the cheap rungs couldn't resolve hits the LLM (and its
        # security pre-screen). Most of the noise firehose stops here.
        Ai::EmailClassifier.new(email).classify! if decision.needs_llm?
      rescue => e
        Rails.logger.error("[EmailProcessJob] Triage failed for email #{email.id}, falling back to classifier: #{e.message}")
        Ai::EmailClassifier.new(email).classify!
      end
    end

    # Bridge the rules-engine category onto a tag: mail sorted into a noise bucket
    # (notifications/promotions/social/updates) gets the workspace's matching
    # default group tag, so the inbox collapses it. Additive and idempotent — runs
    # even when the message already carries other tags, and reads the persisted
    # category on a re-process. Independent of the triage block above.
    apply_bucket_tag(email)

    thread = find_or_create_thread(email)

    was_new_thread = !email.email_thread_id
    email.update!(email_thread: thread)

    # Keep the thread's denormalized reply timestamps current so "has the owner
    # replied / do they hold the last word" stays a cheap column check downstream.
    # A reply from the other party retires any pending follow-up (pure data).
    update_thread_reply_times(thread, email)
    Emails::FollowUpClearer.call(thread) unless is_outbound?(email)

    if thread.agent_thread && !was_new_thread && thread.agent_thread.agent_messages.any?
      if is_outbound?(email)
        owner_user = email.email_account.email_account_users.find_by(owner: true)&.user
        thread.agent_thread.agent_messages.where(draft: true).update_all(outdated: true)
        I18n.with_locale(owner_user&.locale.presence || I18n.default_locale) do
          thread.agent_thread.agent_messages.create!(
            content: I18n.t("jobs.email_process.auto_replied_notice"),
            author_type: :ai,
            user: owner_user
          )
        end
        Turbo::StreamsChannel.broadcast_append_to(
          thread,
          target: "comments_list",
          partial: "email_comments/comment",
          locals: { comment: thread.agent_thread.agent_messages.last, email_message: thread.latest_message }
        )
      end
    end

    Contacts::Identifier.new(email).identify!

    # Profile the sender when they're someone worth profiling — a real person or
    # vendor — instead of after an arbitrary email count. Contacts::AnalysisGate is
    # a cheap, LLM-free deny-list over the triage category + bulk/automated headers
    # already resolved above; needs_analysis? keeps it to never-profiled (or
    # 30-day-stale) contacts so an active sender doesn't re-enqueue a no-op job per
    # message. (The count threshold still gates the backlog catch-up — see
    # Contacts::PendingAnalysisCatchUp — until that's type-gated too.)
    contact = email.contact
    if text_ai_available && contact&.needs_analysis? && Contacts::AnalysisGate.analyze?(email)
      ContactAnalysisJob.perform_later(contact.id)
    end

    apply_sender_rules(email)
    apply_email_rules(email) unless was_already_processed

    email.processed! unless was_already_processed

    # Live inbox: a freshly-ingested message floats its thread to the top of every
    # reader's open inbox (and refreshes the row in place where it's already shown).
    # First ingest only, so a full resync re-walking old mail doesn't re-surface it;
    # the broadcaster's inbox-folder gate keeps sent-only threads out of the list.
    Emails::InboxBroadcaster.upsert(thread) if thread && !was_already_processed

    WorkflowTriggerJob.perform_later(email.id) if Features.workflows? && !was_already_processed

    # Generic domain event (system-originated). Coexists with the dedicated
    # email_received trigger above; lets workflows/activity react to inbound mail
    # via the generic event bus. Workspace is explicit (jobs don't set Current).
    unless was_already_processed
      Events.publish(
        "email.received",
        subject: email,
        actor: nil,
        workspace: email.email_account.workspace,
        payload: {
          "subject" => email.subject.to_s,
          "from" => email.from_address.to_s,
          "to" => email.to_address.to_s,
          "account_email" => email.email_account&.email_address.to_s
        }
      )
    end

    # Best-effort: extract calendar-worthy reminders from this email (gated to skip
    # bulk/dateless mail). Runs once, mirroring the WorkflowTriggerJob guard.
    # Needs a text model, so it's skipped when no AI provider is configured.
    Reminders::EmailExtractionJob.perform_later(email.id) if text_ai_available && !was_already_processed

    # Best-effort: extract action items (tasks) the reader must do. Gated by the
    # Tasks readiness flag here; the job re-checks the workspace's :tasks entitlement.
    # Staggered 2 minutes behind the reminders job so the novelty gate in the tasks
    # builder can see reminder rows the same email just staged (the two jobs share the
    # ai_email_extraction concurrency key and otherwise race).
    Tasks::EmailExtractionJob.set(wait: 2.minutes).perform_later(email.id) if Features.tasks? && text_ai_available && !was_already_processed

    # An outbound reply may deserve a follow-up if the other party goes quiet — let
    # the AI decide whether and when. Outbound-only, once per first ingest (so a full
    # resync re-processing old sent mail doesn't re-analyse it).
    if text_ai_available && !was_already_processed && is_outbound?(email) && email.email_thread_id
      Emails::FollowUpAnalysisJob.perform_later(email.email_thread_id, email.id)
    end
  rescue => e
    email.update_columns(status: EmailMessage.statuses[:failed], updated_at: Time.current) unless was_already_processed
    Rails.logger.error("[EmailProcessJob] Error processing email #{email.id}: #{e.message}")
    raise
  ensure
    Current.workspace = nil
  end

  private

  # A readable, threaded inbox row for self-generated mail (so the user can read the
  # digest) without any AI analysis, tagging, contact profiling, or workflow / event
  # triggers. Mirrors only the minimal "make it exist and stay visible" slice of the
  # normal path; every extractor and trigger in #perform is deliberately skipped.
  def finalize_without_analysis(email, was_already_processed)
    thread = find_or_create_thread(email)
    email.update!(email_thread: thread)
    email.processed! unless was_already_processed
    Emails::InboxBroadcaster.upsert(thread) if thread && !was_already_processed
  end

  # Attach the workspace's default group tag for this email's rules category, so
  # low-priority mail collapses into its inbox group. Tolerant of failure so it
  # never fails the ingest (Tags::DefaultGroups.tag_email! self-heals provisioning
  # and no-ops for personal/important/unknown/nil categories).
  def apply_bucket_tag(email)
    Tags::DefaultGroups.tag_email!(email)
  rescue => e
    Rails.logger.error("[EmailProcessJob] bucket tag failed for email #{email.id}: #{e.message}")
  end

  # Workspace-level inbox rules, applied once on first ingest (never on reprocess).
  # Tolerant of failure: a raising rule must never fail ingestion.
  def apply_email_rules(email)
    EmailRules::Applier.new(email).call
  rescue => e
    Rails.logger.error("[EmailProcessJob] email rules failed for email #{email.id}: #{e.message}")
  end

  # Per-sender rules, applied once the Contact is resolved. Tolerant of failure so
  # a rule never fails the whole ingest.
  def apply_sender_rules(email)
    contact = email.contact
    return unless contact

    # Inherit the sender's characteristic tags so they propagate to new mail and
    # help grouping/display downstream (sender auto-tagging fills these in).
    contact.sender_tags.each do |tag|
      email.email_message_tags.find_or_create_by!(tag: tag)
    end

    # Blocked senders: auto-archive so the mail leaves the inbox folder and thus
    # drops out of Skim and the feed. Whitelist "pending" senders need nothing
    # here — their mail stays in the inbox and surfaces only in Skim's Pending
    # bucket, awaiting an allow/deny decision.
    Tools::Archive.call(email) if contact.blocked?
  rescue => e
    Rails.logger.error("[EmailProcessJob] sender rules failed for email #{email.id}: #{e.message}")
  end

  EXCLUDED_URL_PREFIXES = %r{email_images/|rails/}

  def find_or_create_thread(email)
    Emails::Threading.find_or_create(email)
  end

  def is_outbound?(email)
    email.outbound?
  end

  # Bump the thread's last_outbound_at / last_inbound_at to this message's time.
  # Atomic GREATEST so concurrent processing / out-of-order sync can only move the
  # watermark forward. update_column-style (no callbacks, no updated_at touch).
  def update_thread_reply_times(thread, email)
    ts = email.received_at
    return if ts.nil?

    column = is_outbound?(email) ? "last_outbound_at" : "last_inbound_at"
    EmailThread.where(id: thread.id).update_all(
      [ "#{column} = GREATEST(COALESCE(#{column}, ?), ?)", ts, ts ]
    )
  end

  def rewrite_relative_urls(body, account_id)
    body.gsub(/src=["']\/(?!#{EXCLUDED_URL_PREFIXES})([^"']+)["']/) do
      "src=\"/email_images/#{account_id}/#{$1}\""
    end.gsub(/url\(["']?\/(?!#{EXCLUDED_URL_PREFIXES})([^"')]+)["']?\)/) do
      "url(/email_images/#{account_id}/#{$1})"
    end
  end

  def process_inline_images(email, mail_client)
    urls = email.body.scan(%r{src=["'](/mail/ImageDisplay\?[^"']+)["']}).flatten
    return if urls.empty?

    new_body = email.body.dup
    changed = false

    urls.each do |url|
      query = (url.split("?", 2).last || "").gsub("&amp;", "&")
      params = {}
      query.split("&").each do |pair|
        key, value = pair.split("=", 2)
        next unless key && value && key.exclude?("amp;")
        params[key] = CGI.unescape(value)
      end

      cid = params["cid"]
      filename = params["f"]
      next unless cid.present?

      begin
        raw = mail_client.download_inline_image(email.provider_message_id, email.provider_folder_id, cid)
        next if raw.nil? || raw.empty?

        content_type = mime_type_for(filename) || "image/png"
        email.files.attach(
          io: StringIO.new(raw),
          filename: filename || "inline_image",
          content_type: content_type
        )
        blob = email.files_attachments.last&.blob
        next unless blob

        new_body.gsub!(url, blob_path(blob))
        new_body.gsub!(url.gsub("&amp;", "&"), blob_path(blob))
        changed = true
      rescue => e
        Rails.logger.warn("[EmailProcessJob] Inline image download failed for email #{email.id}: #{e.message}")
      end
    end

    email.update!(body: new_body) if changed
  end

  def process_base64_images(email)
    pattern = /src=["']data:(image\/[^;]+);base64,([^"']+)["']/
    matches = email.body.scan(pattern)
    return if matches.empty?

    new_body = email.body.dup
    seen = Set.new
    changed = false

    matches.each do |(mime_type, b64_data)|
      next if seen.include?(b64_data)
      seen.add(b64_data)

      begin
        raw = Base64.decode64(b64_data)
        next if raw.blank?

        ext = mime_type.split("/").last.split("+").first
        filename = "inline_image.#{ext}"

        email.files.attach(
          io: StringIO.new(raw),
          filename: filename,
          content_type: mime_type
        )
        blob = email.files_attachments.last&.blob
        next unless blob

        full_uri = "data:#{mime_type};base64,#{b64_data}"
        new_body.gsub!(full_uri, blob_path(blob))
        changed = true
      rescue => e
        Rails.logger.warn("[EmailProcessJob] Base64 image extraction failed for email #{email.id}: #{e.message}")
      end
    end

    email.update!(body: new_body) if changed
  end

  def process_attachments(email, mail_client)
    Emails::AttachmentIngester.call(email, mail_client)
  end

  def blob_path(blob)
    "/rails/active_storage/blobs/#{blob.signed_id}/#{blob.filename}"
  end

  def mime_type_for(filename)
    case File.extname(filename.to_s).downcase
    when ".pdf" then "application/pdf"
    when ".png" then "image/png"
    when ".jpg", ".jpeg" then "image/jpeg"
    when ".gif" then "image/gif"
    when ".doc" then "application/msword"
    when ".docx" then "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    when ".xls" then "application/vnd.ms-excel"
    when ".xlsx" then "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    when ".zip" then "application/zip"
    else "application/octet-stream"
    end
  end
end
