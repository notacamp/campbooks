class EmailToolsController < ApplicationController
  include ActionView::RecordIdentifier # bare dom_id(...) in the Turbo rendering

  before_action :require_authentication

  SEND_TOOLS = %w[draft_reply draft_follow_up send_reply save_draft send_draft forward_email].freeze

  # Where a Scout draft preview renders, per inbox surface. The compose slots
  # already exist in each surface (the drawer uses a distinct id so it never
  # collides with the full page's thread_compose_target — see EmailDetail), so
  # the preview and the "Edit in composer" composer share one slot and simply
  # swap. A nil surface (the Discussion panel) keeps the comments_list flow.
  COMPOSE_SLOTS = { "drawer" => "drawer_compose_target", "detail" => "thread_compose_target" }.freeze

  # Single-thread actions executed through the shared EmailActions registry. The
  # draft/send cluster stays inline below (UI-only, with bespoke rendering).
  REGISTRY_TOOLS = %w[add_tag remove_tag archive unarchive trash snooze unsnooze forward_email
                      create_calendar_event
                      create_task_from_email link_task_to_email
                      pin unpin
                      star_sender unstar_sender block_sender unblock_sender allow_sender].freeze

  # Approve / Dismiss on a Scout-suggested reminder, rendered inline as buttons on
  # the discussion message (see Reminders::EmailExtractionJob). Confirming runs the
  # same Reminders::Confirm path as the feed card / the /reminders page.
  REMINDER_TOOLS = %w[confirm_reminder dismiss_reminder].freeze

  def create
    email_message = EmailMessage.where(email_account: Current.user.readable_email_accounts)
                                .find(params[:id])

    tool = params[:tool]
    # The draft tools are the AI-backed ones here; the rest (archive/tag/send…)
    # are plain actions that work without a provider.
    return if %w[draft_reply draft_follow_up].include?(tool) && require_ai_provider!(:text)

    args = params[:args] || {}
    args = JSON.parse(args) if args.is_a?(String)
    args = args.to_unsafe_h if args.respond_to?(:to_unsafe_h)
    args = (args || {}).with_indifferent_access
    # The draft preview card is a single form that posts subject/body as
    # top-level fields and routes Save/Send/Edit via formaction. Fold those into
    # args so the send/save tools see the human's edited text (previously the
    # Send button's body was silently dropped).
    args[:subject] = params[:subject] if params[:subject].present?
    args[:body] = params[:body] if params[:body].present?

    if SEND_TOOLS.include?(tool) && !email_message.email_account.sendable_by?(Current.user)
      render json: { error: t(".no_send_permission") }, status: :forbidden
      return
    end

    result = if REGISTRY_TOOLS.include?(tool)
      outcome = EmailActions.run(tool, email_message: email_message, args: args, user: Current.user)
      @action_error = outcome[:message] unless outcome[:success]
      @registry_message = outcome[:message]
      outcome[:success] ? outcome[:result] : nil
    else
      case tool
      when "draft_reply"
        Tools::DraftReply.call(email_message, args, user: Current.user)
      when "draft_follow_up"
        Tools::DraftFollowUp.call(email_message, args, user: Current.user)
      when "confirm_reminder"
        confirm_reminder(args)
      when "dismiss_reminder"
        dismiss_reminder(args)
      when "discard_draft"
        discard_draft(email_message)
      when "send_reply"
        send_reply(email_message, args)
      when "save_draft"
        save_draft(email_message, args)
      when "send_draft"
        send_draft(email_message, args)
      end
    end

    respond_to do |format|
      format.turbo_stream do
        if result
          streams = [ turbo_stream.remove("scout_typing") ]
          toast = nil

          # Persist completed action on the originating AgentMessage
          if params[:agent_message_id].present? && REMINDER_TOOLS.include?(tool)
            # Approve/Dismiss are a mutually-exclusive pair: acting on one resolves
            # the whole suggestion, so collapse BOTH buttons into a single outcome pill.
            outcome = reminder_outcome(tool, result)
            actions_html = complete_reminder_action(params[:agent_message_id], outcome)
            streams << turbo_stream.replace("actions_agent_message_#{params[:agent_message_id]}", actions_html) if actions_html
          elsif params[:agent_message_id].present?
            action_message = case tool
            when "add_tag" then "Tagged as '#{result.dig(:tag, :name)}'"
            when "remove_tag" then "Tag '#{result.dig(:tag, :name)}' removed"
            when "archive" then "Thread archived"
            when "trash" then "Thread trashed"
            when "forward_email" then "Forwarded to #{result[:to_address]}"
            when "create_calendar_event" then @registry_message
            when "snooze" then "Snoozed"
            when "unsnooze" then "Unsnoozed"
            when "star_sender", "unstar_sender", "block_sender", "unblock_sender", "allow_sender" then @registry_message
            when "create_task_from_email", "link_task_to_email" then @registry_message
            end
            if action_message
              actions_html = complete_suggested_action(params[:agent_message_id], tool, action_message)
              if actions_html
                streams << turbo_stream.replace("actions_agent_message_#{params[:agent_message_id]}", actions_html)
              end
            end
          end

          streams += case tool
          when "add_tag"
            toast = { message: t(".tagged", name: result.dig(:tag, :name)), variant: :success }
            [ turbo_stream.replace(
                dom_id(email_message, :thread_tags),
                partial: "email_messages/thread_tags",
                locals: { message: email_message }
              ) ]
          when "remove_tag"
            toast = { message: t(".untagged", name: result.dig(:tag, :name)), variant: :success }
            [ turbo_stream.replace(
                dom_id(email_message, :thread_tags),
                partial: "email_messages/thread_tags",
                locals: { message: email_message }
              ) ]
          when "draft_reply", "draft_follow_up"
            slot = draft_slot
            if result[:needs_info]
              thread = email_message.email_thread
              if thread
                agent_thread_for(thread)&.agent_messages&.where(draft: true, outdated: false)&.update_all(outdated: true)
                agent_thread_for(thread)&.agent_messages&.create!(
                  content: result[:questions].map { |q| q["question"] }.join("\n"),
                  author_type: :ai,
                  draft: true,
                  ai_suggested_actions: result[:questions],
                  user: Current.user
                )
              end
              if params[:surface] == "dock"
                # The ghost slot sits inside the compose <form>, so the
                # question form can't nest there — point the user at Scout.
                [ notify_stream(t(".draft_needs_info"), severity: :warning) ]
              else
                locals = { questions: result[:questions], email_message: email_message, surface: params[:surface] }
                if slot
                  [ turbo_stream.update(slot, partial: "email_comments/draft_questions", locals: locals) ]
                else
                  [ turbo_stream.append("comments_list", partial: "email_comments/draft_questions", locals: locals) ]
                end
              end
            else
              thread = email_message.email_thread
              agent_thread_for(thread)&.agent_messages&.where(draft: true, outdated: false)&.update_all(outdated: true)
              if thread
                agent_thread_for(thread)&.agent_messages&.create!(
                  content: result[:draft]["body"],
                  author_type: :ai,
                  draft: true,
                  ai_suggested_actions: [],
                  user: Current.user
                )
              end
              if params[:surface] == "dock"
                # Regeneration inside the open Dock: swap the ghost in place.
                [ turbo_stream.update("compose_scout_slot", partial: "email_compose/scout_draft",
                                      locals: { text: result[:draft]["body"], message: email_message }) ]
              elsif slot
                # "Suggest reply" from the thread/drawer strips: the reply now
                # starts answered — open the Dock with the ghost pre-loaded.
                [ dock_with_scout_stream(email_message, result[:draft]) ]
              else
                toast = { message: t(".draft_ready"), variant: :success }
                locals = { draft: result[:draft], email_message: email_message, surface: params[:surface] }
                [ turbo_stream.append("comments_list", partial: "email_comments/draft", locals: locals) ]
              end
            end
          when "discard_draft"
            if (slot = draft_slot)
              [ turbo_stream.update(slot, "") ]
            else
              streams = [ turbo_stream.remove(dom_id(email_message, :draft)) ]
              if result[:comment]
                streams << turbo_stream.append(
                  "comments_list",
                  partial: "email_comments/comment",
                  locals: { comment: result[:comment], email_message: email_message }
                )
              end
              streams
            end
          when "send_reply"
            toast = { message: t(".reply_sent"), variant: :success }
            # The owner now holds the last word, so refresh the Scout strip: the
            # reply CTA flips from "Suggest reply" to "Draft follow-up" in place.
            refresh = [ refresh_scout_actions(email_message) ].compact
            cleared = if (slot = draft_slot)
              [ turbo_stream.update(slot, "") ]
            else
              s = [ turbo_stream.remove(dom_id(email_message, :draft)) ]
              if result[:comment]
                s << turbo_stream.append(
                  "comments_list",
                  partial: "email_comments/comment",
                  locals: { comment: result[:comment], email_message: email_message }
                )
              end
              s
            end
            cleared + refresh
          when "save_draft"
            toast = { message: t(".draft_saved"), variant: :success }
            [ turbo_stream.replace(
                "draft_actions_#{email_message.id}",
                partial: "email_comments/draft_saved",
                locals: { provider_draft_id: result[:provider_draft_id], email_message: email_message }
              ) ]
          when "send_draft"
            toast = { message: t(".draft_sent"), variant: :success }
            [ turbo_stream.remove(dom_id(email_message, :draft)) ]
          when "archive"
            # Recoverable: offer an Undo toast (Undo -> unarchive) instead of a
            # plain success toast. The swipe controller renders this stream.
            #
            # A Rewind highlight card (home feed) isn't a materialized feed item,
            # so the inbox-node removals don't apply — remove the card by id and
            # carry surface/reason through the undo so it re-inserts that card.
            removals = if params[:surface] == "rewind"
              [ turbo_stream.remove("rewind_highlight_#{email_message.id}") ]
            else
              clear_thread_from_view(email_message)
            end
            undo_params = { tool: "unarchive" }
            if params[:surface] == "rewind"
              undo_params[:surface] = "rewind"
              undo_params[:reason] = params[:reason] if params[:reason].present?
            end
            removals + [
              turbo_stream.append(
                Campbooks::ActionToast::REGION_ID,
                partial: "shared/undo_toast",
                locals: {
                  message: t(".thread_archived"),
                  endpoint: tool_email_message_path(email_message),
                  params: undo_params
                }
              )
            ]
          when "unarchive"
            toast = { message: t(".thread_unarchived"), variant: :success }
            if params[:surface] == "rewind"
              # Undo from a Rewind highlight: re-insert that card at the top of the
              # feed timeline (mirrors Feed::ItemsController#undo).
              [ turbo_stream.prepend("feed_timeline",
                  render_to_string(
                    Campbooks::Feed::HighlightCard.new(email: email_message, reason: (params[:reason].presence || "starred")),
                    layout: false
                  )) ]
            else
              # Restore the row in the acting tab directly. The inbox_feed broadcast
              # (Emails::InboxBroadcaster#upsert) also prepends, but only the
              # unfiltered default inbox subscribes to it — an Undo clicked while
              # viewing a folder (#294) or a search never got its row back without a
              # reload. The acting tab just watched this exact row leave on archive,
              # so putting it back here is what Undo means; Turbo's prepend
              # de-duplicates by id, so the tabs that also get the broadcast
              # converge to a single row. No-ops where #email_threads doesn't exist.
              if (thread = email_message.email_thread)&.latest_message
                [ turbo_stream.prepend(
                    Emails::InboxBroadcaster::THREADS_CONTAINER,
                    partial: "email_messages/thread_row",
                    locals: { thread: thread, active: false }
                  ) ]
              else
                []
              end
            end
          when "trash"
            toast = { message: t(".thread_trashed"), variant: :success }
            streams = clear_thread_from_view(email_message)
          when "snooze"
            time_str = result[:snoozed_until]&.strftime("%b %d at %H:%M")
            toast = { message: t(".thread_snoozed", time: time_str), variant: :success }
            streams = clear_thread_from_view(email_message, empty_message: t(".conversation_snoozed"))
          when "unsnooze"
            toast = { message: t(".thread_unsnoozed"), variant: :success }
            streams = clear_thread_from_view(email_message, empty_message: t(".no_email_selected"))
          when "forward_email"
            toast = { message: t(".forwarded_to", address: result[:to_address]), variant: :success }
            []
          when "create_calendar_event"
            toast = { message: t(".event_created", title: result[:title]), variant: :success }
            []
          when "confirm_reminder"
            toast = { message: result[:on_calendar] ? t(".reminder_confirmed", title: result[:title]) : t(".reminder_confirmed_no_calendar"), variant: :success }
            []
          when "dismiss_reminder"
            toast = { message: t(".reminder_dismissed"), variant: :success }
            []
          when "create_task_from_email", "link_task_to_email"
            toast = { message: @registry_message, variant: :success }
            []
          when "pin", "unpin"
            # Re-render the row in place so its star + action (and pinned state)
            # flip immediately. The thread relocates to/from the Priority section
            # on the next inbox load (the section is rebuilt server-side per load).
            toast = { message: @registry_message, variant: :success }
            if (thread = email_message.email_thread)
              [ turbo_stream.replace(
                  dom_id(thread, :thread_item),
                  partial: "email_messages/thread_row",
                  locals: { thread: thread, active: false }
                ) ]
            else
              []
            end
          when "block_sender"
            # The block archives existing mail in the background; clear the thread
            # from the view now for immediate feedback.
            toast = { message: @registry_message, variant: :success }
            clear_thread_from_view(email_message)
          when "star_sender", "unstar_sender", "unblock_sender", "allow_sender"
            toast = { message: @registry_message, variant: :success }
            []
          else
            []
          end

          streams << notify_stream(toast[:message], severity: toast[:variant]) if toast
          render turbo_stream: streams
        else
          render turbo_stream: [
            turbo_stream.remove("scout_typing"),
            notify_stream(@action_error || t(".action_failed"), severity: :error)
          ], status: :unprocessable_entity
        end
      end
      format.html do
        if result
          if tool == "archive"
            redirect_to email_messages_path, success: t(".thread_archived")
          elsif tool == "unarchive"
            redirect_to email_messages_path, success: t(".thread_unarchived")
          elsif tool == "snooze"
            redirect_to email_messages_path, success: t(".thread_snoozed_html")
          elsif tool == "unsnooze"
            redirect_to email_message_path(email_message), success: t(".thread_unsnoozed")
          elsif tool == "add_tag"
            redirect_to email_message_path(email_message), success: t(".tagged", name: result.dig(:tag, :name))
          elsif tool == "remove_tag"
            redirect_to email_message_path(email_message), success: t(".untagged", name: result.dig(:tag, :name))
          elsif tool == "send_reply"
            redirect_to email_message_path(email_message), success: t(".reply_sent")
          elsif tool == "save_draft"
            redirect_to email_message_path(email_message), success: t(".save_draft_html")
          elsif tool == "send_draft"
            redirect_to email_message_path(email_message), success: t(".draft_sent")
          elsif tool == "pin" || tool == "unpin"
            redirect_to email_message_path(email_message), success: @registry_message
          else
            redirect_to email_message_path(email_message)
          end
        else
          redirect_to email_message_path(email_message), error: @action_error || t(".action_failed")
        end
      end
    end
  end

  def complete_suggested_action(agent_message_id, tool, message)
    return nil unless agent_message_id.present?

    msg = AgentMessage.find_by(id: agent_message_id, user: Current.user)
    return nil unless msg

    suggested = msg.ai_suggested_actions.dup
    action = suggested.find { |a| a["tool"] == tool }
    return nil unless action

    suggested.delete(action)
    auto = (msg.ai_auto_actions.dup || [])
    auto << { "tool" => tool, "success" => true, "message" => message }

    msg.update!(ai_suggested_actions: suggested, ai_auto_actions: auto)

    router = Rails.application.routes.url_helpers
    email_id = params[:id]
    tool_url_builder = ->(t, a) {
      { url: router.tool_email_message_path(email_id, tool: t, args: a), method: :post }
    }

    component = Campbooks::ChatActions.new(
      auto_actions: auto,
      suggested_actions: suggested,
      tool_url_builder: tool_url_builder,
      message_id: msg.id
    )
    render_to_string(component, layout: false)
  rescue => e
    Rails.logger.error("[EmailToolsController] complete_suggested_action error: #{e.class}: #{e.message}")
    nil
  end

  # Confirm a Scout-suggested reminder onto the calendar (same path as the feed
  # card / /reminders page). Idempotent: a reminder already confirmed elsewhere
  # just reports its existing state so the buttons still resolve cleanly.
  def confirm_reminder(args)
    reminder = Reminder.accessible_to(Current.user).find_by(id: args[:reminder_id])
    return nil unless reminder
    return { confirmed: true, on_calendar: reminder.calendar_event_id.present?, title: reminder.title } if reminder.confirmed?

    result = Reminders::Confirm.call(reminder, user: Current.user)
    return nil unless result.success?
    { confirmed: true, on_calendar: result.calendar?, title: reminder.title }
  end

  def dismiss_reminder(args)
    reminder = Reminder.accessible_to(Current.user).find_by(id: args[:reminder_id])
    return nil unless reminder
    unless reminder.dismissed?
      reminder.dismissed!
      Events.publish("reminder.dismissed", subject: reminder, payload: { "title" => reminder.title, "due_at" => reminder.due_at&.iso8601 })
    end
    { dismissed: true, title: reminder.title }
  end

  # The pill text + tone shown in place of the Approve/Dismiss buttons once one is
  # clicked. Dismiss reads neutral (not a green "success"). Absolute keys: this is a
  # helper, not the action, so a relative `t(".")` would resolve to the wrong scope.
  def reminder_outcome(tool, result)
    if tool == "dismiss_reminder"
      { message: t("email_tools.create.reminder_dismissed"), neutral: true }
    elsif result[:on_calendar]
      { message: t("email_tools.create.reminder_confirmed", title: result[:title]), neutral: false }
    else
      { message: t("email_tools.create.reminder_confirmed_no_calendar"), neutral: false }
    end
  end

  # Sibling of complete_suggested_action, but clears the WHOLE Approve/Dismiss pair
  # (acting on one resolves the suggestion) and renders a single outcome pill.
  def complete_reminder_action(agent_message_id, outcome)
    return nil unless agent_message_id.present?

    msg = AgentMessage.find_by(id: agent_message_id, user: Current.user)
    return nil unless msg

    suggested = msg.ai_suggested_actions.reject { |a| REMINDER_TOOLS.include?(a["tool"]) }
    auto = (msg.ai_auto_actions.dup || [])
    auto << { "tool" => "confirm_reminder", "success" => true, "variant" => (outcome[:neutral] ? "neutral" : nil), "message" => outcome[:message] }.compact

    msg.update!(ai_suggested_actions: suggested, ai_auto_actions: auto)

    router = Rails.application.routes.url_helpers
    email_id = params[:id]
    tool_url_builder = ->(t, a) {
      { url: router.tool_email_message_path(email_id, tool: t, args: a), method: :post }
    }

    component = Campbooks::ChatActions.new(
      auto_actions: auto,
      suggested_actions: suggested,
      tool_url_builder: tool_url_builder,
      message_id: msg.id
    )
    render_to_string(component, layout: false)
  rescue => e
    Rails.logger.error("[EmailToolsController] complete_reminder_action error: #{e.class}: #{e.message}")
    nil
  end

  # The compose slot to render a Scout draft preview into, or nil for the
  # Discussion panel (comments_list flow). See COMPOSE_SLOTS.
  def draft_slot
    COMPOSE_SLOTS[params[:surface].to_s]
  end

  # A ready Scout draft opens the Dock with the ghost block pre-loaded — the
  # reply starts answered instead of a card floating above the thread.
  def dock_with_scout_stream(email_message, draft)
    prefill = Emails::ComposePrefill.for(message: email_message, mode: "reply")
    turbo_stream.update("compose_dock", partial: "email_compose/dock", locals: {
      mode: :reply,
      message: email_message,
      draft: nil,
      to: prefill.to,
      cc: prefill.cc,
      bcc: "",
      subject: prefill.subject,
      body: "",
      quoted_body: prefill.quoted_body,
      signatures: Current.user.signatures.ordered.includes(:email_accounts),
      signature_id: Signature.default_for(Current.user, email_message.email_account)&.id,
      accounts: [],
      attachment_entries: [],
      scout_draft: draft["body"]
    })
  end

  def agent_thread_for(email_thread)
    return nil unless email_thread
    email_thread.agent_thread || email_thread.create_agent_thread!(
      title: email_thread.subject,
      purpose: :email_chat,
      user: Current.user,
      workspace: Current.user.workspace
    )
  end

  def clear_thread_from_view(email_message, empty_message: t(".conversation_archived"))
    streams = [ turbo_stream.remove("email_todo_#{email_message.id}") ]
    if (thread = email_message.email_thread)
      streams << turbo_stream.remove(dom_id(thread, :thread_item))
    end
    streams << turbo_stream.replace("email_content",
      partial: "email_messages/empty_detail",
      locals: { message: empty_message })
    streams
  end

  def send_reply(email_message, args)
    mail_client = email_message.email_account.mail_client
    subject = args["subject"] || "Re: #{email_message.subject}"
    body = args["body"].to_s

    draft_result = mail_client.save_draft(
      subject: subject,
      body: body,
      to_address: Emails::ComposePrefill.reply_to_address(email_message)
    )
    return nil unless draft_result

    draft_id = draft_result["messageId"] || draft_result["id"]
    return nil unless draft_id

    mail_client.send_draft(draft_id)

    thread = email_message.email_thread
    comment = nil
    if thread
      # Mark the thread as awaiting their reply right away so the conversation drops
      # from Skim/the feed and the Scout strip can flip to "Draft follow-up" without
      # waiting for the sent copy to sync back.
      thread.update_column(:last_outbound_at, Time.current)
      agent_thread_for(thread)&.agent_messages&.where(draft: true, outdated: false)&.update_all(outdated: true)
      comment = agent_thread_for(thread)&.agent_messages&.create!(
        content: body,
        author_type: :ai,
        ai_suggested_actions: [],
        user: Current.user
      )
    end
    { sent: true, comment: comment }
  rescue => e
    Rails.logger.error("[EmailTools] send_reply failed: #{e.message}")
    nil
  end

  # Re-render the open email's Scout strip so its lead CTA reflects the thread's
  # current state — after a reply, "Suggest reply" becomes "Draft follow-up". Only
  # the drawer/detail surfaces carry the strip; the Discussion panel (nil surface)
  # has none, so this is a no-op there.
  def refresh_scout_actions(email_message)
    surface = params[:surface].presence
    return nil unless %w[drawer detail].include?(surface)

    email_message.reload # pick up the thread's just-updated last_outbound_at
    turbo_stream.replace(
      "scout_actions_#{email_message.id}",
      render_to_string(
        Campbooks::EmailScoutActions.new(
          message: email_message,
          surface: surface,
          can_send: email_message.email_account.sendable_by?(Current.user),
          class: Campbooks::EmailScoutActions::SURFACE_CLASS
        ),
        layout: false
      )
    )
  end

  def discard_draft(email_message)
    thread = email_message.email_thread
    return nil unless thread
    agent_thread_for(thread)&.agent_messages&.where(draft: true, outdated: false)&.update_all(outdated: true)
    comment = agent_thread_for(thread)&.agent_messages&.create!(
      content: "I see you discarded that draft. What would you like instead? Should I try a different tone, focus on different points, or suggest something else?",
      author_type: :ai,
      ai_suggested_actions: [
        { "tool" => "draft_reply", "args" => { "summary" => "Try a shorter, more direct version" } },
        { "tool" => "draft_reply", "args" => { "summary" => "Try a more formal tone" } },
        { "tool" => "draft_reply", "args" => { "summary" => "Focus on different aspects" } }
      ],
      user: Current.user
    )
    { discarded: true, comment: comment }
  end

  def save_draft(email_message, args)
    mail_client = email_message.email_account.mail_client
    draft_result = mail_client.save_draft(
      subject: args["subject"] || "Re: #{email_message.subject}",
      body: args["body"].to_s,
      to_address: Emails::ComposePrefill.reply_to_address(email_message)
    )
    draft_result ? { provider_draft_id: draft_result["messageId"] || draft_result["id"] } : nil
  end

  def send_draft(email_message, args)
    mail_client = email_message.email_account.mail_client
    mail_client.send_draft(args["draft_message_id"])
  end
end
