# frozen_string_literal: true

module Campbooks
  class AvatarSizes < Base
    def initialize(name: nil)
      @name = name
    end

    def view_template
      div(class: "flex items-center gap-3 p-6") do
        %i[sm md lg].each { |s| render Avatar.new(name: @name, size: s) }
      end
    end
  end
end
