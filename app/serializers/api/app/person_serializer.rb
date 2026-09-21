# frozen_string_literal: true

module Api
  module App
    # Serializes the full person page: standing, stand note, contact facts, and
    # the first page of conversation threads. Designed for GET /api/app/people/:id.
    class PersonSerializer
      def initialize(person, standing:, threads:, threads_pagy:, profile: nil)
        @person       = person
        @standing     = standing
        @threads      = threads
        @threads_pagy = threads_pagy
        @profile      = profile
      end

      def as_json
        data = {
          id:          @person.id,
          name:        @person.display_name,
          stand_note:  stand_note,
          threads:     @threads.map { |ct| Api::App::ConversationThreadSerializer.new(ct).as_json },
          threads_meta: threads_meta
        }

        if @profile
          data[:contact] = contact_facts(@profile)
        end

        data
      end

      private

      def stand_note
        ::People::StandCopy.note(
          @standing,
          name: @person.display_name,
          zone: Current.user&.effective_time_zone || ::Time.zone
        )
      rescue StandardError
        nil
      end

      def threads_meta
        return {} unless @threads_pagy

        {
          page:        @threads_pagy.page,
          total:       @threads_pagy.count,
          total_pages: @threads_pagy.pages
        }
      rescue StandardError
        {}
      end

      def contact_facts(profile)
        {
          emails:       (profile.emails || []).map { |addr, primary| { address: addr, primary: primary } },
          organization: profile.organization&.then { |o| { id: o.id, name: o.name } },
          relationship: profile.relationship,
          sender_kind:  profile.sender_kind,
          starred:      profile.starred?,
          tags:         profile.tags&.map { |t| { id: t.id, name: t.name } } || [],
          email_total:  profile.counts&.dig(:total) || 0
        }
      rescue StandardError
        {}
      end
    end
  end
end
