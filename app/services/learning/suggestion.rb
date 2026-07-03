module Learning
  # The result of a consensus lookup: the winning `label` (domain-opaque — a
  # DocumentType id, a skim action, "confirmed"/"dismissed", …), which signal
  # tier it came from (`source`), and the supporting counts (`count` of the
  # winning label out of `total` examples seen for that tier). Shared by every
  # domain so the engine stays domain-agnostic.
  Suggestion = Data.define(:label, :source, :count, :total)
end
