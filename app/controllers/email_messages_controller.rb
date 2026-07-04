# Pagy's countless extra powers the thread sidebar's infinite scroll (no COUNT query
# on the GROUP BY'd thread scope). Required here, co-located with its only consumer,
# so it loads on controller (re)load — no dev-server restart needed after a deploy.
require "pagy/extras/countless"

class EmailMessagesController < ApplicationController
  before_action :require_authentication
  layout :select_layout

  # Sidebar thread list page size. Sized to roughly fill the first viewport so the
  # initial render stays light; the rest streams in on scroll (infinite scroll).
  THREAD_PAGE_LIMIT = 25

  # Flat search-results page size (keyword mode). Mirrors the thread page size.
  SEARCH_PAGE_LIMIT = 25

  # Max pinned ("Priority") threads shown in the top section. Pinned mail is a
  # small, curated set, so this is just a guard against a runaway list.
  PINNED_LIMIT = 50

  def index
    respond_to do |format|
      # Infinite-scroll: the sidebar's lazy `threads_pagination` frame requests the
      # next page as a turbo stream and appends it to the list.
      format.turbo_stream do
        @current_group = params[:group].presence
        @current_smart_group = valid_smart_group_param
        @pagy, @threads = pagy_countless(
          build_thread_scope(params[:folder_id], @current_group, folder_name: params[:folder_name].presence, exclude_pinned: true, exclude_waiting_reply: true, smart_group: @current_smart_group),
          limit: THREAD_PAGE_LIMIT
        )
      rescue Pagy::OverflowError
        # Requested page is past the end — e.g. a stale sentinel, or the list shrank
        # since it was rendered. Nothing more to append; just retire the sentinel.
        render turbo_stream: turbo_stream.remove("threads_pagination")
      end

      format.html do
        scope = EmailMessage.where(email_account: Current.user.readable_email_accounts)

        if params[:folder_name].present?
          scope = scope.where(provider_folder_id: custom_folder_provider_ids(params[:folder_name]))
        elsif params[:folder_id] == "all"
          # Show all folders — no filter
        elsif params[:folder_id].present?
          folder_ids = equivalent_folder_ids(params[:folder_id])
          scope = scope.where(provider_folder_id: folder_ids)
        else
          inbox_ids = inbox_folder_ids
          scope = scope.where(provider_folder_id: inbox_ids) if inbox_ids.any?
        end

        if (smart_group = valid_smart_group_param)
          # Smart-group drill-in: land on a message inside the bucket.
          bundled = smart_groups_service.bundled_scope_for(smart_group)
          scope = bundled ? scope.where(email_thread_id: bundled) : scope.none
        elsif params[:group].present?
          scope = scope.joins(:tags)
                       .where(tags: { group_name: params[:group], workspace_id: Current.user.workspace_id })
        else
          # Skip threads hidden from the main list (grouped threads) so the redirect
          # lands on a message that appears in the sidebar — otherwise no row is
          # marked active and the keyboard shortcuts (x/e/#/mark) have nothing to act on.
          if (hidden = grouped_thread_scope)
            scope = scope.where.not(email_thread_id: hidden)
          end
          # Same for smart-bundled threads (inbox root only) — they surface as
          # collapsed group rows, not inline.
          if inbox_root? && (bundled = smart_groups_service.bundled_scope)
            scope = scope.where.not(email_thread_id: bundled)
          end
        end

        latest = scope.order(received_at: :desc).first
        if latest
          folder_id = params[:folder_name].present? ? nil : (params[:folder_id].presence || inbox_folder_id)
          redirect_to email_message_path(latest, folder_id: folder_id, folder_name: params[:folder_name].presence, group: params[:group].presence, smart_group: valid_smart_group_param, inbox_settings: params[:inbox_settings].presence, show_list: params[:show_list].presence)
        else
          @accounts = Current.user.readable_email_accounts.ordered
          # With everything bundled the main list is empty but the inbox isn't:
          # the empty state shows the group rows instead of "no mail yet".
          @smart_group_items = inbox_root? && !valid_smart_group_param ? smart_groups_service.build_groups(inbox_folder_ids) : []
          render :empty
        end
      end
    end
  end

  # Inbox search & filters. Renders a flat, permission-scoped message list into the
  # `email_search_results` Turbo Frame (the middle list pane) — the search bar form
  # navigates that frame, so the folder sidebar + reading pane stay put. Always
  # renders the `index` template; its frame branches on @search_active. Keyword mode
  # (the default) paginates like the inbox; meaning mode returns a single ranked
  # page. The turbo_stream format serves the infinite-scroll append only.
  def search
    @search_params = search_params
    @search_active = !blank_search?

    if @search_active
      @folder_ids = resolve_search_folder_ids(@search_params[:folder])
      searcher = Emails::Search.new(user: Current.user, params: @search_params, folder_ids: @folder_ids)

      if searcher.text_query?
        # Free text → relevance ranking (embedding + keyword): a single bounded page.
        @messages = searcher.results
        @pagy = nil
      else
        # Filters only → browse, paginated by recency.
        @pagy, @messages = pagy_countless(searcher.scope, limit: SEARCH_PAGE_LIMIT)
      end
    else
      # Empty submit (cleared box, no filters) — restore the normal inbox list
      # inside the frame rather than 404/redirect.
      @current_group = nil
      @pagy, @threads = pagy_countless(build_thread_scope(nil, nil), limit: THREAD_PAGE_LIMIT)
    end

    respond_to do |format|
      # Folder/account/tag chrome is only needed by the full render, not the
      # lazy pagination stream.
      format.html { load_inbox_chrome; render :index }
      format.turbo_stream # search.turbo_stream.erb — infinite-scroll append
    end
  rescue Pagy::OverflowError
    # Stale infinite-scroll sentinel past the end of the results — retire it.
    render turbo_stream: turbo_stream.remove("threads_pagination")
  end

  def show
    @message = EmailMessage.where(email_account: readable_accounts)
                           .includes(:email_account, :email_scan_log, :tags)
                           .find(params[:id])
    @thread = @message.email_thread
    @agent_thread = @thread&.agent_thread
    @comments = @agent_thread&.agent_messages&.chronological || []
    @can_send = @message.email_account.sendable_by?(Current.user)

    # Redirect to folder context if email is not in any account's inbox
    if params[:folder_id].blank? && params[:folder_name].blank? && @message.provider_folder_id.present?
      inbox_ids = inbox_folder_ids
      if inbox_ids.any? && !inbox_ids.include?(@message.provider_folder_id)
        return redirect_to email_message_path(@message, folder_id: @message.provider_folder_id, inbox_settings: params[:inbox_settings].presence)
      end
    end

    AuditEvent.log("email_message_read", user: Current.user, request: request, target: @message, via: "web")
    mark_thread_read

    # Gather attachments from all messages in the thread (or just this message if no thread)
    @thread_messages = @thread ? @thread.email_messages.includes(:files_attachments, :email_account).order(received_at: :desc).to_a : [ @message ]
    message_ids = @thread_messages.map(&:id)
    @thread_documents = Document.joins(:document_email_messages)
                                .where(document_email_messages: { email_message_id: message_ids })
                                .distinct
                                .includes(:classification)
                                .order(created_at: :desc)
    raw_files = @thread_messages.flat_map { |m| m.files.map { |f| [ m, f ] } }
    @thread_files = raw_files.reject { |_, f| @thread_documents.any? { |d| d.original_file.filename.to_s == f.filename.to_s } }

    # In-place navigation: the "email_detail" turbo frame wraps only the reading
    # pane, so a frame request (thread-row click, arrow keys) answers with just
    # that pane — the thread list, folder rail and chrome are neither queried
    # nor re-rendered, which is what keeps the list's scroll position intact.
    if turbo_frame_request_id == "email_detail"
      @folders = build_folder_list(folder_counts_for_lists) # detail-context needs the move-to-folder list
      return render :show_detail, layout: false
    end

    # Load thread list for sidebar (full page loads only — frame requests
    # returned above). Only the first page is loaded up front; the rest streams
    # in on scroll.
    readable_ids = readable_account_ids

    # Keep any active inbox search/filter alive across opening an email: when the
    # result link carries search params, render the list pane as the *filtered*
    # results (and repopulate the search bar from them) instead of the folder
    # thread list — so opening a result doesn't reset the filter back to the inbox.
    @search_params = search_params
    @search_active = !blank_search?

    if @search_active
      @folder_ids = resolve_search_folder_ids(@search_params[:folder])
      searcher = Emails::Search.new(user: Current.user, params: @search_params, folder_ids: @folder_ids)
      if searcher.text_query?
        @messages = searcher.results
        @pagy = nil
      else
        @pagy, @messages = pagy_countless(searcher.scope, limit: SEARCH_PAGE_LIMIT)
      end
      # The folder-list view branches on these; keep them inert in search mode.
      @current_group = nil
      @current_smart_group = nil
      @tag_groups = {}
      @group_items = []
      @smart_group_items = []
      @pinned_threads = []
      @waiting_threads = []
      @threads = []
    else
      # Build tag groups with per-group queries (powers the group chips in the sidebar)
      @current_group = params[:group].presence
      @current_smart_group = valid_smart_group_param
      @tag_groups = build_tag_groups(readable_ids, params[:folder_id])

      # Pinned ("Priority") threads ride at the top in their own section, shown only
      # on this first page; the paginated stream below excludes them (exclude_pinned)
      # so each pinned thread appears exactly once.
      @pinned_threads = build_thread_scope(params[:folder_id], @current_group, folder_name: params[:folder_name].presence, smart_group: @current_smart_group)
                          .where(id: EmailThread.pinned)
                          .limit(PINNED_LIMIT)
                          .to_a

      # "Waiting on replies" — threads where the owner holds the last word, in their
      # own sticky section above the date list (like Priority). Excluded from the
      # paginated stream below (exclude_waiting_reply) so each shows exactly once.
      @waiting_threads = waiting_thread_scope(params[:folder_id], @current_group, folder_name: params[:folder_name].presence)

      @pagy, @threads = pagy_countless(
        build_thread_scope(params[:folder_id], @current_group, folder_name: params[:folder_name].presence, exclude_pinned: true, exclude_waiting_reply: true, smart_group: @current_smart_group),
        limit: THREAD_PAGE_LIMIT
      )

      @group_items = @tag_groups.filter_map do |name, data|
        next unless data[:count] > 0
        { label: name, count: data[:count], senders: data[:senders], color: data[:color] }
      end

      # Smart-group rows (bundled low-priority mail) — inbox root only, and not
      # while drilled into a bucket.
      @smart_group_items =
        if inbox_root? && @current_smart_group.nil?
          smart_groups_service.build_groups(inbox_folder_ids)
        else
          []
        end
    end

    # Build folder list merged across accounts by folder name
    folder_counts = folder_counts_for_lists
    @folders = build_folder_list(folder_counts)
    @common_folders = baseline_folders(folder_counts)
    @mail_folders = custom_folders
    @accounts = readable_accounts
    @current_folder = params[:folder_name].presence || params[:folder_id] || inbox_folder_id
    @all_tags = workspace_tags.uniq(&:name)
  end

  # The Desk: the full-page compose surface. Opens blank (new message), resumes
  # a parked draft (?draft_id), or expands a Dock reply (?mode&reply_to) — in
  # the last two cases the envelope, body, quote and attachments carry over.
  def new
    @sendable_accounts = Current.user.sendable_email_accounts
    @account = @sendable_accounts.first
    @signatures = Current.user.signatures.ordered.includes(:email_accounts)

    @draft = Current.user.draft_emails.find_by(id: params[:draft_id]) if params[:draft_id].present?
    @reply_source = @draft&.in_reply_to
    @mode = @draft&.mode || "new_message"

    if @draft.nil? && params[:reply_to].present? && EmailComposeController::MODES.include?(params[:mode].to_s)
      @reply_source = EmailMessage.accessible_to(Current.user).find_by(id: params[:reply_to])
      @mode = params[:mode].to_s if @reply_source
    end

    if @draft
      @to = @draft.to_address.to_s
      @cc = @draft.cc_address.to_s
      @bcc = @draft.bcc_address.to_s
      @subject = @draft.subject.to_s
      @body = @draft.body.to_s
      @quoted_body = @draft.quoted_body.to_s
      @attachment_entries = @draft.attachment_entries
      @signature_id = @draft.signature_id
    elsif @reply_source
      prefill = Emails::ComposePrefill.for(message: @reply_source, mode: @mode)
      @to = prefill.to
      @cc = prefill.cc
      @bcc = ""
      @subject = prefill.subject
      @body = ""
      @quoted_body = prefill.quoted_body
      @attachment_entries = @mode == "forward" ? Emails::ComposePrefill.forward_attachment_entries(@reply_source) : []
      @signature_id = Signature.default_for(Current.user, @reply_source.email_account)&.id
    else
      @to = @cc = @bcc = @subject = @body = @quoted_body = ""
      @attachment_entries = []
      @signature_id = (Signature.default_for(Current.user, @account)&.id if @account)
    end

    # Thread created lazily on first message — avoids empty threads in sidebar
    @compose_thread = nil
    @compose_messages = []
  end

  def dismiss_todo
    @message = EmailMessage.where(email_account: Current.user.readable_email_accounts).find(params[:id])
    @message.update!(ai_todo_dismissed: true)

    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [
          turbo_stream.remove("email_todo_#{@message.id}"),
          notify_stream(t(".dismissed"), severity: :success)
        ]
      end
      format.html { redirect_back fallback_location: root_path, success: t(".dismissed") }
    end
  end

  # Dismiss the "waiting on reply" nudge for a thread from the inbox section: mark
  # it dismissed (EmailThread.awaiting_reply then drops it) and re-render the
  # Waiting section so the row leaves — and the whole band disappears when it was
  # the last one.
  def dismiss_follow_up
    message = EmailMessage.where(email_account: Current.user.readable_email_accounts).find(params[:id])
    message.email_thread&.update(follow_up_dismissed_at: Time.current)

    @current_group = params[:group].presence
    @waiting_threads = waiting_thread_scope(params[:folder_id], @current_group, folder_name: params[:folder_name].presence)

    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [
          turbo_stream.replace("waiting_replies_section",
            partial: "email_messages/waiting_section",
            locals: { waiting_threads: @waiting_threads }),
          notify_stream(t(".dismissed"), severity: :success)
        ]
      end
      format.html { redirect_back fallback_location: email_messages_path, success: t(".dismissed") }
    end
  end

  def drawer_content
    @message = EmailMessage.where(email_account: Current.user.readable_email_accounts)
                           .includes(:email_account, :tags, :email_thread)
                           .find(params[:id])
    @thread = @message.email_thread
    messages = @thread ? @thread.email_messages.includes(:files_attachments).order(received_at: :desc) : [ @message ]
    message_ids = messages.map(&:id)
    @thread_documents = Document.joins(:document_email_messages)
                                .where(document_email_messages: { email_message_id: message_ids })
                                .distinct
                                .includes(:classification)
                                .order(created_at: :desc)
    # Gate the inline reply composer on send permission (read-only shared inboxes
    # can view but not reply).
    @can_send = @message.email_account.sendable_by?(Current.user)

    # Count of real (non-draft) discussion comments, for the drawer's Discussion
    # badge. Only queried when a thread already has an agent thread — most inbox
    # mail has none (discussions are opt-in, created on the first comment).
    agent_thread = @thread&.agent_thread
    @discussion_count = agent_thread ? agent_thread.agent_messages.chronological.reject(&:draft?).size : 0

    # Opening the email in the bottom-right drawer is a read, same as the
    # full-page view — mark the thread read so the inbox unread dot/bold and
    # counts clear (issue #135).
    AuditEvent.log("email_message_read", user: Current.user, request: request, target: @message, via: "web")
    mark_thread_read

    render layout: false
  end

  private

  # Opening a thread marks every message read — clears the inbox unread dot/bold,
  # the "unread" filters, and the unread counts — and stamps viewed_at, which
  # clears the Mail nav attention dot (Navigation::Attention#new_mail?). The
  # viewed_at clause also catches messages that synced in already-read but were
  # never opened here, so the nav dot still clears. Shared by the full-page `show`
  # and the bottom-right `drawer_content` so opening either surface marks read.
  def mark_thread_read
    return unless @thread

    messages = @thread.email_messages
    unread_ids = messages.where(read: false).pluck(:provider_message_id)
    messages.where(read: false).or(messages.where(viewed_at: nil))
            .update_all(read: true, viewed_at: Time.current, updated_at: Time.current)
    return if unread_ids.empty?

    MarkReadJob.perform_later(@message.email_account_id, unread_ids) if @message.email_account_id
    # Live inbox: clear the unread dot on this thread's row in every other open
    # inbox (other tabs/devices, teammates sharing the mailbox).
    Emails::InboxBroadcaster.replace(@thread)
  end

  def readable_accounts
    @readable_accounts ||= Current.user.readable_email_accounts.ordered.to_a
  end

  def readable_account_ids
    @readable_account_ids ||= readable_accounts.map(&:id)
  end

  # Per-folder distinct thread counts, shared by the folder pane and the
  # move-to-folder pickers. Cached briefly — it walks every message row.
  def folder_counts_for_lists
    Rails.cache.fetch("folder_counts/#{readable_account_ids.sort.join('_')}", expires_in: 1.minute) do
      EmailMessage.where(email_account_id: readable_account_ids).group(:provider_folder_id).distinct.count(:email_thread_id)
    end
  end

  # Folder list merged across accounts by folder name.
  def build_folder_list(folder_counts)
    folder_mappings[:name_to_ids].map { |name, ids|
      count = ids.sum { |id| folder_counts[id] || 0 }
      { id: ids.first, name: name, count: count }
    }.sort_by { |f| f[:name] == "Inbox" ? "  " : f[:name] }
  end

  def workspace_tags
    @workspace_tags ||= Tag.visible_for(Current.workspace).by_name.to_a
  end

  # Search params the bar may submit. `q` is the free-text relevance query; the
  # rest are exact structured filters.
  def search_params
    params.permit(
      :q, :folder, :sender, :domain, :date_from, :date_to,
      :has_attachment, :unread, :category, :priority, :tag_match, :page,
      account_ids: [], tag_ids: []
    )
  end

  # No text and no filters → not really a search; the caller bounces to the inbox.
  def blank_search?
    return false if @search_params[:q].present?
    return false if Array(@search_params[:account_ids]).any?(&:present?)
    return false if Array(@search_params[:tag_ids]).any?(&:present?)
    return false if @search_params[:folder].present? && @search_params[:folder] != "all"

    %i[sender domain date_from date_to has_attachment unread category priority]
      .none? { |k| @search_params[k].present? }
  end

  # A folder *name* (chosen in the panel) → the provider folder ids that share it
  # across the user's accounts, via the controller's cached, mail-client-backed
  # mapping. nil = no folder filter; [] = the name matched no folders.
  def resolve_search_folder_ids(folder)
    return nil if folder.blank? || folder == "all"
    name_to_folder_ids(folder)
  end

  # Folder sidebar + account/tag chrome for the full-page (non-Turbo) search render.
  def load_inbox_chrome
    readable_ids = readable_account_ids
    folder_counts = Rails.cache.fetch("folder_counts/#{readable_ids.sort.join('_')}", expires_in: 1.minute) do
      EmailMessage.where(email_account_id: readable_ids).group(:provider_folder_id).distinct.count(:email_thread_id)
    end
    @folders = folder_mappings[:name_to_ids].map { |name, ids|
      { id: ids.first, name: name, count: ids.sum { |id| folder_counts[id] || 0 } }
    }.sort_by { |f| f[:name] == "Inbox" ? "  " : f[:name] }
    @common_folders = baseline_folders(folder_counts)
    @mail_folders = custom_folders
    @accounts = readable_accounts
    @all_tags = workspace_tags.uniq(&:name)
    @current_folder = @search_params[:folder]
  end

  # Smart groups (bundled low-priority mail) — one service per request; its
  # bundled scope is memoized inside, so the main-list exclusion and the group
  # rows share the same computation.
  def smart_groups_service
    @smart_groups_service ||= Emails::SmartGroups.new(Current.user, readable_account_ids)
  end

  # The ?smart_group= param, but only when it names a real bucket — anything
  # else reads as "not drilled in" rather than erroring.
  def valid_smart_group_param
    bucket = params[:smart_group].presence
    bucket if bucket && User::SMART_GROUP_BUCKETS.include?(bucket)
  end

  # The unfiltered default inbox — the only view that bundles. Folder views
  # (incl. "all") and custom folders show everything inline.
  def inbox_root?
    params[:folder_id].blank? && params[:folder_name].blank?
  end

  # EmailThreads hidden from the main list because they belong to a tag group.
  # Returns an AR relation usable as a `where.not(...)` subquery, or nil when no
  # grouped tags exist. Shared by `index` (so the redirect lands on a message
  # that actually appears in the sidebar) and `show` (which builds the list).
  def grouped_thread_scope
    grouped_tag_names = workspace_tags.select { |t| t.group_name.present? }.map(&:name)
    return nil if grouped_tag_names.empty?

    EmailThread.joins(email_messages: { email_message_tags: :tag })
               .where(tags: { name: grouped_tag_names })
  end

  # The ordered EmailThread relation backing the sidebar list, with the same
  # folder/group filtering used everywhere. Shared by `show`, `new`, and the
  # paginated `index` turbo stream so every page of the infinite scroll is
  # consistent. Returns a GROUP BY'd relation ordered by latest message — paginate
  # it with `pagy_countless` (a COUNT would return a hash here).
  def build_thread_scope(folder_id, group, folder_name: nil, exclude_pinned: false, exclude_waiting_reply: false, smart_group: nil)
    scope = EmailThread.where(email_account_id: readable_account_ids)
                       .includes(:email_account, email_messages: [ :email_account, :tags, :files_attachments, { documents: :classification } ])
                       .joins(:email_messages)
                       .group("email_threads.id")
                       .order(Arel.sql("MAX(email_messages.received_at) DESC"))

    inbox_view = false
    if folder_name.present?
      # Custom folder — resolved by name from the persisted email_folders mirror.
      scope = scope.where(email_messages: { provider_folder_id: custom_folder_provider_ids(folder_name) })
    elsif folder_id == "all"
      # Show all folders — no filter
    elsif folder_id.present?
      scope = scope.where(email_messages: { provider_folder_id: equivalent_folder_ids(folder_id) })
    else
      inbox_ids = inbox_folder_ids
      scope = scope.where(email_messages: { provider_folder_id: inbox_ids }) if inbox_ids.any?
      inbox_view = true
    end

    if smart_group.present?
      # Smart-group drill-in: only the bucket's bundled threads. The inbox
      # folder filter above still applies to the message JOIN — bundled threads
      # have inbox messages by construction, so nothing valid is dropped.
      bundled = smart_groups_service.bundled_scope_for(smart_group)
      scope = bundled ? scope.where(id: bundled) : scope.none
    elsif group.present?
      # Filter to a specific tag group.
      group_thread_ids = EmailThread.joins(email_messages: { email_message_tags: :tag })
                                    .where(tags: { group_name: group, workspace_id: Current.user.workspace_id })
                                    .distinct.pluck(:id)
      scope = scope.where(id: group_thread_ids)
    else
      if (hidden = grouped_thread_scope)
        # Exclude grouped threads from the main list (they surface as group chips).
        scope = scope.where.not(id: hidden)
      end
      # Same for smart-bundled threads — collapsed into group rows on the inbox
      # root; folder/search views stay untouched.
      if inbox_view && (bundled = smart_groups_service.bundled_scope)
        scope = scope.where.not(id: bundled)
      end
    end

    # The main date-sectioned list drops pinned threads so they show only once,
    # in the Priority section above it (and on every infinite-scroll page).
    scope = scope.where.not(id: EmailThread.pinned) if exclude_pinned

    # Likewise drops "waiting on reply" threads so they show only once, in the
    # Waiting section above it (and never on infinite-scroll pages).
    scope = scope.where.not(id: EmailThread.awaiting_reply) if exclude_waiting_reply

    scope
  end

  # The threads for the inbox "Waiting on replies" section: everything in the
  # current folder view where the owner holds the last word, minus pinned (which
  # ride in their own Priority section). Mirrors the @pinned_threads build.
  def waiting_thread_scope(folder_id, group, folder_name: nil)
    build_thread_scope(folder_id, group, folder_name: folder_name)
      .where(id: EmailThread.awaiting_reply)
      .where.not(id: EmailThread.pinned)
      .limit(PINNED_LIMIT)
      .to_a
  end

  def build_tag_groups(readable_ids, folder_id)
    grouped = workspace_tags.select { |t| t.group_name.present? }.group_by(&:group_name)
    return {} if grouped.empty?

    base = EmailThread.where(email_account_id: readable_ids).joins(:email_messages)
    if folder_id.present? && folder_id != "all"
      folder_ids = equivalent_folder_ids(folder_id)
      base = base.where(email_messages: { provider_folder_id: folder_ids })
    end

    grouped.transform_values do |tags|
      tag_names = tags.map(&:name)
      count = base.joins(email_messages: { email_message_tags: :tag })
                  .where(tags: { name: tag_names })
                  .distinct.count

      # Collect up to 3 unique sender addresses for the group header
      sender_rows = EmailMessage
        .joins(:email_thread, email_message_tags: :tag)
        .where(tags: { name: tag_names })
        .where(email_thread: { email_account_id: readable_ids })
        .order(received_at: :desc)
        .limit(20)
        .pluck(:from_address, :contact_id, :email_account_id)
      top_rows = sender_rows.uniq { |r| r[0] }.first(3)
      account_colors = EmailAccount.where(id: top_rows.map { |r| r[2] }.compact.uniq)
                                   .pluck(:id, :color).to_h
      senders = top_rows.map do |addr, contact_id, account_id|
        { email: addr, contact_id: contact_id, sent: false, account_color: account_colors[account_id] }
      end

      { tags: tags, count: count, senders: senders, color: tags.first&.color }
    end
  end

  def inbox_folder_ids
    name_to_folder_ids("Inbox")
  end

  def inbox_folder_id
    inbox_folder_ids.first
  end

  def folder_mappings
    @folder_mappings ||= begin
      account_ids = Current.user.readable_email_accounts.pluck(:id).sort
      cache_key = "folder_mappings/user_#{Current.user.id}/#{account_ids.join('_')}"

      Rails.cache.fetch(cache_key, expires_in: 5.minutes) do
        name_to_ids = {}
        id_to_name = {}
        id_to_account = {}

        Current.user.readable_email_accounts.each do |account|
          client = account.mail_client rescue nil
          next unless client
          folders = client.list_folders rescue []
          folders.each do |f|
            name = f["folderName"]
            id = f["folderId"]
            next unless name && id
            name_to_ids[name] ||= []
            name_to_ids[name] << id unless name_to_ids[name].include?(id)
            id_to_name[id] ||= name
            id_to_account[id] ||= account
          end
        end

        { name_to_ids: name_to_ids, id_to_name: id_to_name, id_to_account: id_to_account }
      end
    end
  end

  def name_to_folder_ids(name)
    folder_mappings[:name_to_ids][name] || []
  end

  # User-defined folders for the chip bar (workspace-scoped, ordered).
  def custom_folders
    @custom_folders ||= Current.user.workspace.mail_folders.ordered.to_a
  end

  # Provider folder ids for a custom folder name, resolved from the persisted
  # email_folders mirror across the user's readable accounts. Instant after a
  # folder is provisioned — no dependency on the 5-minute folder_mappings cache.
  def custom_folder_provider_ids(name)
    EmailFolder.where(email_account_id: readable_account_ids)
               .where("LOWER(name) = ?", name.to_s.downcase)
               .pluck(:provider_folder_id)
  end

  def equivalent_folder_ids(folder_id)
    name = folder_mappings[:id_to_name][folder_id]
    return [ folder_id ] unless name
    name_to_folder_ids(name)
  end

  BASELINE_FOLDERS = %w[Inbox Sent Drafts Archive Spam Trash].freeze

  def baseline_folders(folder_counts)
    name_to_ids = folder_mappings[:name_to_ids]

    BASELINE_FOLDERS.filter_map { |name|
      ids = name_to_ids[name]
      next nil unless ids&.any?
      { id: ids.first, name: name, count: ids.sum { |id| folder_counts[id] || 0 } }
    }
  end

  def select_layout
    "email"
  end
end
