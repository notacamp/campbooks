# frozen_string_literal: true

require "pagy/extras/array"

module Api
  module App
    module People
      # GET /api/app/people/:id
      # The person page: standing + stand note + first page of threads + contact facts.
      # Marks the newest thread read (like the web app does on open).
      class PersonController < Api::App::BaseController
        THREADS_PER_PAGE = 8

        def show
          person = current_workspace.people.find(params[:id])
          build_conversation(person)
          standing = resolve_standing(person)

          render_data(
            Api::App::PersonSerializer.new(
              person,
              standing:     standing,
              threads:      @conversation_threads,
              threads_pagy: @conversation_pagy,
              profile:      nil  # profile is available via /details
            ).as_json
          )
        end

        private

        def build_conversation(person)
          contact_ids = person.contacts.ids
          thread_ids  = conversation_thread_ids(contact_ids)

          latest_per_thread = EmailMessage.where(email_thread_id: thread_ids)
                                          .accessible_to(current_user)
                                          .group(:email_thread_id)
                                          .maximum(:received_at)
          ordered_ids = latest_per_thread.sort_by { |_id, at| at || ::Time.at(0) }.map(&:first).reverse

          @conversation_pagy, page_ids = pagy_array(ordered_ids, limit: THREADS_PER_PAGE,
                                                     page: params[:page])

          threads_by_id    = EmailThread.where(id: page_ids).index_by(&:id)
          heads_by_thread  = EmailMessage.where(email_thread_id: page_ids)
                                         .accessible_to(current_user)
                                         .pluck(:email_thread_id, :id, :received_at)
                                         .group_by(&:first)

          eager_id      = @conversation_pagy.page == 1 ? page_ids.first : nil
          eager_messages = eager_id ? conversation_messages(eager_id) : nil

          @conversation_threads = page_ids.filter_map do |tid|
            thread = threads_by_id[tid]
            heads  = heads_by_thread[tid]
            next unless thread && heads&.any?

            if tid == eager_id
              ::People::ConversationThread.new(thread: thread, messages: eager_messages)
            else
              _tid, newest_id, latest_at = heads.max_by { |(_t, _id, at)| at || ::Time.at(0) }
              ::People::ConversationThread.new(thread: thread, count: heads.size,
                                               latest_at: latest_at, newest_id: newest_id)
            end
          end

          # Mark the newest thread read (mirrors PeopleController#mark_newest_thread_read).
          newest_thread = @conversation_pagy.page == 1 ? @conversation_threads.first : nil
          if newest_thread&.thread && Emails::MarkThreadRead.call(newest_thread.thread)
            ::People::Standings.refresh_counterpart!(current_user, person)
          end
        end

        def conversation_thread_ids(contact_ids)
          return [] if contact_ids.empty?

          EmailMessage.where(contact_id: contact_ids).where.not(email_thread_id: nil)
                      .distinct.pluck(:email_thread_id)
        end

        def conversation_messages(thread_id)
          EmailMessage.where(email_thread_id: thread_id)
                      .accessible_to(current_user)
                      .includes(:contact, :email_account, files_attachments: :blob)
                      .order(:received_at, :id).to_a
        end

        def resolve_standing(person)
          PeopleStanding.for_user(current_user).find_by(counterpart: person)&.standing ||
            ::People::Standing.for_person(person, user: current_user)
        end
      end
    end
  end
end
