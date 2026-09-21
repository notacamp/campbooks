# frozen_string_literal: true

require "pagy/extras/countless"

module Api
  module App
    module People
      # GET /api/app/people/streams        — streams list
      # GET /api/app/people/streams/:name  — single stream's thread list
      class StreamsController < Api::App::BaseController
        STREAM_THREADS_PER_PAGE = 25

        # GET /api/app/people/streams
        def index
          groups = tag_groups_service.build_groups(inbox_folder_ids)
          streams = groups.map { |group| stream_summary(group) }
          render_data(streams.map { |s| Api::App::StreamSerializer.new(s).as_json })
        end

        # GET /api/app/people/streams/:name
        def show
          stream_name = params[:name].to_s
          base = tag_groups_service.group_scope(stream_name)
          return render_not_found unless base

          scoped = base.where(email_account_id: readable_account_ids)
                       .includes(:email_account, :email_messages)
                       .order(Arel.sql("(SELECT MAX(m.received_at) FROM email_messages m WHERE m.email_thread_id = email_threads.id) DESC NULLS LAST"))

          pagy, threads = pagy_countless(scoped, limit: STREAM_THREADS_PER_PAGE)
          render_page(threads.map { |t| thread_summary(t) }, pagy)
        end

        private

        def stream_summary(group)
          name = group[:label]
          {
            name:    name,
            icon:    nil,
            count:   group[:count],
            last_at: group_last_at(name),
            note:    nil
          }
        end

        def group_last_at(name)
          scope = tag_groups_service.group_scope(name)
          return nil if scope.nil?

          scope.joins(:email_messages).maximum("email_messages.received_at")
        end

        def thread_summary(thread)
          newest = thread.email_messages.max_by(&:received_at)
          {
            id:         thread.id,
            subject:    thread.display_subject,
            account_id: thread.email_account_id,
            newest_at:  newest&.received_at&.iso8601
          }
        end

        def readable_accounts
          @readable_accounts ||= current_user.readable_email_accounts.ordered.to_a
        end

        def readable_account_ids
          @readable_account_ids ||= readable_accounts.map(&:id)
        end

        def inbox_folder_ids
          @inbox_folder_ids ||= Emails::InboxFolders.ids_for(readable_accounts)
        end

        def tag_groups_service
          @tag_groups_service ||= Emails::TagGroups.new(current_workspace, readable_account_ids)
        end
      end
    end
  end
end
