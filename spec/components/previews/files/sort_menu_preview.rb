# frozen_string_literal: true

module Files
  # Preview for the grid-view sort dropdown. On the Files page it is hidden
  # until grid mode reveals it (data-files-pane="sort"); the wrapper here forces
  # it visible so the states can be inspected in isolation.
  class SortMenuPreview < Lookbook::Preview
    # Default order (no explicit sort) — the summary shows "Added".
    def default
      render_with_template(template: "files/sort_menu_preview/wrapped",
        locals: { sorter: Documents::Sorter.from_params({}) })
    end

    # Active sort — name ascending.
    def active_name_asc
      render_with_template(template: "files/sort_menu_preview/wrapped",
        locals: { sorter: Documents::Sorter.from_params({ sort: "name", dir: "asc" }) })
    end

    # Active sort — added descending.
    def active_added_desc
      render_with_template(template: "files/sort_menu_preview/wrapped",
        locals: { sorter: Documents::Sorter.from_params({ sort: "added", dir: "desc" }) })
    end
  end
end
