module Feed
  # Acts on a single home-feed card. Runs its suggested action — via the shared
  # EmailActions registry — then replies with Turbo Streams that remove the card
  # and raise a toast. Reversible actions raise an Undo toast and can be rolled
  # back via #undo, so a mis-tap is never lost.
  class ItemsController < ApplicationController
    include ActionView::RecordIdentifier # dom_id(feed_item) ⇒ "feed_item_<id>"

    # Actions the feed can roll back (see #undo). Everything else gets a plain toast.
    REVERSIBLE = %w[archive add_tag].freeze

    before_action :set_item

    # POST /feed/items/:id/act — perform the chosen action on the underlying record.
    def act
      result = perform_action
      @item.mark_acted! if result[:success]

      respond_to do |format|
        format.turbo_stream do
          if result[:success]
            render turbo_stream: [ turbo_stream.remove(dom_id(@item)), success_toast(result[:message]) ]
          else
            render turbo_stream: notify_stream(result[:message], severity: :error), status: :unprocessable_entity
          end
        end
        format.html { redirect_back fallback_location: root_path }
      end
    end

    # POST /feed/items/:id/dismiss — hide just this card; the record is untouched.
    def dismiss
      @item.dismiss!
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

    private

    # Scoped to the user's own feed: anyone else's item 404s (not 403) so we don't
    # leak its existence — matches the app-wide permission-errors convention.
    def set_item
      @item = current_user.feed_items.find(params[:id])
    rescue ActiveRecord::RecordNotFound
      head :not_found
    end

    def perform_action
      subject = @item.subject
      return failure(t("feed.items.gone")) if subject.nil?

      case @item.subject_type
      when "EmailMessage" then run_email_action(subject)
      when "Reminder"     then run_reminder_action(subject)
      else failure(t("feed.items.unsupported"))
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

    # EmailActions re-checks read/send permission against the mailbox, so a viewer
    # can't archive/send beyond their grant even from the feed.
    def run_email_action(email_message)
      return dismiss_follow_up(email_message) if params[:tool].to_s == "dismiss_follow_up"

      EmailActions.run(params[:tool], email_message: email_message, args: params[:args] || {}, user: current_user)
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
      end
    rescue => e
      Rails.logger.error("[Feed::ItemsController] reverse_action failed: #{e.class}: #{e.message}")
    end

    def success_toast(message)
      if REVERSIBLE.include?(params[:tool].to_s)
        undo_toast(message, tool: params[:tool], args: (params[:args]&.to_unsafe_h || {}))
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
