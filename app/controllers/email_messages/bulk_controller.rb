class EmailMessages::BulkController < ApplicationController
  before_action :require_authentication

  def create
    email_ids = Array(params[:email_ids]).map(&:to_i).uniq
    return render_error(t(".no_emails_selected")) if email_ids.empty?

    # Expand to all messages in the selected threads
    all_ids = expand_to_threads(email_ids)

    result = dispatch_tool(params[:tool], all_ids, email_ids)

    respond_to do |format|
      format.turbo_stream do
        if result
          streams = build_response(params[:tool], result, email_ids, all_ids)
          render turbo_stream: streams
        else
          render_error(t(".action_failed"))
        end
      end
      format.html do
        if result
          redirect_to email_messages_path, success: success_message(params[:tool], result)
        else
          redirect_to email_messages_path, error: t(".action_failed_retry")
        end
      end
    end
  end

  private

  def dispatch_tool(tool, all_ids, selected_ids)
    case tool
    when "archive"
      Tools::BulkArchive.call("email_ids" => all_ids)
    when "unarchive"
      Tools::BulkUnarchive.call("email_ids" => all_ids)
    when "mark_read"
      Tools::BulkMarkRead.call(email_ids: selected_ids, read: true)
    when "mark_unread"
      Tools::BulkMarkRead.call(email_ids: selected_ids, read: false)
    when "move_to_folder"
      folder_name = params[:folder_name].presence
      folder_id = params[:folder_id].presence
      return nil unless folder_name || folder_id
      Tools::BulkMoveToFolder.call(email_ids: selected_ids, folder_id: folder_id, folder_name: folder_name)
    when "tag"
      action = params[:tag_action] || "add"
      Tools::BulkTag.call("email_ids" => all_ids, "tag_name" => params[:tag_name], "action" => action)
    when "delete"
      Tools::BulkDelete.call(email_ids: selected_ids)
    when "process_ai"
      Tools::BulkProcessAi.call(email_ids: selected_ids)
    when "scout_chat"
      Tools::BulkScoutChat.call(email_ids: selected_ids, user: Current.user)
    when "snooze"
      snoozed_until = params[:snoozed_until].presence
      return nil unless snoozed_until
      Tools::BulkSnooze.call("email_ids" => all_ids, "snoozed_until" => snoozed_until)
    when "unsnooze"
      Tools::BulkUnsnooze.call("email_ids" => all_ids)
    else
      nil
    end
  end

  def build_response(tool, result, selected_ids, all_ids = [])
    streams = []
    toast = nil

    case tool
    when "archive"
      # Reversible: offer an Undo snackbar (Undo -> bulk unarchive the same ids).
      streams << turbo_stream.remove("bulk_selection_toolbar")
      streams << bulk_undo_stream(
        t(".archived", count: result[:archived_count]), tool: "unarchive", ids: all_ids
      )
    when "unarchive"
      toast = { message: t(".unarchived", count: result[:unarchived_count]), variant: :success }
      streams.concat(thread_list_refresh)
    when "mark_read"
      toast = { message: t(".marked_read", count: result[:count]), variant: :success }
      # Refresh the thread list to update unread dots
      streams.concat(thread_list_refresh)
    when "mark_unread"
      toast = { message: t(".marked_unread", count: result[:count]), variant: :success }
      streams.concat(thread_list_refresh)
    when "move_to_folder"
      # Drag-to-folder / tap-to-move (a named custom folder, with the source folder
      # in `from`): the moved threads leave the current view, so remove their rows
      # and offer an Undo that moves them back. The command-palette move (folder_id)
      # and the Undo round-trip itself fall through to a plain refresh + toast.
      if result[:folder_name].present? && params[:from].present?
        EmailThread.where(id: Array(result[:thread_ids])).each do |thread|
          streams << turbo_stream.remove(helpers.dom_id(thread, :thread_item))
        end
        streams << turbo_stream.remove("bulk_selection_toolbar")
        streams << bulk_undo_stream(
          t(".moved_to_folder_named", folder: result[:folder_name], count: result[:count]),
          tool: "move_to_folder", ids: all_ids, extra: { "folder_name" => params[:from] }
        )
      else
        toast = { message: t(".moved_to_folder", count: result[:count]), variant: :success }
        streams.concat(thread_list_refresh)
      end
    when "tag"
      verb = result[:action] == "remove" ? t(".tag_verb_removed") : t(".tag_verb_added")
      message = t(".tagged", verb: verb, tag_name: result[:tag_name], count: result[:tagged_count])
      streams.concat(thread_list_refresh)
      if result[:action] == "remove"
        toast = { message: message, variant: :success }
      else
        # Reversible: Undo removes the tag we just added from the same messages.
        streams << bulk_undo_stream(message, tool: "tag", ids: all_ids,
          extra: { "tag_action" => "remove", "tag_name" => result[:tag_name] })
      end
    when "forward"
      # Build combined forward compose for selected emails
      messages = EmailMessage.accessible_to(Current.user).where(id: selected_ids).includes(:email_account, :email_thread).order(received_at: :desc)
      if messages.any?
        first = messages.first
        combined_body = messages.map { |msg|
          from = msg.from_address || "Unknown"
          date = msg.received_at&.strftime("%b %d, %Y at %H:%M") || "Unknown date"
          body = msg.body.presence || msg.summary.presence || "(no content)"
          "<br><p style='font-size:12px;color:#9ca3af;'>---------- Forwarded message ----------<br><b>From:</b> #{ERB::Util.html_escape(from)}<br><b>Date:</b> #{date}<br><b>Subject:</b> #{ERB::Util.html_escape(msg.subject || '')}<br><b>To:</b> #{ERB::Util.html_escape(msg.to_address || '')}</p><br>#{body}"
        }.join("<br><hr style='border:0;border-top:1px dashed #d1d5db;margin:16px 0'><br>")

        streams << turbo_stream.prepend("thread_compose_target",
          partial: "email_compose/compose_area",
          locals: {
            email_message: first,
            mode: :forward,
            to_address: "",
            cc_address: "",
            subject: "Fwd: #{messages.size} emails",
            quoted_body: combined_body,
            signature_content: Signature.default_for(Current.user, first.email_account)&.content
          })
        toast = { message: t(".forwarded_compose", count: messages.size), variant: :success }
      end
    when "delete"
      toast = { message: t(".deleted", count: result[:count]), variant: :success }
      streams.concat(thread_list_refresh)
    when "process_ai"
      toast = { message: t(".processing_ai", count: result[:count]), variant: :success }
    when "scout_chat"
      toast = { message: t(".sent_to_scout", count: result[:message_count]), variant: :success }
      streams << notify_stream("<a href='#{scout_thread_path(result[:thread_id])}' class='underline'>#{t('.open_scout_chat')}</a>".html_safe, severity: :info)
    when "snooze"
      # Reversible: offer an Undo snackbar (Undo -> bulk unsnooze the same ids).
      streams << turbo_stream.remove("bulk_selection_toolbar")
      streams << bulk_undo_stream(
        t(".snoozed", count: result[:snoozed_count]), tool: "unsnooze", ids: all_ids
      )
    when "unsnooze"
      toast = { message: t(".unsnoozed", count: result[:unsnoozed_count]), variant: :success }
      streams.concat(thread_list_refresh)
    end

    streams << notify_stream(toast[:message], severity: toast[:variant]) if toast
    streams
  end

  def expand_to_threads(email_ids)
    base = EmailMessage.accessible_to(Current.user)
    thread_ids = base.where(id: email_ids).where.not(email_thread_id: nil).pluck(:email_thread_id).uniq
    base.where(email_thread_id: thread_ids).pluck(:id)
  end

  def reload_threads
    readable_ids = Current.user.readable_email_accounts.pluck(:id)
    EmailThread.where(email_account_id: readable_ids)
               .includes(:email_account, email_messages: [ :tags, :files_attachments ])
               .joins(:email_messages)
               .group("email_threads.id")
               .order(Arel.sql("MAX(email_messages.received_at) DESC"))
               .limit(100)
  end

  # Refresh the sidebar list after a bulk action. `update` (not `replace`) keeps the
  # #email_threads container — its id, stimulus controller, and density attribute —
  # and the sibling infinite-scroll sentinel intact. The sentinel is cleared because
  # it points at a now-stale page offset; navigating to a thread restores paging.
  def thread_list_refresh
    [
      turbo_stream.update("email_threads", partial: "email_messages/thread_list", locals: { threads: reload_threads }),
      turbo_stream.remove("threads_pagination")
    ]
  end

  # Appends an Undo snackbar for a reversible bulk action. The Undo button POSTs
  # the reverse tool + the affected ids back to this same bulk endpoint, which
  # restores the threads and refreshes the list.
  def bulk_undo_stream(message, tool:, ids:, extra: {})
    turbo_stream.append(
      Campbooks::ActionToast::REGION_ID,
      partial: "shared/undo_toast",
      locals: {
        message: message,
        endpoint: bulk_email_messages_path,
        params: { "tool" => tool, "email_ids[]" => ids }.merge(extra)
      }
    )
  end

  def success_message(tool, result)
    case tool
    when "archive" then t(".archived_html", count: result[:archived_count])
    when "mark_read" then t(".marked_read_html", count: result[:count])
    when "mark_unread" then t(".marked_unread_html", count: result[:count])
    when "move_to_folder" then t(".moved_html", count: result[:count])
    when "tag"
      verb = result[:action] == "remove" ? t(".tag_verb_removed") : t(".tag_verb_added")
      t(".tagged_html", verb: verb, tag_name: result[:tag_name])
    when "delete" then t(".deleted_html", count: result[:count])
    when "process_ai" then t(".ai_processing_html", count: result[:count])
    when "scout_chat" then t(".scout_html", count: result[:message_count])
    when "snooze" then t(".snoozed_html", count: result[:snoozed_count])
    else t(".done")
    end
  end

  def render_error(message)
    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [
          notify_stream(message, severity: :error)
        ], status: :unprocessable_entity
      end
      format.html do
        redirect_to email_messages_path, error: message
      end
    end
  end
end
