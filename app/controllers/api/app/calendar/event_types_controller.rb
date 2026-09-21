# frozen_string_literal: true

module Api
  module App
    module Calendar
      # Event types CRUD for the workspace. Event types are workspace-scoped
      # "tags" (name + icon + AI prompt) for auto-classifying calendar events.
      class EventTypesController < Api::App::BaseController
        before_action :set_type, only: %i[update destroy]

        def index
          types = current_workspace.event_types.order(:name)
          render_data(types.map { |t| serialize(t) })
        end

        def create
          type = current_workspace.event_types.new(type_params)
          type.save!
          render_data(serialize(type), status: :created)
        end

        def update
          @type.update!(type_params)
          render_data(serialize(@type))
        end

        def destroy
          @type.destroy!
          head :no_content
        end

        # POST /api/app/event_types/starters — seed the starter set, skipping
        # names that already exist (idempotent).
        def starters
          existing = current_workspace.event_types.pluck(:name).map(&:downcase)
          created = EventType::STARTERS.filter_map do |attrs|
            next if existing.include?(attrs[:name].downcase)
            current_workspace.event_types.create!(attrs)
          end
          types = current_workspace.event_types.order(:name)
          render_data(types.map { |t| serialize(t) })
        end

        private

        def set_type
          @type = current_workspace.event_types.find(params[:id])
        end

        def type_params
          params.permit(:name, :icon, :prompt)
        end

        def serialize(event_type)
          Api::App::Calendar::EventTypeSerializer.new(event_type).as_json
        end
      end
    end
  end
end
