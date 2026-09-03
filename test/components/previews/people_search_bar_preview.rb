# frozen_string_literal: true

# Preview for Campbooks::People::SearchBar — the People list's live search field.
class PeopleSearchBarPreview < Lookbook::Preview
  def empty
    render(Campbooks::People::SearchBar.new)
  end

  def with_query
    render(Campbooks::People::SearchBar.new(q: "cloudhost"))
  end
end
