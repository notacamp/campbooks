class FilesController < ApplicationController
  # The Files area — a unified file manager over the workspace's files (Documents)
  # organized into custom folders (MailFolder). #index lists everything ("All
  # files"); #show scopes to a single folder (its subfolders + filed contents).
  # Both render the same template; @folder distinguishes the two.
  def index
    load_folders
    return if render_search

    load_filter_data
    # "All files": the workspace's internal documents + uploaded files. Emails only
    # surface inside a folder they've been filed into (see #show). The filter strip
    # narrows every kind shown here, not just the files (see #filtered_internal_docs).
    @internal_docs = filtered_internal_docs(Current.workspace.authored_documents.accessible_to(Current.user))
    @filed_emails = []

    scope = files_scope
    # Workspace-wide review queue size — drives the header "Review N" button (Skim).
    @needs_review_count = Current.workspace.documents.needs_review.count
    # Carried over from the old Documents index: bulk re-analyze + export history.
    @reprocessable_count = scope.rewhere(review_status: :pending, ai_status: [ :pending, :completed, :failed ]).count
    @exports = Current.workspace.exports.recent.limit(10)

    @pagy, @files = pagy(scope, items: 30)
    # Show the file-manager layout (sidebar + list) once the workspace has anything to
    # organize — files, internal docs, or folders — independent of the active filter,
    # so filtering down to zero shows "no matches" rather than the first-run CTA.
    @has_any_files = @folders.any? || Current.workspace.documents.exists? || Current.workspace.authored_documents.exists?

    # Visiting Files clears the nav attention dot: stamp viewed_at on the docs that
    # drive it (needs_review), independent of the active filter. Matches
    # Navigation::Attention#new_documents?. (Moved here from DocumentsController#index.)
    Current.workspace.documents.needs_review.where(viewed_at: nil).update_all(viewed_at: Time.current)

    respond_to do |format|
      format.html
      format.turbo_stream
    end
  end

  def show
    @folder = Current.workspace.mail_folders.find(params[:id])
    # 404 (not 403) for a restricted folder the user can't read — don't leak existence.
    return head :not_found unless @folder.readable_by?(Current.user)

    @subfolders = @folder.children.to_a
    load_folders
    return if render_search

    load_filter_data
    @internal_docs = filtered_internal_docs(@folder.authored_documents)
    @filed_emails = filtered_filed_emails(@folder.email_messages.accessible_to(Current.user))
    # Skim is workspace-wide, so the "Review N" button shows inside a folder too.
    @needs_review_count = Current.workspace.documents.needs_review.count
    @pagy, @files = pagy(files_scope, items: 30)
    @has_any_files = true

    respond_to do |format|
      format.html { render :index }
      format.turbo_stream { render :index }
    end
  end

  private

  # When the search box carries a query, swap the structural browse for a ranked
  # Documents::Search result set (documents/files only — internal docs and filed
  # emails keep their own surfaces). Returns true once it has rendered, so the
  # action returns; false to fall through to the normal browse/filter path. Assumes
  # @folder and @folders are already loaded by the caller.
  def render_search
    return false if params[:q].blank?

    @search_query = params[:q].to_s.strip
    searcher = Documents::Search.new(
      user: Current.user, workspace: Current.workspace, params:, folder: @folder
    )
    @files = searcher.results
    @pagy = nil
    @internal_docs = []
    @filed_emails = []
    @subfolders ||= []
    @needs_review_count = Current.workspace.documents.needs_review.count
    @reprocessable_count = 0
    @exports = []
    @has_any_files = true # never show the first-run CTA mid-search

    load_filter_data # the filter strip still renders alongside the search box

    respond_to do |format|
      format.html { render :index }
      format.turbo_stream { render :index }
    end
    true
  end

  def load_folders
    @folders = Current.workspace.mail_folders.accessible_to(Current.user).ordered.to_a
    # Badge counts every filed item kind (files + internal docs + emails).
    @folder_counts = MailFolder.item_counts(@folders)
  end

  # Inputs for the (collapsible) filter strip — the document types and the fixed
  # category list. The folder dropdown reuses @folders (already permission-scoped).
  def load_filter_data
    @document_types = Current.workspace.document_types.order(:name)
    @categories = DocumentType::CATEGORIES
  end

  # "All files" by default; a single folder's contents when scoped (via #show, or
  # the ?folder_id= filter the mobile picker uses). Mirrors DocumentsController's
  # includes/order so rows render without N+1s.
  def files_scope
    scope = if @folder
      @folder.documents
    elsif params[:folder_id].present?
      Current.workspace.documents.accessible_to(Current.user).in_folder(params[:folder_id])
    else
      Current.workspace.documents.accessible_to(Current.user)
    end
    scope = apply_filters(scope)
    scope.includes(:classification).with_attached_original_file.starred_first.recent
  end

  # The same Document filter scopes the old Documents index used. Each scope guards
  # against a blank param (returns `all`), so an unfiltered load is unaffected.
  def apply_filters(scope)
    scope = scope.by_type(params[:type])
    scope = scope.by_category(params[:category])
    scope = scope.by_review_status(params[:review_status])
    scope = scope.by_ai_status(params[:ai_status])
    scope = scope.for_month(*month_filter) if month_filter
    scope
  end

  # Internal documents (AuthoredDocument) shown alongside the files. The filter strip
  # narrows the whole folder view: type/category/review_status are document-only
  # attributes, so any of them excludes internal docs entirely; a month filter still
  # applies, by created_at. Columns are table-qualified — inside a folder the scope
  # joins folder_memberships, which also has a created_at (else: ambiguous column).
  def filtered_internal_docs(scope)
    return [] if document_specific_filter?
    scope = scope.where(authored_documents: { created_at: month_time_range(*month_filter) }) if month_filter
    scope.recent.limit(50).to_a
  end

  # Filed emails shown inside a folder — same rule as internal docs (month filters by
  # received_at; any document-specific filter excludes them).
  def filtered_filed_emails(scope)
    return [] if document_specific_filter?
    scope = scope.order(received_at: :desc)
    scope = scope.where(email_messages: { received_at: month_time_range(*month_filter) }) if month_filter
    scope.limit(50).to_a
  end

  # True when a filter that only Documents can satisfy is active — so non-document
  # items (internal docs, emails) fall out of the result set.
  def document_specific_filter?
    params[:type].present? || params[:category].present? ||
      params[:review_status].present? || params[:ai_status].present?
  end

  # A whole calendar month as a timestamp range (for created_at / received_at, which
  # are datetimes — unlike Document#document_date, a plain date, which `for_month` handles).
  def month_time_range(year, month)
    start_date = Date.new(year, month, 1)
    start_date.beginning_of_day..start_date.end_of_month.end_of_day
  end

  # The month picker (<input type="month">) submits a single "YYYY-MM" value; parse
  # it into the [year, month] pair `for_month` expects. Nil when absent/unparseable.
  # Uses Date.strptime with an explicit format so Rails' lenient Date.parse doesn't
  # silently convert strings like "garbage" into valid dates (it would interpret
  # "garbage-01" as the 1st of the current month, applying a wrong filter).
  def month_filter
    return if params[:month].blank?

    date = Date.strptime(params[:month], "%Y-%m")
    [ date.year, date.month ]
  rescue ArgumentError
    nil
  end
end
