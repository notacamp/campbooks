# frozen_string_literal: true

module Api
  module App
    module People
      # POST /api/app/people/:id/action
      # Row action: done / undo_done / snooze / unsnooze / star / unstar /
      # archive / unarchive.
      #
      # paid settles the person's late invoice via Document#mark_settled! (mirrors
      # the web People paid action); a landed bank match still wins later.
      #
      # Returns: { data: { row:, undo_kind:, undo_params: } }
      # undo_kind + undo_params let the client issue an undo with a follow-up POST.
      class ActionsController < Api::App::BaseController
        SUPPORTED_KINDS = %w[done undo_done snooze unsnooze star unstar archive unarchive paid].freeze
        DONE_KINDS      = ::People::Directory::DONE_KINDS

        before_action :set_row
        before_action :set_email_message
        before_action :set_feed_item

        def create
          kind = params[:kind].to_s
          unless SUPPORTED_KINDS.include?(kind)
            return render_error("unsupported_action",
                                "Unsupported action kind '#{kind}'.",
                                status: :unprocessable_entity)
          end

          result = dispatch_kind(kind)

          unless result[:success]
            return render_error("action_failed", result[:message] || "Action failed.",
                                status: :unprocessable_entity)
          end

          # Refresh the single row so the response carries current state.
          ::People::Standings.refresh_counterpart!(current_user, @row.counterpart)
          fresh_row = PeopleStanding.for_user(current_user).find_by(counterpart_id: params[:id])
          serialized_row = fresh_row ? Api::App::PeopleStandingSerializer.new(fresh_row).as_json : nil

          # Realtime: fan the row change out to this user's SPA so an open People
          # list updates live (pilot surface; other surfaces adopt the same
          # one-liner). See api-migration/realtime.md.
          UserSyncChannel.publish(current_user, topic: "people",
                                  action: serialized_row ? "upsert" : "remove",
                                  id: params[:id], payload: serialized_row || {})

          render_data({
            row:         serialized_row,
            undo_kind:   result[:undo_kind],
            undo_params: result[:undo_params],
            message:     result[:message]
          })
        end

        private

        def set_row
          @row = PeopleStanding.for_user(current_user).find_by(counterpart_id: params[:id])
          render_not_found unless @row
        end

        def set_email_message
          return unless @row

          id = @row.email_message_id.presence || params[:email_message_id].presence
          @message = id && EmailMessage.accessible_to(current_user).find_by(id: id)
        end

        def set_feed_item
          return unless @row

          id = @row.feed_item_id.presence || params[:feed_item_id].presence
          @item = id && current_user.feed_items.find_by(id: id)
        end

        def dispatch_kind(kind)
          case kind
          when "done"        then do_done
          when "undo_done"   then do_undo_done
          when "snooze"      then do_snooze
          when "unsnooze"    then do_unsnooze
          when "star"        then do_star
          when "unstar"      then do_unstar
          when "archive"     then do_archive
          when "unarchive"   then do_unarchive
          when "paid"        then do_paid
          else
            { success: false, message: "Unsupported action." }
          end
        end

        # ── Done ────────────────────────────────────────────────────────────────

        def do_done
          return { success: false, message: "No active item to dismiss." } unless @item && DONE_KINDS.include?(@item.kind)

          if @item.kind == "follow_up"
            @item.subject&.email_thread&.update_columns(follow_up_dismissed_at: ::Time.current)
          end
          @item.dismiss!
          {
            success:     true,
            message:     "Done.",
            undo_kind:   "undo_done",
            undo_params: { "feed_item_id" => @item.id }
          }
        end

        def do_undo_done
          return { success: false, message: "Item not found." } unless @item

          if @item.kind == "follow_up"
            @item.subject&.email_thread&.update_columns(follow_up_dismissed_at: nil)
          end
          @item.reactivate!
          { success: true, message: "Restored." }
        end

        # ── Snooze ───────────────────────────────────────────────────────────────

        def do_snooze
          return { success: false, message: "No message to snooze." } unless @message

          snooze_until = resolve_snooze_until
          result = EmailActions.run("snooze", email_message: @message,
                                   args: { "snoozed_until" => snooze_until.iso8601 },
                                   user: current_user)
          return { success: false, message: result[:message] } unless result[:success]

          {
            success:     true,
            message:     result[:message],
            undo_kind:   "unsnooze",
            undo_params: { "email_message_id" => @message.id }
          }
        end

        def do_unsnooze
          return { success: false, message: "No message to unsnooze." } unless @message

          result = EmailActions.run("unsnooze", email_message: @message, args: {}, user: current_user)
          return { success: false, message: result[:message] } unless result[:success]

          { success: true, message: result[:message] }
        end

        # ── Star ─────────────────────────────────────────────────────────────────

        def do_star
          contact = busiest_contact
          return { success: false, message: "Contact not found." } unless contact

          contact.star!
          {
            success:   true,
            message:   "Starred.",
            undo_kind: "unstar"
          }
        end

        def do_unstar
          contact = busiest_contact
          return { success: false, message: "Contact not found." } unless contact

          contact.unstar!
          {
            success:   true,
            message:   "Unstarred.",
            undo_kind: "star"
          }
        end

        # ── Archive ───────────────────────────────────────────────────────────────

        def do_archive
          return { success: false, message: "No message to archive." } unless @message

          result = EmailActions.run("archive", email_message: @message, args: {}, user: current_user)
          return { success: false, message: result[:message] } unless result[:success]

          {
            success:     true,
            message:     result[:message],
            undo_kind:   "unarchive",
            undo_params: { "email_message_id" => @message.id }
          }
        end

        def do_unarchive
          return { success: false, message: "No message to unarchive." } unless @message

          result = EmailActions.run("unarchive", email_message: @message, args: {}, user: current_user)
          return { success: false, message: result[:message] } unless result[:success]

          { success: true, message: result[:message] }
        end

        # ── Paid ──────────────────────────────────────────────────────────────────

        # Mark the person's late invoice paid: settles the Document (a landed bank
        # match still wins later) and retires the card. Mirrors the web do_paid.
        def do_paid
          unless @item && %w[late_payable late_receivable].include?(@item.kind)
            return { success: false, message: "Nothing to mark paid for this person." }
          end

          document = @item.subject
          unless document.is_a?(Document) && document.workspace_id == current_workspace.id
            return { success: false, message: "Invoice not found." }
          end

          document.mark_settled!
          @item.dismiss!
          { success: true, message: "Marked paid." }
        end

        # ── Helpers ───────────────────────────────────────────────────────────────

        def busiest_contact
          person = @row.counterpart
          return nil unless person.respond_to?(:contacts)

          person.contacts.max_by { |c| c.email_count.to_i }
        end

        def resolve_snooze_until
          # Accept an explicit ISO8601 datetime, or a named preset key.
          if params[:snooze_until].present?
            ::Time.zone.parse(params[:snooze_until].to_s) rescue nil
          end || resolve_snooze_preset
        end

        def resolve_snooze_preset
          preset_key = params[:until].to_s
          presets = snooze_presets
          preset = presets.find { |key, _label, _time| key.to_s == preset_key }
          preset ? preset[2] : (presets.find { |key, _l, _t| key == :tomorrow }&.last || 1.day.from_now.change(hour: 9))
        end

        # Inline snooze preset builder (mirrors EmailMessageHelpers#snooze_presets,
        # which is a view helper we cannot include in an API controller).
        def snooze_presets
          now = ::Time.current
          today_4pm = now.change(hour: 16, min: 0, sec: 0)
          today_4pm += 1.day if today_4pm <= now

          today_8pm = now.change(hour: 20, min: 0, sec: 0)
          today_8pm += 1.day if today_8pm <= now

          tomorrow_9am = 1.day.from_now.change(hour: 9, min: 0, sec: 0)
          saturday_9am = next_saturday.change(hour: 9, min: 0, sec: 0)
          monday_9am   = next_monday.change(hour: 9, min: 0, sec: 0)

          [
            [ :later_today,  "Later today",  today_4pm ],
            [ :this_evening, "This evening", today_8pm ],
            [ :tomorrow,     "Tomorrow",     tomorrow_9am ],
            [ :this_weekend, "This weekend", saturday_9am ],
            [ :next_week,    "Next week",    monday_9am ]
          ]
        end

        def next_saturday
          today = ::Date.current
          days_ahead = (6 - today.wday) % 7
          days_ahead = 7 if days_ahead.zero?
          (today + days_ahead).in_time_zone
        end

        def next_monday
          today = ::Date.current
          days_ahead = (1 - today.wday) % 7
          days_ahead = 7 if days_ahead.zero?
          (today + days_ahead).in_time_zone
        end
      end
    end
  end
end
