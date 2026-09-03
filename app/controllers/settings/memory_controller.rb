# frozen_string_literal: true

module Settings
  # Scout's memory (bold layout): the inbox/AI settings pages re-expressed as
  # editable sentences. Reads the derived Scout::Memory::Catalog; teach creates a
  # real record; confirm/remove act on a learned-or-taught entry. Gated on the
  # bold-layout flag alone (the page works for classic-mode users when the flag is
  # on) — see require_bold_layout_enabled.
  class MemoryController < Settings::BaseController
    before_action :require_bold_layout_enabled

    def show
      @facet = normalize_facet(params[:facet])
      @query = params[:q].to_s
      @facet_counts = catalog.facet_counts
      @total = catalog.total
      @entries = catalog.entries_for(@facet)
    end

    def teach
      @result = Scout::Memory::Teacher.new(workspace: Current.workspace, user: current_user).learn(params[:sentence])
      @entry = catalog.entry(@result.entry_id) if @result.created?

      respond_to do |format|
        format.turbo_stream
        format.html { redirect_to settings_memory_path }
      end
    end

    def confirm
      act(:confirm)
    end

    def destroy
      act(:remove)
    end

    private

    def act(action)
      @entry = catalog.perform(action, params[:id])
      @action = action

      respond_to do |format|
        format.turbo_stream { render :update_entry }
        format.html { redirect_to settings_memory_path }
      end
    end

    def catalog
      @catalog ||= Scout::Memory::Catalog.for(Current.workspace, current_user)
    end

    def normalize_facet(facet)
      return nil if facet.blank?

      symbol = facet.to_sym
      Scout::Memory::Entry::FACETS.include?(symbol) ? symbol : nil
    end
  end
end
