# frozen_string_literal: true

module Api
  module App
    # Serializes the org page: standing, stand note, and the PeopleStanding rows
    # for member persons and services.
    class OrgSerializer
      def initialize(org, standing:, person_rows:, service_rows:)
        @org          = org
        @standing     = standing
        @person_rows  = person_rows
        @service_rows = service_rows
      end

      def as_json
        {
          id:         @org.id,
          name:       @org.name,
          stand_note: stand_note,
          people:     @person_rows.map { |r| Api::App::PeopleStandingSerializer.new(r).as_json },
          services:   @service_rows.map { |r| Api::App::PeopleStandingSerializer.new(r).as_json }
        }
      end

      private

      def stand_note
        ::People::StandCopy.note(
          @standing,
          name: @org.name,
          zone: Current.user&.effective_time_zone || ::Time.zone
        )
      rescue StandardError
        nil
      end
    end
  end
end
