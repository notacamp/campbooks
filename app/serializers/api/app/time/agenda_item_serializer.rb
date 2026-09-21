# frozen_string_literal: true

module Api
  module App
    module Time
      # Serializes one Time::AgendaItem (a Data.define value object) to a plain hash
      # the SPA renders as an agenda row. Carries kind-specific extras so the client
      # never has to reverse-engineer the kind from the record fields.
      class AgendaItemSerializer
        def initialize(item)
          @item = item
        end

        def as_json
          base = {
            kind:             @item.kind,
            id:               record_id,
            title:            @item.title,
            at:               @item.at&.iso8601,
            day:              @item.day&.iso8601,
            all_day:          @item.all_day,
            overdue:          @item.overdue,
            duration_minutes: @item.duration_minutes,
            color:            @item.color,
            source_label:     @item.source_label,
            source_path:      @item.source_path,
            emphasis:         @item.emphasis,
            why:              @item.why,
            prep_name:        @item.prep_name,
            prep_detail:      @item.prep_detail,
            handed:           @item.handed,
            actions:          @item.actions
          }

          base.merge(kind_extras)
        end

        private

        def record_id
          r = @item.record
          return nil unless r

          r.id
        end

        def kind_extras
          case @item.kind
          when :task
            task_extras
          when :deadline
            reminder_extras
          when :event
            event_extras
          when :focus
            focus_extras
          else
            {}
          end
        end

        def task_extras
          task = @item.record
          return {} unless task

          {
            task: {
              id:           task.id,
              status:       task.status,
              priority:     task.priority,
              ai_suggested: task.ai_suggested,
              snoozed_until: task.snoozed_until&.iso8601
            }
          }
        end

        def reminder_extras
          reminder = @item.record
          return {} unless reminder

          {
            reminder: {
              id:            reminder.id,
              status:        reminder.status,
              reminder_type: reminder.reminder_type,
              snoozed_until: reminder.snoozed_until&.iso8601
            }
          }
        end

        def event_extras
          event = @item.record
          return {} unless event

          {
            event: {
              id:               event.id,
              provider_event_id: event.provider_event_id,
              join_url:         event.join_url,
              rsvp_status:      event.rsvp_status,
              calendar_id:      event.calendar_id
            }
          }
        end

        def focus_extras
          block = @item.record
          return {} unless block

          {
            focus_block: {
              id:               block.id,
              status:           block.status,
              task_id:          block.task_id,
              calendar_event_id: block.calendar_event_id
            }
          }
        end
      end
    end
  end
end
