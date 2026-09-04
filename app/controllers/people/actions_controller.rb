# frozen_string_literal: true

module People
  # Acts on a single People-list row: Done, Undo Done, Snooze, Unsnooze, Star,
  # Unstar, Archive, Unarchive. Resolves the row for the current user, delegates
  # to the existing mechanisms (FeedItem#dismiss!, EmailActions, Contact#star!/unstar!),
  # refreshes the single counterpart via People::Standings.refresh_counterpart!,
  # then replies with Turbo Streams: a re-render of the full counterpart list
  # into the people_results frame plus an ActionToast with Undo when reversible.
  class ActionsController < ApplicationController
    include PeopleLayout
    include ActionView::RecordIdentifier
    include EmailMessageHelpers

    DONE_KINDS = People::Directory::DONE_KINDS

    before_action :set_row
    before_action :set_email_message
    before_action :set_feed_item

    def create
      result = dispatch_kind(params[:kind])
      return if performed? # do_done/etc may have rendered a 404 directly.

      if result[:success]
        # Refresh the single row so the list re-renders with the current state.
        People::Standings.refresh_counterpart!(current_user, @row.counterpart)
        build_people_list
        respond_to do |format|
          format.turbo_stream do
            render turbo_stream: [
              turbo_stream.replace("people_results",
                partial: "people/counterpart_list"),
              result[:toast]
            ].compact
          end
          format.html { redirect_to people_path }
        end
      else
        respond_to do |format|
          format.turbo_stream do
            render turbo_stream: notify_stream(result[:message], severity: :error),
                   status: :unprocessable_entity
          end
          format.html { redirect_to people_path, alert: result[:message] }
        end
      end
    end

    private

    def set_row
      @row = PeopleStanding.for_user(current_user).find_by(counterpart_id: params[:id])
      head :not_found unless @row
    end

    def set_email_message
      return unless @row
      id = @row.email_message_id
      @message = id && EmailMessage.accessible_to(current_user).find_by(id: id)
    end

    def set_feed_item
      return unless @row
      id = @row.feed_item_id
      @item = id && current_user.feed_items.find_by(id: id)
    end

    # Route to the right handler for each kind.
    def dispatch_kind(kind)
      case kind
      when "done"       then do_done
      when "undo_done"  then do_undo_done
      when "snooze"     then do_snooze
      when "unsnooze"   then do_unsnooze
      when "star"       then do_star
      when "unstar"     then do_unstar
      when "archive"    then do_archive
      when "unarchive"  then do_unarchive
      else
        failure(t("people.actions.unsupported"))
      end
    end

    # ── Done ───────────────────────────────────────────────────────────────────

    def do_done
      return head :not_found unless @item && DONE_KINDS.include?(@item.kind)

      if @item.kind == "follow_up"
        @item.subject&.email_thread&.update_columns(follow_up_dismissed_at: Time.current)
      end
      @item.dismiss!
      name = people_first_name_for(@row)
      {
        success: true,
        message: t("people.actions.done", name: name),
        toast: undo_toast(t("people.actions.done", name: name),
                          endpoint: people_action_path(params[:id], :undo_done),
                          undo_label: t("people.actions.undo"))
      }
    end

    def do_undo_done
      return failure(t("people.actions.not_found")) unless @item

      if @item.kind == "follow_up"
        @item.subject&.email_thread&.update_columns(follow_up_dismissed_at: nil)
      end
      @item.reactivate!
      { success: true, message: t("people.actions.restored"),
        toast: notify_stream(t("people.actions.restored"), severity: :info) }
    end

    # ── Snooze ─────────────────────────────────────────────────────────────────

    def do_snooze
      return failure(t("people.actions.needs_message")) unless @message

      snooze_until = resolve_snooze_until
      result = EmailActions.run("snooze", email_message: @message,
                                args: { until: snooze_until.iso8601 }, user: current_user)
      return failure(result[:message]) unless result[:success]

      {
        success: true,
        message: t("people.actions.snoozed", until: l(snooze_until, format: :short)),
        toast: undo_toast(t("people.actions.snoozed", until: l(snooze_until, format: :short)),
                          endpoint: people_action_path(params[:id], :unsnooze),
                          undo_label: t("people.actions.undo"))
      }
    end

    def do_unsnooze
      return failure(t("people.actions.needs_message")) unless @message

      result = EmailActions.run("unsnooze", email_message: @message, args: {}, user: current_user)
      return failure(result[:message]) unless result[:success]

      { success: true, message: t("people.actions.unsnoozed"),
        toast: notify_stream(t("people.actions.unsnoozed"), severity: :success) }
    end

    # ── Star ───────────────────────────────────────────────────────────────────

    def do_star
      contact = busiest_contact
      return failure(t("people.actions.not_found")) unless contact

      contact.star!
      contact.track_event("contact.starred") rescue nil
      name = people_first_name_for(@row)
      {
        success: true,
        message: t("people.actions.starred", name: name),
        toast: undo_toast(t("people.actions.starred", name: name),
                          endpoint: people_action_path(params[:id], :unstar),
                          undo_label: t("people.actions.undo"))
      }
    end

    def do_unstar
      contact = busiest_contact
      return failure(t("people.actions.not_found")) unless contact

      contact.unstar!
      contact.track_event("contact.unstarred") rescue nil
      name = people_first_name_for(@row)
      {
        success: true,
        message: t("people.actions.unstarred", name: name),
        toast: undo_toast(t("people.actions.unstarred", name: name),
                          endpoint: people_action_path(params[:id], :star),
                          undo_label: t("people.actions.undo"))
      }
    end

    # ── Archive ────────────────────────────────────────────────────────────────

    def do_archive
      return failure(t("people.actions.needs_message")) unless @message

      result = EmailActions.run("archive", email_message: @message, args: {}, user: current_user)
      return failure(result[:message]) unless result[:success]

      name = people_first_name_for(@row)
      {
        success: true,
        message: t("people.actions.archived", name: name),
        toast: undo_toast(t("people.actions.archived", name: name),
                          endpoint: people_action_path(params[:id], :unarchive),
                          undo_label: t("people.actions.undo"))
      }
    end

    def do_unarchive
      return failure(t("people.actions.needs_message")) unless @message

      result = EmailActions.run("unarchive", email_message: @message, args: {}, user: current_user)
      return failure(result[:message]) unless result[:success]

      { success: true, message: t("people.actions.unarchived"),
        toast: notify_stream(t("people.actions.unarchived"), severity: :success) }
    end

    # ── Helpers ────────────────────────────────────────────────────────────────

    def busiest_contact
      person = @row.counterpart
      return nil unless person.respond_to?(:contacts)

      person.contacts.max_by { |c| c.email_count.to_i }
    end

    def resolve_snooze_until
      preset_key = params[:until].to_s
      presets = snooze_presets
      preset = presets.find { |key, _label, _time| key.to_s == preset_key }
      # default to tomorrow morning
      preset ? preset[2] : presets.find { |key, _l, _t| key == :tomorrow }&.last || 1.day.from_now.change(hour: 9)
    end

    def undo_toast(message, endpoint:, undo_label: nil)
      turbo_stream.append(
        Campbooks::ActionToast::REGION_ID,
        render_to_string(
          Campbooks::ActionToast.new(
            message: message, variant: :success,
            undo: { endpoint: endpoint, params: { "_method" => "post" }, label: undo_label || t("people.actions.undo") }
          ),
          layout: false
        )
      )
    end

    def failure(message)
      { success: false, message: message }
    end

    def people_first_name_for(row)
      row.name.to_s.split(" ").first.presence || row.name
    end
  end
end
