# frozen_string_literal: true

module Api
  module App
    module Settings
      # Scout's memory — the inbox/AI settings as editable sentences derived on
      # demand from the behavior records. Teach creates a real rule/habit; confirm
      # and remove act on a learned or taught entry.
      class MemoryController < Api::App::BaseController
        # GET /api/app/settings/memory
        def show
          facet = normalize_facet(params[:facet])
          entries = catalog.entries_for(facet)

          render_data({
            entries: entries.map { |e| Api::App::Settings::MemoryEntrySerializer.new(e).as_json },
            facet_counts: catalog.facet_counts.to_h,
            total: catalog.total
          })
        end

        # POST /api/app/settings/memory/teach
        def teach
          result = ::Scout::Memory::Teacher.new(workspace: current_workspace, user: current_user)
                                         .learn(params[:sentence])

          if result.created?
            entry = catalog.entry(result.entry_id)
            render json: {
              data: {
                created: true,
                entry: entry ? Api::App::Settings::MemoryEntrySerializer.new(entry).as_json : nil
              }
            }, status: :created
          else
            render json: { data: { created: false, reason: result.respond_to?(:reason) ? result.reason : nil } }
          end
        end

        # POST /api/app/settings/memory/entries/:id/confirm
        def confirm
          entry = catalog.perform(:confirm, params[:id])
          render_data(Api::App::Settings::MemoryEntrySerializer.new(entry).as_json)
        rescue => e
          render_error("invalid", e.message, status: :unprocessable_entity)
        end

        # DELETE /api/app/settings/memory/entries/:id
        def destroy
          catalog.perform(:remove, params[:id])
          render json: {}, status: :no_content
        rescue => e
          render_error("invalid", e.message, status: :unprocessable_entity)
        end

        private

        def catalog
          @catalog ||= ::Scout::Memory::Catalog.for(current_workspace, current_user)
        end

        def normalize_facet(facet)
          return nil if facet.blank?

          symbol = facet.to_sym
          ::Scout::Memory::Entry::FACETS.include?(symbol) ? symbol : nil
        end
      end
    end
  end
end
