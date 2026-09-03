# frozen_string_literal: true

# Previews for Campbooks::People::StreamIcon — the round kind tile for a stream.
class PeopleStreamIconPreview < Lookbook::Preview
  def bell = render(Campbooks::People::StreamIcon.new(kind: :bell))
  def mail = render(Campbooks::People::StreamIcon.new(kind: :mail))
  def file = render(Campbooks::People::StreamIcon.new(kind: :file))
  def users = render(Campbooks::People::StreamIcon.new(kind: :users))
end
