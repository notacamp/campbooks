# frozen_string_literal: true

module Files
  # Preview for the sortable column header used in the Files table. Shows the
  # inactive, active-ascending, and active-descending states.
  class SortableHeaderPreview < Lookbook::Preview
    # Inactive — not the current sort column.
    def inactive
      render_with_template(locals: {
        label: "Name",
        sort_key: "name",
        sorter: inactive_sorter,
        filters: Documents::Filters.new,
        q: nil,
        folder: nil,
        width_class: "w-5/12"
      })
    end

    # Active ascending — this column is the active sort, direction asc.
    def active_asc
      render_with_template(locals: {
        label: "Added",
        sort_key: "added",
        sorter: active_sorter("added", "asc"),
        filters: Documents::Filters.new,
        q: nil,
        folder: nil,
        width_class: "w-2/12"
      })
    end

    # Active descending — this column is the active sort, direction desc.
    def active_desc
      render_with_template(locals: {
        label: "Added",
        sort_key: "added",
        sorter: active_sorter("added", "desc"),
        filters: Documents::Filters.new,
        q: nil,
        folder: nil,
        width_class: "w-2/12"
      })
    end

    private

    def inactive_sorter
      Documents::Sorter.from_params({})
    end

    def active_sorter(key, dir)
      Documents::Sorter.from_params({ sort: key, dir: dir })
    end
  end
end
