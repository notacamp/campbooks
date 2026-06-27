class DocumentsController < ApplicationController
  before_action :set_document, only: [ :show, :file, :rename, :update, :approve, :reject, :toggle_star, :reprocess, :push_to_notion, :push_to_drive, :push_to_zoho_drive ]

  def index
    @document_types = Current.workspace.document_types.order(:name)
    @categories = DocumentType::CATEGORIES
    @mail_folders = Current.workspace.mail_folders.ordered

    documents = Current.workspace.documents.includes(:classification).with_attached_original_file.starred_first.recent
    documents = documents.by_type(params[:type]) if params[:type].present?
    documents = documents.by_category(params[:category]) if params[:category].present?
    documents = documents.by_review_status(params[:review_status])
    documents = documents.by_ai_status(params[:ai_status])
    documents = documents.in_folder(params[:folder_id]) if params[:folder_id].present?

    if params[:year].present? && params[:month].present?
      documents = documents.for_month(params[:year].to_i, params[:month].to_i)
    end

    @reprocessable_count = documents.rewhere(review_status: :pending, ai_status: [ :pending, :completed, :failed ]).count
    @exports = Current.workspace.exports.recent.limit(10)

    # Tell a brand-new/empty workspace (→ onboarding empty state with CTAs) apart from
    # a filter that simply matched nothing (→ "no matches" row), so the zero-state
    # points the right way. The setup hub itself now lives on home only.
    @has_any_documents = Current.workspace.documents.exists?
    @email_connected = Current.workspace.email_accounts.active.exists?

    @pagy, @documents = pagy(documents)

    respond_to do |format|
      format.html
      format.turbo_stream
    end
  end

  def create
    files = Array(params[:files])

    if files.empty?
      redirect_to documents_path, error: t(".no_files")
      return
    end

    documents = files.map do |file|
      document = Document.new(source: :manual_upload, ai_status: :pending, review_status: :pending, workspace: Current.workspace)
      document.original_file.attach(file)
      document.save!
      DocumentProcessJob.perform_later(document.id)
      document
    end

    notify_all_users(documents)

    if documents.size == 1
      redirect_to document_path(documents.first), success: t(".uploaded_one")
    else
      redirect_to documents_path, success: t(".uploaded_many", count: documents.size)
    end
  rescue => e
    redirect_to documents_path, error: t(".upload_failed", message: e.message)
  end

  def show
    AuditEvent.log("document_read", user: Current.user, request: request, target: @document, via: "web")
    @document_types = Current.workspace.document_types.order(:category, :name)
    @drive_account = Current.workspace.google_drive_accounts.connected.first
    @notion_connected = Current.workspace.notion_integrations.active.exists?
  end

  def file
    blob = if params[:type] == "processed" && @document.processed_pdf.attached?
      @document.processed_pdf.blob
    else
      @document.original_file.blob
    end

    if blob.nil?
      redirect_to @document, error: t(".not_available")
      return
    end

    # Serve inline by default (so it previews in an <img>/<iframe>); force a
    # download when the caller asks for it (the Skim card's Download control).
    url = if params[:disposition] == "attachment"
      rails_storage_proxy_url(blob, host: request.base_url, disposition: "attachment")
    else
      rails_storage_proxy_url(blob, host: request.base_url)
    end
    redirect_to url, allow_other_host: true
  end

  # Inline rename from the documents list. Sets the display name (metadata["title"]);
  # blank clears it back to the entity/filename. Returns JSON for the
  # document-rename Stimulus controller's fire-and-forget save.
  def rename
    @document.assign_title(params.dig(:document, :title))
    @document.save!
    render json: { ok: true, display_title: @document.display_title }
  end

  def update
    was_review = @document.review_pending?
    reclassified = reclassifying?(document_params)

    if @document.update(document_params)
      @document.generate_canonical_filename!
      if was_review && reclassified
        # Re-filing a document under review is itself the sign-off — approve it in
        # the same step so it leaves the review queue (mirrors Skim's reclassify).
        @document.approve!(by: current_user)
        finalize_approval(@document)
        redirect_to @document, success: t(".reclassified_approved")
      else
        redirect_to @document, success: t(".saved")
      end
    else
      render :show, status: :unprocessable_entity
    end
  end

  def approve
    @document.approve!(by: current_user)
    finalize_approval(@document)
    respond_to do |format|
      format.turbo_stream { render turbo_stream: document_row_streams(@document) }
      format.html { redirect_to @document, success: t(".approved") }
    end
  end

  # Reject (junk / not-a-doc / wrong): drops the document out of the review queue
  # without deleting it. Reversible — reanalyzing or restoring returns it to review.
  def reject
    @document.reject!
    Notifier.documents_need_review(@document.workspace, bump: false)
    respond_to do |format|
      format.turbo_stream { render turbo_stream: document_row_streams(@document) }
      format.html { redirect_to documents_path, success: t(".rejected") }
    end
  end

  # Star / unstar from the documents list. Flips the flag and swaps the row + card in
  # place, so the icon updates without yanking the entry to the top — the starred-first
  # order takes effect on the next list load.
  def toggle_star
    @document.update!(starred: !@document.starred?)
    respond_to do |format|
      format.turbo_stream { render turbo_stream: document_row_streams(@document) }
      format.html { redirect_back fallback_location: documents_path }
    end
  end

  def reprocess
    return if require_ai_provider!(:documents)

    was_failed = @document.ai_failed?
    @document.update!(ai_status: :pending, review_status: :pending, ai_processing_attempts: 0,
                      ai_error: nil, reviewed_by: nil, reviewed_at: nil)
    Notifier.document_recovered(@document) if was_failed
    Notifier.documents_need_review(@document.workspace, bump: false)
    DocumentProcessJob.perform_later(@document.id)
    redirect_to @document, success: t(".queued")
  end

  def push_to_notion
    mapping = @document.notion_database_mapping
    unless mapping
      redirect_to @document, error: t(".no_mapping") and return
    end

    NotionPushJob.perform_later(@document.id, mapping.id)
    redirect_to @document, success: t(".queued")
  end

  def push_to_drive
    unless Current.workspace.google_drive_accounts.connected.exists?
      redirect_to @document, error: t(".not_connected") and return
    end

    config = @document.classification&.google_drive_config
    unless config
      redirect_to @document, error: t(".no_config") and return
    end

    GoogleDrivePushJob.perform_later(@document.id)
    redirect_to @document, success: t(".queued")
  end

  def push_to_zoho_drive
    accounts = Current.workspace.zoho_drive_accounts.active
    mapping = DriveFolderMapping.where(zoho_drive_account: accounts).find_by(document_type_id: @document.document_type_id)
    mapping ||= DriveFolderMapping.where(zoho_drive_account: accounts).find_by(document_type_id: nil)
    unless mapping
      redirect_to @document, error: t(".no_mapping") and return
    end

    unless mapping.zoho_drive_account.active?
      redirect_to @document, error: t(".inactive_account") and return
    end

    ZohoDriveUploadJob.perform_later(@document.id)
    redirect_to @document, success: t(".queued")
  end

  def reprocess_all
    documents = Current.workspace.documents
    documents = documents.by_type(params[:type]) if params[:type].present?
    documents = documents.by_category(params[:category]) if params[:category].present?

    if params[:year].present? && params[:month].present?
      documents = documents.for_month(params[:year].to_i, params[:month].to_i)
    end

    documents = documents.reprocessable
    count = documents.count
    documents.find_each do |doc|
      was_failed = doc.ai_failed?
      doc.update!(ai_status: :pending, review_status: :pending, ai_processing_attempts: 0,
                  ai_error: nil, reviewed_by: nil, reviewed_at: nil)
      Notifier.document_recovered(doc) if was_failed
      DocumentProcessJob.perform_later(doc.id)
    end
    Notifier.documents_need_review(Current.workspace, bump: false)
    redirect_to documents_path(type: params[:type], category: params[:category], review_status: params[:review_status], month: params[:month]),
                success: t(".queued", count: count)
  end

  def export
    year, month = nil
    if params[:month].present?
      date = Date.parse("#{params[:month]}-01")
      year = date.year.to_s
      month = date.month.to_s
    end

    filters = {
      "type" => params[:type].presence,
      "category" => params[:category].presence,
      "year" => year,
      "month" => month
    }.compact

    export = Current.workspace.exports.create!(status: :pending, filters: filters)
    ExportJob.perform_later(export.id)
    redirect_to documents_path(type: params[:type], category: params[:category], review_status: params[:review_status], month: params[:month]),
                success: t(".generating")
  end

  def merge
    ids = params[:ids].to_s.split(",").map(&:to_i).uniq
    @docs = Current.workspace.documents.includes(:classification, :email_messages).where(id: ids).order(:id)

    if @docs.size < 2
      redirect_to documents_path, error: t(".too_few")
    end
  end

  def perform_merge
    keep = Current.workspace.documents.find(params[:keep_id])
    merge_ids = Array(params[:merge_ids]).map(&:to_i) - [ keep.id ]
    merged = 0

    merge_ids.each do |id|
      dup = Current.workspace.documents.find_by(id: id)
      next unless dup

      dup.document_email_messages.find_each do |dem|
        keep.document_email_messages.find_or_create_by!(email_message_id: dem.email_message_id)
      end

      # Adopt data from merged doc if keep doc has less
      if keep.ai_extraction_data.blank? && dup.ai_extraction_data.present?
        keep.update_columns(
          ai_extraction_data: dup.ai_extraction_data,
          ai_confidence_score: dup.ai_confidence_score,
          metadata: dup.metadata,
          vendor_name: dup.vendor_name.presence || keep.vendor_name,
          client_name: dup.client_name.presence || keep.client_name,
          invoice_number: dup.invoice_number.presence || keep.invoice_number,
          receipt_number: dup.receipt_number.presence || keep.receipt_number,
          document_date: dup.document_date || keep.document_date,
          amount_cents: dup.amount_cents || keep.amount_cents,
          bank_name: dup.bank_name.presence || keep.bank_name
        )
      end

      dup.document_email_messages.delete_all
      dup.original_file.purge if dup.original_file.attached?
      dup.processed_pdf.purge if dup.processed_pdf.attached?
      dup.delete
      merged += 1
    end

    redirect_to keep, success: t(".merged", count: merged)
  end

  private

  def set_document
    @document = Current.workspace.documents.includes(:classification).find(params[:id])
  end

  # True when these params move the document to a different classification.
  def reclassifying?(permitted)
    new_id = permitted[:document_type_id]
    new_id.present? && new_id.to_i != @document.document_type_id
  end

  # Human signed off — recount the review badge and run the post-approval drive pushes
  # (Google + Zoho auto-sync) via the shared finalize job. Immediate here (the detail
  # page has no Skim Undo window); the job self-guards on review_approved?.
  def finalize_approval(document)
    Notifier.documents_need_review(document.workspace, bump: false)
    Documents::FinalizeApprovalJob.perform_later(document.id)
  end

  # Turbo Stream that swaps a single documents-list entry in place after a quick
  # action (approve/reject/star), so the badge, star and inline actions update without
  # a full reload. Both representations are swapped: the desktop table row and the
  # mobile card (distinct dom_ids), so whichever is on screen stays in sync.
  def document_row_streams(document)
    [
      turbo_stream.replace(document, partial: "documents/row", locals: { doc: document }),
      turbo_stream.replace(helpers.dom_id(document, :card), partial: "documents/card", locals: { doc: document })
    ]
  end

  def notify_all_users(documents)
    count = documents.size
    label = count == 1 ? documents.first.original_file.filename.to_s : "#{count} documents"
    Current.workspace.users.find_each do |user|
      next if user == current_user

      Notification.notify(
        user: user,
        category: :activity,
        priority: :activity, # quiet team-activity tier — no toast
        title: "New document uploaded",
        body: "#{current_user.name} uploaded #{label}",
        link_url: count == 1 ? document_path(documents.first) : documents_path,
        group_key: "manual_upload",
        respect_preferences: false
      )
    end
  end

  def document_params
    params.require(:document).permit(
      :document_type_id, :vendor_name, :vendor_nif, :document_date, :due_date,
      :invoice_number, :amount_cents, :currency, :buyer_nif, :tax_amount_cents,
      :tax_rate, :description, :expense_category, :company_vat_present,
      :client_name, :client_nif,
      :bank_name, :account_number, :period_start, :period_end,
      :opening_balance_cents, :closing_balance_cents,
      :receipt_number, :payment_method,
      metadata: {}
    )
  end
end
