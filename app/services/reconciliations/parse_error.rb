# frozen_string_literal: true

module Reconciliations
  # Raised by CsvParser when the input cannot be interpreted as a bank statement.
  # Caught by ParseJob and persisted as `parse_error` on the Reconciliation.
  class ParseError < StandardError; end
end
