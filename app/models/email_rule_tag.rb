# frozen_string_literal: true

# Join model between EmailRule and Tag. Represents the "apply this tag" action
# of a rule.  Both sides cascade-delete via the DB constraint (Migration B).
class EmailRuleTag < ApplicationRecord
  belongs_to :email_rule
  belongs_to :tag
end
