module Feed
  # Acts on a single home-feed card. Runs its suggested action — via the shared
  # EmailActions registry — then replies with Turbo Streams that remove the card
  # and raise a toast. Reversible actions raise an Undo toast and can be rolled
  # back via #undo, so a mis-tap is never lost.
  class ItemsController < ApplicationController
    include ActionView::RecordIdentifier # dom_id(feed_item) ⇒ "feed_item_<id>"

    # Actions the feed can roll back (see #undo). Everything else gets a plain toast.
    REVERSIBLE = %w[archive add_tag hold_task schedule_task snooze_task].freeze

    # Ask tools that can be triggered from an email card's chip row (carrying the
    # ask's id) — they act on the ask without removing the email card.
    ASK_CHIP_TOOLS = %w[hold_task schedule_task snooze_task].freeze

    before_action :set_item

    # POST /feed/items/:id/act — perform the chosen action on the underlying record.
    def act
      return act_on_email_ask if email_ask_action?

      result = perform_action
      if result[:success]
        @item.mark_acted!
        if tag_suggestion_item?
          # undo_tag_filing means the user rejected the auto-filing; all other
          # successful acts on a tag card (the legacy "File it") are accepted.
          verdict = params[:tool].to_s == "undo_tag_filing" ? :rejected : :accepted
          record_tag_suggestion_learning(verdict)
        end
        People::StandingsRefreshJob.enqueue_for(current_user.id)
      end

      respond_to do |format|
        format.turbo_stream do
          if result[:success]
            render turbo_stream: [ turbo_stream.remove(dom_id(@item)), success_toast(result) ]
          else
            render turbo_stream: notify_stream(result[:message], severity: :error), status: :unprocessable_entity
          end
        end
        format.html { redirect_back fallback_location: root_path }
      end
    end

    # An ask action fired from an EMAIL card's chip row (args[task_id] present): run
    # it on that ask, but leave the email card in place — only the chip row updates.
    def email_ask_action?
      @item.subject_type == "EmailMessage" &&
        ASK_CHIP_TOOLS.include?(params[:tool].to_s) &&
        params.dig(:args, :task_id).present?
    end

    def act_on_email_ask
      email = @item.subject
      task = Task.accessible_to(current_user).find_by(id: params.dig(:args, :task_id))
      return render_ask_chip_error(t("feed.items.gone")) unless email && task

      result = run_task_action(task)
      if result[:success]
        People::StandingsRefreshJob.enqueue_for(current_user.id)
        respond_to do |format|
          format.turbo_stream do
            render turbo_stream: [
              turbo_stream.replace("ask_chips_#{email.id}",
                render_to_string(Campbooks::Feed::AskChips.new(item: @item, subject: email), layout: false)),
              notify_stream(result[:message], severity: :success)
            ]
          end
          format.html { redirect_back fallback_location: root_path }
        end
      else
        render_ask_chip_error(result[:message])
      end
    end

    def render_ask_chip_error(message)
      respond_to do |format|
        format.turbo_stream { render turbo_stream: notify_stream(message, severity: :error), status: :unprocessable_entity }
        format.html { redirect_back fallback_location: root_path }
      end
    end

    # POST /feed/items/:id/dismiss — hide just this card; the record is untouched.
    def dismiss
      @item.dismiss!
      record_tag_suggestion_learning(:rejected) if tag_suggestion_item?
      People::StandingsRefreshJob.enqueue_for(current_user.id)
      respond_to do |format|
        format.turbo_stream do
          render turbo_stream: [
            turbo_stream.remove(dom_id(@item)),
            undo_toast(t("feed.items.dismissed"), tool: "dismiss_card", args: {})
          ]
        end
        format.html { redirect_back fallback_location: root_path }
      end
    end

    # POST /feed/items/:id/undo — reverse the last action and restore the card.
    def undo
      reverse_action(@item.subject)
      @item.reactivate!
      People::StandingsRefreshJob.enqueue_for(current_user.id)

      respond_to do |format|
        format.turbo_stream do
          render turbo_stream: [
            turbo_stream.prepend("feed_timeline", render_to_string(Campbooks::Feed::Card.new(item: @item, subject: @item.subject), layout: false)),
            notify_stream(t("feed.items.restored"), severity: :info)
          ]
        end
        format.html { redirect_back fallback_location: root_path }
      end
    end

    # POST /feed/items/:id/seen — mark the card seen (drives "new" treatment).
    def seen
      @item.mark_seen!
      head :no_content
    end

    # GET /feed/items/:id/preview — the card's inline "peek": the underlying
    # email (or a reminder/task's source email) rendered into the collapsed
    # turbo-frame lazily, so making the call never requires leaving the feed.
    def preview
      render Campbooks::Feed::EmailPreviewFrame.new(item: @item, subject: preview_message), layout: false
    end

    private

    # Scoped to the user's own feed: anyone else's item 404s (not 403) so we don't
    # leak its existence — matches the app-wide permission-errors convention.
    def set_item
      @item = current_user.feed_items.find(params[:id])
    rescue ActiveRecord::RecordNotFound
      head :not_found
    end

    def tag_suggestion_item?
      @item.kind == "tag_suggestion" && @item.subject.is_a?(EmailMessage)
    end

    # The email behind this card, re-gated at render time: the feed admitted the
    # subject once, but access can be revoked later (account unshared), so the
    # body is only ever served through accessible_to. Nil (→ the frame's quiet
    # "unavailable" note) for non-email cards and revoked/gone messages.
    #
    # A follow-up card is anchored to the other party's inbound message (so its
    # action addresses them), but the peek must show the mail the user SENT and is
    # chasing — Feed::Sources::FollowUp stamps that message's id for exactly this.
    def preview_message
      id =
        if @item.kind == "follow_up" && @item.data["sent_message_id"].present?
          @item.data["sent_message_id"]
        else
          case (subject = @item.subject)
          when EmailMessage then subject.id
          when Reminder, Task then subject.source_email&.id
          end
        end
      id && EmailMessage.accessible_to(current_user).find_by(id: id)
    end

    # Record the accept/reject so Feed::Sources::TagSuggestion learns to stop
    # resurfacing a tag this user keeps rejecting from a sender. Signals are derived
    # server-side from the (already user-scoped) email and the card's stored tag_name —
    # never from request params. Best-effort: never breaks the feed action.
    def record_tag_suggestion_learning(verdict)
      email = @item.subject
      tag_name = @item.data["tag_name"].to_s
      return if email.nil? || tag_name.blank?

      Learning::Recorder.record(
        domain: "tag_suggestion",
        user: current_user,
        workspace_id: current_user.workspace_id,
        label: verdict,
        subject: email,
        contact_id: email.contact_id,
        sender_domain: Emails::SenderDomain.for(email.from_address),
        signals: { "tag_name" => tag_name }
      )
    rescue => e
      Rails.logger.warn("[Feed::ItemsController] tag-suggestion learning failed: #{e.class}: #{e.message}")
    end

    def perform_action
      subject = @item.subject
      return failure(t("feed.items.gone")) if subject.nil?

      case @item.subject_type
      when "EmailMessage" then run_email_action(subject)
      when "Reminder"     then run_reminder_action(subject)
      when "Task"         then run_task_action(subject)
      when "Document"     then run_document_action(subject)
      when "Notification"  then run_notice_action(subject)
      else failure(t("feed.items.unsupported"))
      end
    end

    # Act on a Document card (the late-receivable chase card): "Mark paid" settles
    # it manually (a bank match, when it lands, still wins). Workspace-scoped.
    def run_document_action(document)
      return failure(t("feed.items.gone")) unless document.workspace_id == current_user.workspace_id

      case params[:tool].to_s
      when "mark_paid"
        document.mark_settled!
        { success: true, message: t("feed.items.receivable_marked_paid") }
      else
        failure(t("feed.items.unsupported"))
      end
    end

    # Resolve a notice from the deck: mark it read and archive it, so it leaves the
    # "Needs you" set and won't regenerate as a card. The feed item is already the
    # user's; guard the notification's ownership too. Non-reversible — a plain toast.
    def run_notice_action(notification)
      return failure(t("feed.items.gone")) unless notification.user_id == current_user.id

      case params[:tool].to_s
      when "notice_done"
        notification.mark_as_read!
        notification.archive!
        { success: true, message: t("feed.items.notice_done") }
      else
        failure(t("feed.items.unsupported"))
      end
    end

    # Confirm a reminder into a calendar event, or dismiss it. Scoped to the user's
    # workspace (the feed item is already theirs; this guards the subject too).
    def run_reminder_action(reminder)
      return failure(t("feed.items.gone")) unless reminder.workspace_id == current_user.workspace_id

      case params[:tool].to_s
      when "confirm"
        result = Reminders::Confirm.call(reminder, user: current_user)
        return { success: false, message: result.error } unless result.success?

        message = result.calendar? ? t("feed.items.reminder_confirmed", title: reminder.title)
                                   : t("feed.items.reminder_confirmed_no_calendar")
        { success: true, message: message }
      when "dismiss_reminder"
        reminder.dismissed!
        { success: true, message: t("feed.items.reminder_dismissed") }
      else
        failure(t("feed.items.unsupported"))
      end
    end

    # Act on an ask from the feed: complete/accept/dismiss it, or take one of its
    # three ways out — hold Scout's slot, set a date, or snooze ("Not now"). The last
    # three carry undo_args so the toast can roll them back.
    def run_task_action(task)
      return failure(t("feed.items.gone")) unless task.workspace_id == current_user.workspace_id

      case params[:tool].to_s
      when "complete"
        task.move_to_status!(:done, by: current_user)
        { success: true, message: t("feed.items.task_completed", title: task.title) }
      when "accept"
        task.move_to_status!(:todo, by: current_user)
        { success: true, message: t("feed.items.task_accepted", title: task.title) }
      when "dismiss_task"
        task.move_to_status!(:cancelled, by: current_user)
        { success: true, message: t("feed.items.task_dismissed") }
      when "hold_task"   then hold_ask(task)
      when "schedule_task" then schedule_ask(task)
      when "snooze_task" then snooze_ask(task)
      else
        failure(t("feed.items.unsupported"))
      end
    end

    def hold_ask(task)
      previous = task.status
      result = Time::FocusHolder.call(task, user: current_user)
      return { success: false, message: result.error } unless result.success?

      when_label = I18n.l(result.slot.in_time_zone(current_user.effective_time_zone), format: "%A %H:%M")
      { success: true, message: t("feed.items.ask_held", when: when_label),
        undo_args: { "focus_block_id" => result.focus_block.id, "previous_status" => previous } }
    end

    def schedule_ask(task)
      previous = task.status
      old_due = task.due_at
      date = Asks::PresetDate.resolve(params.dig(:args, :on), current_user.effective_time_zone)
      return failure(t("feed.items.unsupported")) unless date

      task.schedule!(date, zone: current_user.effective_time_zone, by: current_user)
      { success: true, message: t("feed.items.ask_scheduled", date: I18n.l(date, format: :long)),
        undo_args: { "previous_due_at" => old_due&.iso8601, "previous_status" => previous } }
    end

    def snooze_ask(task)
      previous = task.status
      task.snooze!(by: current_user)
      { success: true, message: t("feed.items.ask_snoozed"),
        undo_args: { "previous_status" => previous } }
    end

    # EmailActions re-checks read/send permission against the mailbox, so a viewer
    # can't archive/send beyond their grant even from the feed.
    def run_email_action(email_message)
      return dismiss_follow_up(email_message) if params[:tool].to_s == "dismiss_follow_up"
      return undo_tag_filing(email_message) if params[:tool].to_s == "undo_tag_filing"

      EmailActions.run(params[:tool], email_message: email_message, args: params[:args] || {}, user: current_user)
    end

    # Undo an auto-filing: remove the tag and resolve the notice card. Uses the
    # same remove_tag path as the existing tag undo (REVERSIBLE add_tag) so all
    # permission and tenancy checks are identical.
    def undo_tag_filing(email_message)
      result = EmailActions.run("remove_tag", email_message: email_message, args: params[:args] || {}, user: current_user)
      return result unless result[:success]

      { success: true, message: t("feed.items.tag_filing_undone") }
    end

    # Retire a follow-up on the thread itself (not just this card) so a later feed
    # generation can't resurface it — mirrors the Skim follow-up dismiss.
    def dismiss_follow_up(email_message)
      email_message.email_thread&.update_columns(follow_up_dismissed_at: Time.current)
      { success: true, message: t("feed.items.follow_up_dismissed") }
    end

    # Reverse a prior action so the restored card is consistent with reality. The
    # feed-item state is always cleared by #reactivate!; this undoes the side effect.
    def reverse_action(subject)
      return unless subject

      case params[:tool].to_s
      when "archive"
        Tools::Unarchive.call(subject) if subject.is_a?(EmailMessage)
      when "add_tag"
        EmailActions.run("remove_tag", email_message: subject, args: params[:args] || {}, user: current_user) if subject.is_a?(EmailMessage)
      when "hold_task"
        reverse_hold(subject) if subject.is_a?(Task)
      when "schedule_task"
        reverse_schedule(subject) if subject.is_a?(Task)
      when "snooze_task"
        subject.update!(snoozed_until: nil) if subject.is_a?(Task)
      end
    rescue => e
      Rails.logger.error("[Feed::ItemsController] reverse_action failed: #{e.class}: #{e.message}")
    end

    # Undo a hold: put a merely-suggested ask back to suggested (holding accepted
    # it), dismiss the focus block, and delete any calendar event the hold spawned
    # (the same outbound-delete path as CalendarEventsController#destroy).
    def reverse_hold(task)
      task.update!(status: :suggested) if params.dig(:args, :previous_status) == "suggested"
      block = FocusBlock.accessible_to(current_user).find_by(id: params.dig(:args, :focus_block_id))
      return unless block

      event = block.calendar_event
      block.dismissed!
      return unless event

      event.update_columns(outbound_pending: true)
      Calendars::EventWriteJob.perform_later(event.id, "delete")
      Events.publish("calendar_event.deleted", subject: event, workspace: event.calendar.workspace,
                     payload: { "title" => event.title })
    end

    # Undo a schedule: restore the prior due date (nil clears it) and, if the ask was
    # only a suggestion before, put it back to suggested.
    def reverse_schedule(task)
      previous_due = params.dig(:args, :previous_due_at)
      task.update!(due_at: previous_due.present? ? Time.iso8601(previous_due) : nil)
      task.update!(status: :suggested) if params.dig(:args, :previous_status) == "suggested"
    end

    def success_toast(result)
      message = result[:message]
      if REVERSIBLE.include?(params[:tool].to_s)
        args = (params[:args]&.to_unsafe_h || {}).merge(result[:undo_args] || {})
        undo_toast(message, tool: params[:tool], args: args)
      else
        notify_stream(message, severity: :success)
      end
    end

    def undo_toast(message, tool:, args:)
      undo_params = { "tool" => tool.to_s }
      (args || {}).each { |key, value| undo_params["args[#{key}]"] = value }
      turbo_stream.append(
        Campbooks::ActionToast::REGION_ID,
        render_to_string(
          Campbooks::ActionToast.new(
            message: message, variant: :success,
            undo: { endpoint: undo_feed_item_path(@item), params: undo_params }
          ),
          layout: false
        )
      )
    end

    def failure(message)
      { success: false, message: message }
    end
  end
end
