# frozen_string_literal: true

module RuboCop
  module Cop
    module Campbooks
      # This project standardized on RSpec (`spec/`). Minitest test classes are
      # not allowed — write an equivalent spec instead.
      #
      # @example
      #   # bad
      #   class WidgetTest < ActiveSupport::TestCase
      #   end
      #
      #   # good
      #   RSpec.describe Widget do
      #   end
      class NoMinitest < Base
        MSG = "Write tests with RSpec in spec/, not Minitest — this project " \
              "standardized on RSpec (see .rubocop.yml Campbooks/NoMinitest)."

        # Minitest base classes (Rails' TestCase family + plain Minitest).
        MINITEST_BASES = %w[
          ActiveSupport::TestCase
          ActionDispatch::IntegrationTest
          ActionController::TestCase
          ActionMailer::TestCase
          ActionView::TestCase
          ActiveJob::TestCase
          ActionCable::Channel::TestCase
          ActionCable::Connection::TestCase
          Minitest::Test
          Minitest::Spec
        ].freeze

        def on_class(node)
          superclass = node.parent_class
          return unless superclass

          add_offense(superclass) if MINITEST_BASES.include?(superclass.source.gsub(/\s+/, ""))
        end
      end
    end
  end
end
