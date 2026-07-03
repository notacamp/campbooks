class EmailComposeController < ApplicationController
  before_action :require_authentication
  before_action :load_message

  MODES = %w[reply reply_all forward new_message].freeze

  def create
    mode = params[:mode].to_s
    unless MODES.include?(mode)
      render json: { error: t(".invalid_mode") }, status: :unprocessable_entity and return
    end

    @mode = mode
    prefill = Emails::ComposePrefill.for(message: @message, mode: mode)
    @to_address = prefill.to
    @cc_address = prefill.cc
    @subject = prefill.subject
    @quoted_body = prefill.quoted_body
    # When opened from a Scout draft ("Edit in composer") or a mode switch, the
    # carried body — which may already hold the signature — pre-fills the
    # editor. Skip the default signature so it isn't appended twice.
    @prefill_body = params[:prefill_body].presence || params[:body].presence
    @signature = @prefill_body.present? ? nil : Signature.default_for(Current.user, @message.email_account)
    @signatures = Current.user.signatures.ordered.includes(:email_accounts)
    # A mode switch carries the working draft along so autosave keeps updating
    # the same row instead of forking a second one.
    @draft = Current.user.draft_emails.find_by(id: params[:draft_email_id]) if params[:draft_email_id].present?
    # Decided behavior: Scout pre-drafts only when a suggestion is already
    # cached on the thread (never a surprise model call on open).
    @scout_draft = cached_scout_draft if %w[reply reply_all].include?(mode) && @prefill_body.blank?

    respond_to do |format|
      format.turbo_stream { render turbo_stream: dock_stream }
      format.html { redirect_to email_message_path(@message) }
    end
  end

  def send_message
    if params[:send_action] == "schedule" && current_entitlements.feature?(:email_scheduling)
      return create_scheduled_email
    end

    args = compose_params

    # Append the selected signature (with separation; never doubled). See
    # Signature.append_to_body — the one place signatures get attached.
    if params[:signature_id].present?
      sig = Current.user.signatures.find_by(id: params[:signature_id])
      args[:body] = Signature.append_to_body(args[:body], sig)
    end

    # Provider dispatch, threading, and sent-record bookkeeping live in
    # Emails::Sender (shared with the public API).
    result = Emails::Sender.call(
      user: Current.user,
      email_account_id: params[:email_account_id],
      to_address: args[:to_address],
      subject: args[:subject],
      body: args[:body],
      cc_address: args[:cc_address],
      bcc_address: args[:bcc_address],
      source_message: @message,
      attachments: collected_attachments,
      attachment_signed_ids: owned_attachment_ids
    )

    return error_response(error_message_for(result)) unless result.ok?

    consume_draft

    if @message
      remove_compose_area
    else
      sent_redirect
    end
  end

  def discard
    remove_compose_area
  end

  private

  def create_scheduled_email
    args = compose_params
    template = scheduled_email_template

    # Bake in the selected signature exactly like an immediate send, so the
    # queued message goes out identical to what "Send now" would have produced.
    # With a template, schedule its raw subject/body + the picked variables so
    # each occurrence re-renders fresh (recurring {{ date }} stays current) and
    # the send job regenerates the document-template PDFs.
    body = template&.body_html.presence || args[:body]
    if params[:signature_id].present?
      sig = Current.user.signatures.find_by(id: params[:signature_id])
      body = Signature.append_to_body(body, sig)
    end

    scheduled = ScheduledEmail.new(
      workspace: Current.workspace,
      email_account_id: params[:email_account_id] || @message&.email_account_id,
      created_by: Current.user,
      email_template: template,
      to_address: args[:to_address],
      subject: template&.subject.presence || args[:subject],
      body: body,
      cc_address: args[:cc_address],
      bcc_address: args[:bcc_address],
      template_context: template ? scheduled_template_context : {},
      scheduled_at: params[:scheduled_at].presence || 1.hour.from_now,
      rrule: params[:rrule].presence
    )

    return error_response(t(".schedule_failed")) unless scheduled.save

    consume_draft

    next_at = scheduled.rrule.present? ? ScheduleCalculator.next_occurrence(scheduled.scheduled_at, scheduled.rrule) : scheduled.scheduled_at
    scheduled.update_columns(next_occurrence_at: next_at)

    toast = t(".scheduled", time: l(scheduled.display_time, format: :short))
    respond_to do |format|
      format.turbo_stream do
        streams = [ notify_stream(toast, severity: :success) ]
        streams << turbo_stream.remove("compose_area_#{@message.id}") if @message
        render turbo_stream: streams
      end
      format.html { redirect_to scheduled_emails_path, notice: toast }
    end
  end

  # The email template a scheduled send was built from (set by the composer's
  # template picker), scoped to the workspace so a forged id can't be linked.
  def scheduled_email_template
    return nil unless current_entitlements.feature?(:email_templates)
    return nil if params[:email_template_id].blank?

    Current.workspace.email_templates.find_by(id: params[:email_template_id])
  end

  # The variable values the picker stashed (JSON), used to re-render the template
  # on every occurrence. Tolerant of malformed input.
  def scheduled_template_context
    raw = params[:template_context]
    return {} if raw.blank?

    parsed = JSON.parse(raw)
    parsed.is_a?(Hash) ? parsed : {}
  rescue JSON::ParserError
    {}
  end

  # Every compose entry — reply buttons, r/a/f shortcuts, the drawer, Scout,
  # mode switches — opens the same Dock (bottom sheet) via the layout's
  # #compose_dock slot.
  def dock_stream
    streams = []
    # "Edit in composer" from a Scout draft card passes the card's dom id to clean up.
    streams << turbo_stream.remove(params[:remove_draft]) if params[:remove_draft].present?
    streams << turbo_stream.update("compose_dock", partial: "email_compose/dock", locals: {
      mode: @mode.to_sym,
      message: @message,
      draft: @draft,
      to: @to_address.to_s,
      cc: @cc_address.to_s,
      bcc: "",
      subject: @subject.to_s,
      body: @prefill_body.to_s,
      quoted_body: @quoted_body.to_s,
      signatures: @signatures,
      signature_id: @signature&.id,
      accounts: [],
      attachment_entries: forward_attachment_entries,
      scout_draft: @scout_draft
    })
    streams
  end

  # The freshest non-outdated Scout draft on the thread (created by the
  # suggest-reply tools). Question prompts also live as draft messages but
  # carry ai_suggested_actions — excluded so they never ghost into the canvas.
  def cached_scout_draft
    agent_thread = @message.email_thread&.agent_thread
    agent_thread&.agent_messages
                &.where(draft: true, outdated: false, author_type: :ai)
                &.where("ai_suggested_actions = '[]'::jsonb") # a hash [] compiles to an empty IN, not jsonb equality
                &.order(created_at: :desc)&.first&.content
  end

  # Forwarding carries the original files along as removable chips (their
  # signed ids resolve at send; ownership check accepts the source message's
  # own blobs — see collected_attachments).
  def forward_attachment_entries
    return [] unless @mode == "forward"

    Emails::ComposePrefill.forward_attachment_entries(@message)
  end

  # A successful send (or schedule) consumes the autosaved draft it was written
  # in, so the pill never resurrects an email that already went out.
  def consume_draft
    return if params[:draft_email_id].blank?

    Current.user.draft_emails.find_by(id: params[:draft_email_id])&.destroy
  end

  # The message's own account — but only when the user may send from it. Read
  # access to a shared inbox (can_read) must not let someone reply/draft from it.
  def sendable_message_account
    account = @message&.email_account
    account if account&.sendable_by?(Current.user)
  end

  # Map Emails::Sender failure codes to the composer's localized error toasts.
  def error_message_for(result)
    key = case result.error_code
    when "no_sendable_account" then ".no_account"
    when "recipient_required" then ".recipient_required"
    else ".send_failed"
    end
    t(key)
  end

  def sent_redirect
    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [
          notify_stream(t(".sent_toast"), severity: :success),
          turbo_stream.update("redirect_script") {
            "<script>setTimeout(function(){ window.location.href = '#{email_messages_path}' }, 1500)</script>".html_safe
          }
        ]
      end
      format.html { redirect_to email_messages_path, success: t(".sent") }
    end
  end

  def load_message
    return if params[:email_account_id].present? && action_name == "send_message"

    @message = Current.user.readable_email_accounts
                        .flat_map { |a| a.email_messages.find_by(id: params[:id]) }
                        .compact.first
    unless @message
      @message = EmailMessage.where(email_account: Current.user.readable_email_accounts)
                              .find(params[:id])
    end
  end

  def compose_params
    {
      subject: params[:subject],
      body: merged_body,
      to_address: params[:to_address],
      cc_address: params[:cc_address].presence,
      bcc_address: params[:bcc_address].presence
    }
  end

  # The new composer keeps the quoted thread OUT of the editor (a collapsed
  # pill + hidden quoted_body field) unless the user expands it. Sends merge it
  # back in, preserving the classic body-then-quote order; the legacy inline
  # form simply never posts quoted_body.
  def merged_body
    [ params[:body], params[:quoted_body] ].select(&:present?).join
  end

  # Resolve the signed blob ids submitted by the attachment tray into the
  # canonical { filename:, content_type:, data: } shape the mail clients consume.
  # Scoped to the user's own uploads so a stray/forged signed id can't attach
  # someone else's blob.
  def collected_attachments
    ids = Array(params[:attachments]).reject(&:blank?)
    return [] if ids.empty?

    allowed = allowed_blob_ids
    ids.filter_map do |signed_id|
      blob = ActiveStorage::Blob.find_signed(signed_id)
      next unless blob && allowed.include?(blob.id)
      { filename: blob.filename.to_s, content_type: blob.content_type, data: blob.download }
    end
  rescue => e
    Rails.logger.error("[EmailCompose] attachment resolve failed: #{e.message}")
    []
  end


  def owned_attachment_ids
    ids = Array(params[:attachments]).reject(&:blank?)
    return [] if ids.empty?
    allowed = allowed_blob_ids
    ids.select do |signed_id|
      blob = ActiveStorage::Blob.find_signed(signed_id)
      blob and allowed.include?(blob.id)
    end
  rescue => e
    Rails.logger.error("[EmailCompose] attachment id resolution failed: " + e.message.to_s)
    []
  end

  # A submitted signed id must be the user's own upload — or, when replying on
  # a thread, one of the source message's own files (that's how a forward
  # carries the originals). @message is permission-checked in load_message, so
  # this never widens access beyond mail the user can already read.
  def allowed_blob_ids
    allowed = Current.user.outbound_attachments.blobs.pluck(:id).to_set
    allowed.merge(@message.files.blobs.pluck(:id)) if @message&.files&.attached?
    allowed
  end

  def extract_draft_id(result)
    return nil unless result
    if result.is_a?(Hash)
      result["messageId"] || result["id"]
    elsif result.is_a?(Array) && result.first.is_a?(Hash)
      result.first["messageId"] || result.first["id"]
    end
  end

  def create_provider_draft(body:)
    return nil unless sendable_message_account

    mail_client = @message.email_account.mail_client
    result = mail_client.save_draft(
      subject: @subject,
      body: body,
      to_address: @to_address.presence,
      cc_address: @cc_address.presence,
      in_reply_to_message_id: @message.provider_message_id
    )
    extract_draft_id(result)
  rescue => e
    Rails.logger.error("[EmailCompose] Failed to create initial draft: #{e.message}")
    nil
  end

  def error_response(message)
    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: notify_stream(message, severity: :error)
      end
      format.html { redirect_to email_message_path(@message), error: message }
    end
  end

  # After a successful thread send: drop the Dock and confirm. (Also clears any
  # legacy inline compose area still in the DOM.)
  def remove_compose_area
    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [
          turbo_stream.update("compose_dock", ""),
          turbo_stream.remove("compose_area_#{@message.id}"),
          notify_stream(t(".sent_toast"), severity: :success)
        ]
      end
      format.html { redirect_to email_message_path(@message), success: t(".sent") }
    end
  end

end
