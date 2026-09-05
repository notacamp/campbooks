# frozen_string_literal: true

class RenamePeopleStandingsTextToDetail < ActiveRecord::Migration[8.1]
  def change
    rename_column :people_standings, :text, :detail
    add_column    :people_standings, :detail_kind, :string
  end
end
