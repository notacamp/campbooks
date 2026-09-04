# frozen_string_literal: true

module People
  # One row in the People list: a person or an organization, with its display
  # bits and Scout's standing precomputed by PeopleController so the row component
  # (Campbooks::People::CounterpartRow) renders without extra queries.
  #
  # `subtitle` is the second line — a person's organization name, or an org's
  # "Organization · N people · N services". `standing` is a People::Standing::Result.
  # `facts` / `score` are the People::Priority inputs and verdict the list ranks
  # by; rows built off the list (a detail pane) may leave them nil.
  #
  # `id` is explicit so rows read from the `people_standings` table can build
  # a Counterpart without a live record (`record` is optional / nil in that path).
  Counterpart = Data.define(
    :id, :kind, :record, :name, :subtitle, :avatar_email, :avatar_initial,
    :last_activity, :standing, :facts, :score
  ) do
    # `id` defaults to the record's id when not given; `record`, `facts`, and
    # `score` are optional so the table-read path can omit them.
    def initialize(id: nil, record: nil, facts: nil, score: nil, **rest)
      super(id: id || record&.id, record: record, facts: facts, score: score, **rest)
    end

    def person? = kind == :person
    def organization? = kind == :organization
    def needs_you? = standing.needs_you
    def overdue_days = standing.overdue_days
    def priority = score&.value || 0.0
  end
end
