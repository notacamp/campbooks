class FilesController < ApplicationController
  # The Files area — a unified file manager over the workspace's files (Documents)
  # organized into custom folders (MailFolder). #index lists everything ("All
  # files"); #show scopes to a single folder (its subfolders + filed contents).
  # Both render the same template; @folder distinguishes the two.
  def index
    load_folders
    build_search

    load_filter_data

    # "All files": the workspace's internal documents + uploaded files. Emails only
    # surface inside a folder they've been filed into (see #show). The filter strip
    # narrows every kind shown here, not just the files (see #filtered_internal_docs).
    @internal_docs = filtered_internal_docs(Current.workspace.authored_documents.accessible_to(Current.user))
    @filed_emails  = []

    @needs_review_count = Current.workspace.documents.needs_review.count

    if @search.text_query?
      # Bounded, ranked result set — no pagination for free-text queries.
      @files = @search.results
      @pagy  = nil
      @reprocessable_count = 0
      @exports = []
    else
      @pagy, @files = pagy(@search.scope, items: 30)
      @reprocessable_count = reprocessable_count
      @exports = Current.workspace.exports.recent.limit(10)
    end

    # Show the file-manager layout (sidebar + list) once the workspace has anything
    # to organise — files, internal docs, or folders — independent of the active
    # filter, so filtering down to zero shows "no matches" rather than the first-run CTA.
    @has_any_files = @folders.any? || Current.workspace.documents.exists? || Current.workspace.authored_documents.exists?

    # Visiting Files clears the nav attention dot.
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
    build_search

    load_filter_data
    @internal_docs = filtered_internal_docs(@folder.authored_documents)
    @filed_emails  = filtered_filed_emails(@folder.email_messages.accessible_to(Current.user))

    # Skim is workspace-wide, so the "Review N" button shows inside a folder too.
    @needs_review_count = Current.workspace.documents.needs_review.count

    if @search.text_query?
      @files = @search.results
      @pagy  = nil
    else
      @pagy, @files = pagy(@search.scope, items: 30)
    end

    @has_any_files = true

    respond_to do |format|
      format.html { render :index }
      format.turbo_stream { render :index }
    end
  end

  private

  # Build one Documents::Search per request, deriving @filters and @search_query.
  # text_query? drives bounded-vs-paginated in the action.
  def build_search
    @search       = Documents::Search.new(
      user: Current.user, workspace: Current.workspace, params: params, folder: @folder
    )
    @filters      = @search.filters
    @search_query = @search.search_text # parsed free text (modifiers stripped)
  end

  def load_folders
    @folders = Current.workspace.mail_folders.accessible_to(Current.user).ordered.to_a
    # Badge counts every filed item kind (files + internal docs + emails).
    @folder_counts = MailFolder.item_counts(@folders)
  end

  # Inputs for the filter strip — document types, categories, and folders.
  # @folders is already available from load_folders (permission-scoped).
  def load_filter_data
    @document_types = Current.workspace.document_types.order(:name)
    @categories     = DocumentType::CATEGORIES
  end

  # Internal documents (AuthoredDocument) shown alongside the files. Any
  # document-specific filter (type/category/review/source/amount/etc.) hides them
  # entirely; the date range still applies via created_at.
  def filtered_internal_docs(scope)
    return [] if @filters.document_specific?

    if (range = @filters.date_range)
      scope = scope.where(authored_documents: { created_at: range })
    end
    scope.recent.limit(50).to_a
  end

  # Filed emails shown inside a folder — same rule as internal docs (date range
  # by received_at; document-specific filter excludes them).
  def filtered_filed_emails(scope)
    return [] if @filters.document_specific?

    scope = scope.order(received_at: :desc)
    if (range = @filters.date_range)
      scope = scope.where(email_messages: { received_at: range })
    end
    scope.limit(50).to_a
  end

  # Number of documents that can be reprocessed under the current filter set.
  # Drives the "Reanalyze N" button label.
  def reprocessable_count
    @filters.apply(
      Current.workspace.documents.accessible_to(Current.user),
      workspace: Current.workspace,
      user: Current.user
    ).reprocessable.count
  end
end
