- **Tracked events can go to your RabbitMQ.** With `CAMPBOOKS_TRACKING__SINK=amqp` and
  `CAMPBOOKS_TRACKING__AMQP_URL`, the worker publishes each tracked event once its change
  commits, to the topic exchange `campbooks.events` with a stream of the same name kept
  for 30 days, and marks it sent only when the broker confirms it. While the broker is
  down, events wait in the database and go out when it is back; sent ones are deleted
  after a day. Off by default, and optional for self-hosters: `deploy/README.md` →
  Monitoring → Tracked events.
