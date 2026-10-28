- **Scout's note no longer stays blank when it could not read a message.** The model that reads
  your mail sometimes spent its whole allowance thinking and wrote nothing, so a conversation's
  "Where things stand" note had nothing to say for good. Scout now thinks less on this job, asks
  again once with more room when an answer comes back empty, and when a message still cannot be
  read the note says "Scout couldn't read this one." A newer message in the same conversation is
  read afresh. Self-hosters on a provider that takes OpenRouter's `reasoning` setting can turn
  it on with `CAMPBOOKS_AI__COMPLETE__REASONING=true`.
