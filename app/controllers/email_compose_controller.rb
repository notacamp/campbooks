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
    @to_address = build_to_address(mode)
    @cc_address = build_cc_address(mode)
    @subject = build_subject(mode)
    @quoted_body = build_quoted_body(mode)
    # When opened from a Scout draft ("Edit in composer"), the draft body — which
    # already carries the signature — pre-fills the editor. Skip the default
    # signature so it isn't appended twice.
    @prefill_body = params[:prefill_body].presence || params[:body].presence
    @signature = @prefill_body.present? ? nil : Signature.default_for(Current.user, @message.email_account)
    @signatures = Current.user.signatures.ordered.includes(:email_accounts)

    respond_to do |format|
      format.turbo_stream { render turbo_stream: compose_area_stream }
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

  # The composer goes to a different target per surface. The bottom-right drawer
  # replaces its OWN slot (a distinct id, because the full page's
  # thread_compose_target is also in the DOM in List/Board layout); the full inbox
  # prepends above the thread as before.
  def compose_area_stream
    locals = {
      email_message: @message,
      mode: @mode.to_sym,
      to_address: @to_address,
      cc_address: @cc_address,
      subject: @subject,
      quoted_body: @quoted_body,
      prefill_body: @prefill_body,
      signature_content: @signature&.content,
      current_signature_id: @signature&.id,
      signatures: @signatures
    }

    streams = []
    # "Edit in composer" from a draft that's a sibling of the compose slot
    # (detail / discussion) passes the card's dom id to clean up.
    streams << turbo_stream.remove(params[:remove_draft]) if params[:remove_draft].present?
    streams << if params[:compose_target] == "drawer_compose_target"
      turbo_stream.update("drawer_compose_target", partial: "email_compose/compose_area", locals: locals)
    else
      turbo_stream.prepend("thread_compose_target", partial: "email_compose/compose_area", locals: locals)
    end
    streams
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

    owned_blob_ids = Current.user.outbound_attachments.blobs.pluck(:id).to_set
    ids.filter_map do |signed_id|
      blob = ActiveStorage::Blob.find_signed(signed_id)
      next unless blob && owned_blob_ids.include?(blob.id)
      { filename: blob.filename.to_s, content_type: blob.content_type, data: blob.download }
    end
  rescue => e
    Rails.logger.error("[EmailCompose] attachment resolve failed: #{e.message}")
    []
  end


  def owned_attachment_ids
    ids = Array(params[:attachments]).reject(&:blank?)
    return [] if ids.empty?
    owned_blob_ids = Current.user.outbound_attachments.blobs.pluck(:id).to_set
    ids.select do |signed_id|
      blob = ActiveStorage::Blob.find_signed(signed_id)
      blob and owned_blob_ids.include?(blob.id)
    end
  rescue => e
    Rails.logger.error("[EmailCompose] attachment id resolution failed: " + e.message.to_s)
    []
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

  def remove_compose_area
    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: turbo_stream.remove("compose_area_#{@message.id}")
      end
      format.html { redirect_to email_message_path(@message), success: t(".sent") }
    end
  end

  def build_to_address(mode)
    case mode
    when "reply" then @message.from_address.to_s
    when "reply_all"
      recipients = []
      recipients << @message.from_address.to_s if @message.from_address.present?
      recipients.concat(parse_addresses(@message.to_address))
      recipients.reject! { |a| own_address?(a) }
      recipients.uniq.join(", ")
    when "forward", "new_message" then ""
    end
  end

  def build_cc_address(mode)
    case mode
    when "reply_all"
      cc_list = parse_addresses(@message.cc_address || "")
      cc_list.reject! { |a| own_address?(a) }
      cc_list.uniq.join(", ")
    when "reply", "forward", "new_message" then ""
    end
  end

  def build_subject(mode)
    case mode
    when "new_message" then ""
    when "forward"
      subject = @message.subject.to_s
      subject.match?(/^Fwd:\s*/i) ? subject : "Fwd: #{subject}"
    else
      subject = @message.subject.to_s
      subject.match?(/^Re:\s*/i) ? subject : "Re: #{subject}"
    end
  end

  def build_quoted_body(mode)
    return "" if mode == "new_message"

    from = @message.from_address || "Unknown"
    date = @message.received_at&.strftime("%b %d, %Y at %H:%M") || "Unknown date"
    body_html = @message.body.presence || @message.summary.presence || "(no content)"

    if mode == "forward"
      <<~HTML
        <br><br>
        <p style="font-size: 12px; color: #9ca3af;">
          ---------- Forwarded message ----------<br>
          <b>From:</b> #{ERB::Util.html_escape(from)}<br>
          <b>Date:</b> #{date}<br>
          <b>Subject:</b> #{ERB::Util.html_escape(@message.subject || "")}<br>
          <b>To:</b> #{ERB::Util.html_escape(@message.to_address || "")}
        </p>
        <br>
        #{body_html}
      HTML
    else
      <<~HTML
        <br><br>
        <blockquote style="border-left: 2px solid #d1d5db; padding-left: 8px; margin-left: 0; color: #6b7280;">
          <p style="font-size: 12px; color: #9ca3af;">
            On #{date}, #{ERB::Util.html_escape(from)} wrote:
          </p>
          #{body_html}
        </blockquote>
      HTML
    end
  end

  def parse_addresses(str)
    return [] if str.blank?
    str.split(",").map(&:strip).select(&:present?)
  end

  # An address belongs to the sending account when its email part matches the
  # account address. Compare the bare email so a "Display Name <addr>" form still
  # gets dropped from reply-all — otherwise the user ends up emailing themselves.
  def own_address?(addr)
    bare_email(addr) == bare_email(@message.email_account.email_address)
  end

  def bare_email(addr)
    str = addr.to_s
    (str[/<([^>]+)>/, 1] || str).strip.downcase
  end
end
